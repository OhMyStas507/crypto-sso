//! ViewModel 层：编排业务用例和事务，向 View 暴露明确的类型。
mod auth;
pub mod contracts;
mod enrollment;
mod idempotency;
mod queries;
mod review;
mod settings;

use crate::{
    config::Config,
    error::Error,
    models::{ClaimHintRevision, IssuerMetadata, Settings},
    security::*,
    store::{IsolationLevel, Store},
};
use crypto_sso_identity_api::ClaimHint;
use crypto_sso_issuer::{Issuer, IssuerPublic, wire};
use std::sync::Arc;

/// 签发服务 ViewModel，共享配置、数据库连接池与签发器，协调各业务用例。
#[derive(Clone)]
pub struct Application {
    pub config: Arc<Config>,
    pub store: Store,
    pub issuer: Arc<Issuer>,
}

impl Application {
    #[tracing::instrument(skip_all)]
    pub async fn bootstrap(config: Config) -> anyhow::Result<Self> {
        let public: IssuerPublic = wire::from_json(&std::fs::read(&config.issuer_public_path)?)?;
        let secret = zeroize::Zeroizing::new(std::fs::read(&config.issuer_secret_path)?);
        let issuer = Arc::new(Issuer::import_secret(&secret, &public)?);
        let store = Store::connect(&config.database_url).await?;
        let mut db = store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        if let Some(id) = IssuerMetadata::filter_by_id(1)
            .first()
            .exec(&mut tx)
            .await?
        {
            if id.issuer_id != issuer.issuer_id().as_str() {
                return Err(
                    Error::new(409, "ISSUER_MISMATCH", "数据库固定的签发身份与配置不符").into(),
                );
            }
        } else {
            toasty::create!(IssuerMetadata {
                id: 1,
                issuer_id: issuer.issuer_id().as_str().to_owned()
            })
            .exec(&mut tx)
            .await?;
        }
        if Settings::filter_by_id(1)
            .first()
            .exec(&mut tx)
            .await?
            .is_none()
        {
            let initial = toasty::create!(Settings {
                id: 1,
                revision: 1,
                claim_hints_revision: 1,
                display_name: public.organization().to_owned(),
                description: "",
                contact: "",
                public_base_url: config.public_origin.clone(),
                registration_enabled: false,
                issuance_enabled: false,
                review_ttl_seconds: 7 * 86400,
                poll_interval_seconds: 5,
                updated_at: now(),
                claim_hints: Vec::<ClaimHint>::new(),
            })
            .exec(&mut tx)
            .await?;
            toasty::create!(ClaimHintRevision {
                revision: initial.claim_hints_revision,
                hints: initial.claim_hints,
            })
            .exec(&mut tx)
            .await?;
        }
        tx.commit().await?;
        Ok(Self {
            config: Arc::new(config),
            store,
            issuer,
        })
    }
}
