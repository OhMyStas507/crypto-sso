//! ViewModel 层：编排业务用例和事务，向 View 暴露明确的类型。
mod auth;
pub mod contracts;
mod enrollment;
mod idempotency;
mod queries;
mod review;
mod settings;
mod setup;

use crate::{
    admin_secrets::AdminSecrets,
    config::Config,
    error::{Error, Result},
    models::*,
    redis_store::RedisStore,
    security::*,
    session_store::RedisSessionStore,
    store::{IsolationLevel, Store},
};
use crypto_sso_issuer::Issuer;
use std::sync::{Arc, RwLock};

/// 签发服务 ViewModel，共享配置、PostgreSQL/Redis 连接与签发器，协调各业务用例。
#[derive(Clone)]
pub struct Application {
    pub config: Arc<Config>,
    pub store: Store,
    pub redis: RedisStore,
    pub http_sessions: RedisSessionStore,
    pub(crate) admin_secrets: Arc<AdminSecrets>,
    issuer: Arc<RwLock<Option<Arc<Issuer>>>>,
}

impl Application {
    #[tracing::instrument(skip_all)]
    pub async fn bootstrap(config: Config) -> anyhow::Result<Self> {
        let redis = RedisStore::connect(&config.redis_url).await?;
        let admin_secrets = Arc::new(AdminSecrets::load(&redis).await?);
        let http_sessions = RedisSessionStore::new(redis.clone());
        let store = Store::connect(&config.database_url).await?;
        let app = Self {
            config: Arc::new(config),
            store,
            redis,
            http_sessions,
            admin_secrets,
            issuer: Arc::new(RwLock::new(None)),
        };
        app.initialize_state().await?;
        if app.initialized().await? {
            app.issuer().await?;
        } else {
            app.ensure_setup_code(false, false).await?;
        }
        Ok(app)
    }

    async fn initialize_state(&self) -> anyhow::Result<()> {
        let store = &self.store;
        let mut db = store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        if Initialization::filter_by_id(1)
            .first()
            .exec(&mut tx)
            .await?
            .is_some()
        {
            return Ok(());
        }
        let metadata = IssuerMetadata::filter_by_id(1)
            .first()
            .exec(&mut tx)
            .await?;
        let settings = Settings::filter_by_id(1).first().exec(&mut tx).await?;
        let has_data = Enrollment::all().first().exec(&mut tx).await?.is_some()
            || Member::all().first().exec(&mut tx).await?.is_some()
            || Administrator::all().first().exec(&mut tx).await?.is_some()
            || AuditRecord::all().first().exec(&mut tx).await?.is_some()
            || StoredChallenge::all()
                .first()
                .exec(&mut tx)
                .await?
                .is_some()
            || ApplicationSession::all()
                .first()
                .exec(&mut tx)
                .await?
                .is_some()
            || ClaimHintRevision::all()
                .first()
                .exec(&mut tx)
                .await?
                .is_some()
            || Idempotency::all().first().exec(&mut tx).await?.is_some();
        // 控制记录丢失但仍有业务记录属于损坏状态，不允许重新领取管理员权限。
        anyhow::ensure!(
            metadata.is_none() && settings.is_none() && !has_data,
            "RECOVERY_REQUIRED: initialization control is missing while business data exists"
        );
        toasty::create!(Initialization {
            id: 1,
            completed: false,
            revision: 1,
            draft_id: token(),
            draft_expires_at: now() + 86400,
            code_hash: "",
            code_expires_at: 0,
            code_used: false,
            draft: InitializationDraft::default(),
        })
        .exec(&mut tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn initialized(&self) -> Result<bool> {
        Ok(Initialization::get_by_id(&mut self.store.database(), &1)
            .await?
            .completed)
    }

    pub async fn issuer(&self) -> Result<Arc<Issuer>> {
        if let Some(issuer) = self
            .issuer
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            return Ok(issuer.clone());
        }
        let state = Initialization::get_by_id(&mut self.store.database(), &1).await?;
        if !state.completed {
            return Err(Error::new(
                503,
                "CENTER_NOT_INITIALIZED",
                "身份中心尚未初始化",
            ));
        }
        let key = state
            .draft
            .key_id
            .as_ref()
            .ok_or_else(|| Error::new(503, "RECOVERY_REQUIRED", "固定签发密钥引用缺失"))?;
        let material = InitializationKey::get_by_id(&mut self.store.database(), key).await?;
        let issuer = tokio::task::spawn_blocking(move || crate::key_store::load(&material))
            .await
            .map_err(|_| Error::unavailable())??;
        let metadata = IssuerMetadata::get_by_id(&mut self.store.database(), &1).await?;
        if metadata.issuer_id != issuer.issuer_id().as_str() {
            return Err(Error::new(
                503,
                "RECOVERY_REQUIRED",
                "签发身份不匹配，请恢复原中心配置",
            ));
        }
        let issuer = Arc::new(issuer);
        *self.issuer.write().unwrap_or_else(|e| e.into_inner()) = Some(issuer.clone());
        Ok(issuer)
    }
}
