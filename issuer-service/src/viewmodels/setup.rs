use super::Application;
use crate::{
    error::{Error, Result},
    key_store,
    models::domain::audit,
    models::*,
    security::*,
    store::IsolationLevel,
};
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use crypto_sso_identity_api::*;
use crypto_sso_issuer::{IssuerPublic, wire};
use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use sha2::Sha256;
use std::path::PathBuf;
use zeroize::Zeroizing;

fn finished() -> Error {
    Error::new(
        409,
        "ALREADY_INITIALIZED",
        "身份中心已完成初始化，请前往管理员登录",
    )
}
fn expired() -> Error {
    Error::new(
        410,
        "SETUP_DRAFT_EXPIRED",
        "初始化草稿已过期，请部署维护者显式废弃过期草稿后重新验证",
    )
}
fn revision() -> Error {
    Error::new(
        409,
        "SETUP_REVISION_CONFLICT",
        "另一页面已更新草稿，请记录当前输入后加载最新配置",
    )
}
fn validation() -> Error {
    Error::new(
        422,
        "VALIDATION_FAILED",
        "请检查账号、密码或服务信息；密码须为 12–128 个字符",
    )
}

fn require_draft(state: &Initialization, draft_id: &str, expected: Option<u32>) -> Result<()> {
    if state.completed {
        return Err(finished());
    }
    if state.draft_id != draft_id {
        return Err(Error::new(
            401,
            "SETUP_SESSION_EXPIRED",
            "请重新验证部署权限",
        ));
    }
    if state.draft_expires_at <= now() {
        return Err(expired());
    }
    if expected.is_some_and(|r| state.revision != r) {
        return Err(revision());
    }
    Ok(())
}

pub(super) async fn create_settings(
    tx: &mut impl toasty::Executor,
    service: &SetupService,
) -> Result<()> {
    toasty::create!(Settings {
        id: 1,
        revision: 1,
        claim_hints_revision: 1,
        display_name: service.display_name.clone(),
        description: service.description.clone(),
        contact: service.contact.clone(),
        public_base_url: service.public_base_url.clone(),
        registration_enabled: false,
        issuance_enabled: false,
        review_ttl_seconds: 604800,
        poll_interval_seconds: 5,
        updated_at: now(),
        claim_hints: Vec::<ClaimHint>::new(),
    })
    .exec(&mut *tx)
    .await?;
    toasty::create!(ClaimHintRevision {
        revision: 1,
        hints: Vec::<ClaimHint>::new()
    })
    .exec(&mut *tx)
    .await?;
    Ok(())
}

async fn key_view(row: InitializationKey) -> Result<SetupKeyView> {
    let public = row
        .public_json
        .as_ref()
        .map(|raw| {
            wire::from_json::<IssuerPublic>(raw.as_bytes()).map_err(|_| Error::unavailable())
        })
        .transpose()?;
    Ok(SetupKeyView {
        operation_id: row.id,
        organization: row.organization,
        status: row.status,
        issuer_id: public
            .as_ref()
            .map(|p| p.issuer_id().map(|id| id.as_str().to_owned()))
            .transpose()
            .map_err(|_| Error::unavailable())?,
        issuer_public: row
            .public_json
            .map(|raw| serde_json::from_str(&raw))
            .transpose()?,
    })
}

async fn draft_view(state: &Initialization, tx: &mut impl toasty::Executor) -> Result<SetupDraft> {
    let key = if let Some(id) = state.draft.key_id.as_ref() {
        Some(key_view(InitializationKey::get_by_id(tx, id).await?).await?)
    } else {
        None
    };
    Ok(SetupDraft {
        draft_id: state.draft_id.clone(),
        revision: state.revision,
        expires_at: state.draft_expires_at,
        username: state.draft.username.clone(),
        password_configured: !state.draft.password_hash.is_empty(),
        service: state.draft.service.clone(),
        key,
    })
}

impl Application {
    /// 仅回收已明确废弃且租约到期的候选；正式引用和当前草稿均不删除。
    pub async fn cleanup_setup_keys(&self) -> Result<usize> {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let mut state = Initialization::get_by_id(&mut tx, &1).await?;
        let mut ids = Vec::new();
        for mut key in InitializationKey::all().exec(&mut tx).await? {
            if key.draft_id == state.draft_id
                || state.draft.key_id.as_ref() == Some(&key.id)
                || key.lease_until > now()
            {
                continue;
            }
            let id = key.id.clone();
            key.update()
                .status("DISCARDED")
                .owner("")
                .lease_until(0)
                .exec(&mut tx)
                .await?;
            ids.push(id);
        }
        // 与最终提交及草稿废弃竞争同一控制行；清理标记提交后，旧操作无法再启用。
        let rev = state.revision;
        state.update().revision(rev).exec(&mut tx).await?;
        tx.commit().await?;
        for id in &ids {
            key_store::remove_candidate(&self.config, id)?;
        }
        Ok(ids.len())
    }

    /// 域分离的 HMAC 同时用于秘密请求幂等和不可猜测的只读回执。
    pub fn setup_mac(&self, domain: &str, value: &[u8]) -> Result<String> {
        let key = &self.config.setup_storage_key;
        let mut mac =
            <Hmac<Sha256> as Mac>::new_from_slice(key).map_err(|_| Error::unavailable())?;
        mac.update(domain.as_bytes());
        mac.update(&[0]);
        mac.update(value);
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    pub async fn setup_status(&self) -> Result<SetupStatus> {
        let state = Initialization::get_by_id(&mut self.store.database(), &1).await?;
        Ok(SetupStatus {
            state: if state.completed {
                SetupState::Initialized
            } else if state.revision > 1 {
                SetupState::SetupInProgress
            } else {
                SetupState::SetupRequired
            },
        })
    }

    /// 码文件先持久化，再提交摘要；文件名含摘要前缀，失败不会覆盖仍有效的旧码。
    pub async fn ensure_setup_code(&self, rotate: bool, discard_expired: bool) -> Result<PathBuf> {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let mut state = Initialization::get_by_id(&mut tx, &1).await?;
        if state.completed {
            return Err(finished());
        }
        if !rotate && !state.code_hash.is_empty() {
            let path = self.code_path(&state.code_hash);
            tracing::info!(path = %path.display(), "initialization code file; use setup-control to rotate an expired or consumed code");
            return Ok(path);
        }
        if discard_expired {
            if state.draft_expires_at > now() {
                return Err(Error::new(
                    409,
                    "SETUP_DRAFT_ACTIVE",
                    "只能显式废弃已过期草稿",
                ));
            }
            if let Some(id) = state.draft.key_id.as_ref() {
                let candidate = InitializationKey::get_by_id(&mut tx, id).await?;
                if candidate.lease_until > now() {
                    return Err(revision());
                }
            }
            state.draft_id = token();
            state.draft = InitializationDraft::default().into();
            state.revision += 1;
            state.draft_expires_at = now() + 86400;
        }
        let code = Zeroizing::new(token());
        let hash = digest(&code);
        let path = self.code_path(&hash);
        key_store::write_exclusive(&path, code.as_bytes()).map_err(|_| Error::unavailable())?;
        let draft = state.draft.0.clone();
        let draft_id = state.draft_id.clone();
        let rev = state.revision;
        let expiry = state.draft_expires_at;
        state
            .update()
            .code_hash(hash)
            .code_expires_at(now() + 1800)
            .code_used(false)
            .draft_id(draft_id)
            .draft(draft)
            .revision(rev)
            .draft_expires_at(expiry)
            .exec(&mut tx)
            .await?;
        tx.commit().await?;
        tracing::info!(path = %path.display(), "initialization code file created; code is never logged");
        Ok(path)
    }

    fn code_path(&self, hash: &str) -> PathBuf {
        self.config
            .setup_dir
            .join(format!("initialization-code-{}.txt", &hash[..16]))
    }

    pub async fn setup_verify(&self, code: String) -> Result<String> {
        let code = Zeroizing::new(code);
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let mut state = Initialization::get_by_id(&mut tx, &1).await?;
        if state.completed {
            return Err(finished());
        }
        if !equal(&state.code_hash, &digest(code.trim()))
            || state.code_used
            || state.code_expires_at <= now()
        {
            return Err(Error::new(
                401,
                "SETUP_VERIFICATION_FAILED",
                "初始化码无效或已过期，请从部署主机重新领取",
            ));
        }
        let id = state.draft_id.clone();
        let path = self.code_path(&state.code_hash);
        state.update().code_used(true).exec(&mut tx).await?;
        tx.commit().await?;
        let _ = std::fs::remove_file(path);
        Ok(id)
    }

    pub async fn setup_draft(&self, draft_id: &str) -> Result<SetupDraft> {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::RepeatableRead)
            .read_only(true)
            .begin()
            .await?;
        let state = Initialization::get_by_id(&mut tx, &1).await?;
        require_draft(&state, draft_id, None)?;
        let view = draft_view(&state, &mut tx).await?;
        tx.commit().await?;
        Ok(view)
    }

    async fn setup_replay(
        &self,
        tx: &mut impl toasty::Executor,
        state: &Initialization,
        key: &str,
        hash: &str,
    ) -> Result<Option<SetupDraft>> {
        if !valid_token(key) {
            return Err(Error::invalid());
        }
        if let Some(row) = Idempotency::filter_by_key(format!("setup/{}/{key}", state.draft_id))
            .first()
            .exec(tx)
            .await?
        {
            if !equal(&row.digest, hash) {
                return Err(Error::new(
                    409,
                    "IDEMPOTENCY_CONFLICT",
                    "同一操作号对应不同内容，请核对原操作",
                ));
            }
            return Ok(Some(serde_json::from_value(row.result)?));
        }
        Ok(None)
    }

    async fn setup_save(
        &self,
        tx: &mut impl toasty::Executor,
        state: &mut Initialization,
        key: &str,
        hash: &str,
        data: InitializationDraft,
    ) -> Result<SetupDraft> {
        let new_revision = state.revision + 1;
        let expiry = now() + 86400;
        state
            .update()
            .revision(new_revision)
            .draft_expires_at(expiry)
            .draft(data)
            .exec(&mut *tx)
            .await?;
        let view = draft_view(state, &mut *tx).await?;
        toasty::create!(Idempotency {
            key: format!("setup/{}/{key}", state.draft_id),
            digest: hash.to_owned(),
            result: serde_json::to_value(&view)?
        })
        .exec(tx)
        .await?;
        Ok(view)
    }

    pub async fn setup_admin(
        &self,
        draft_id: &str,
        mut input: SetupAdminInput,
        key: &str,
    ) -> Result<SetupDraft> {
        let encoded = Zeroizing::new(serde_json::to_vec(&input)?);
        let hash = self.setup_mac("admin", &encoded)?;
        let password = input.password.take().map(Zeroizing::new);
        let username = input.username.trim().to_owned();
        if !(3..=128).contains(&username.chars().count())
            || username.chars().any(char::is_control)
            || password
                .as_ref()
                .is_some_and(|p| !(12..=128).contains(&p.chars().count()))
        {
            return Err(validation());
        }
        let password_hash = if let Some(password) = password {
            Some(
                tokio::task::spawn_blocking(move || {
                    Argon2::default()
                        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
                        .map(|h| h.to_string())
                        .map_err(|_| Error::unavailable())
                })
                .await
                .map_err(|_| Error::unavailable())??,
            )
        } else {
            None
        };
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let mut state = Initialization::get_by_id(&mut tx, &1).await?;
        require_draft(&state, draft_id, None)?;
        if let Some(view) = self.setup_replay(&mut tx, &state, key, &hash).await? {
            return Ok(view);
        }
        require_draft(&state, draft_id, Some(input.expected_revision))?;
        let mut data = state.draft.0.clone();
        data.username = username;
        if let Some(hash) = password_hash {
            data.password_hash = hash;
        }
        if data.password_hash.is_empty() {
            return Err(validation());
        }
        let view = self
            .setup_save(&mut tx, &mut state, key, &hash, data)
            .await?;
        tx.commit().await?;
        Ok(view)
    }

    pub async fn setup_service(
        &self,
        draft_id: &str,
        input: SetupServiceInput,
        key: &str,
    ) -> Result<SetupDraft> {
        let s = &input.service;
        if s.display_name.trim().is_empty()
            || s.display_name.len() > 128
            || s.description.len() > 4096
            || s.contact.len() > 1024
            || crate::config::canonical_origin(&s.public_base_url).is_err()
        {
            return Err(validation());
        }
        let hash = self.setup_mac("service", &serde_json::to_vec(&input)?)?;
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let mut state = Initialization::get_by_id(&mut tx, &1).await?;
        require_draft(&state, draft_id, None)?;
        if let Some(view) = self.setup_replay(&mut tx, &state, key, &hash).await? {
            return Ok(view);
        }
        require_draft(&state, draft_id, Some(input.expected_revision))?;
        let mut data = state.draft.0.clone();
        data.service = Some(input.service);
        let view = self
            .setup_save(&mut tx, &mut state, key, &hash, data)
            .await?;
        tx.commit().await?;
        Ok(view)
    }

    /// 操作号同时决定不可覆盖的文件名；过期租约接管时优先恢复原文件。
    pub async fn setup_generate(
        &self,
        draft_id: &str,
        input: SetupKeyInput,
        key: &str,
    ) -> Result<SetupKeyView> {
        if !valid_token(key)
            || input.organization.trim() != input.organization
            || input.organization.is_empty()
            || input.organization.len() > 128
            || input.organization.chars().any(char::is_control)
        {
            return Err(validation());
        }
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let mut state = Initialization::get_by_id(&mut tx, &1).await?;
        require_draft(&state, draft_id, None)?;
        if let Some(current) = state.draft.key_id.as_ref()
            && current != key
        {
            return Err(Error::new(
                409,
                "KEY_ALREADY_PREPARED",
                "已有签发身份生成操作，请继续原操作",
            ));
        }
        let owner = token();
        if let Some(mut candidate) = InitializationKey::filter_by_id(key.to_owned())
            .first()
            .exec(&mut tx)
            .await?
        {
            if candidate.draft_id != draft_id || candidate.organization != input.organization {
                return Err(revision());
            }
            if candidate.status == "READY" || candidate.lease_until > now() {
                return key_view(candidate).await;
            }
            candidate
                .update()
                .status("PENDING")
                .owner(owner.clone())
                .lease_until(now() + 120)
                .exec(&mut tx)
                .await?;
        } else {
            require_draft(&state, draft_id, Some(input.expected_revision))?;
            if state.draft.password_hash.is_empty() || state.draft.service.is_none() {
                return Err(validation());
            }
            toasty::create!(InitializationKey {
                id: key.to_owned(),
                draft_id: draft_id.to_owned(),
                organization: input.organization.clone(),
                status: "PENDING",
                owner: owner.clone(),
                lease_until: now() + 120,
                public_json: None::<String>
            })
            .exec(&mut tx)
            .await?;
            let mut data = state.draft.0.clone();
            data.key_id = Some(key.into());
            let rev = state.revision + 1;
            state
                .update()
                .draft(data)
                .revision(rev)
                .draft_expires_at(now() + 86400)
                .exec(&mut tx)
                .await?;
        }
        tx.commit().await?;
        let config = self.config.clone();
        let id = key.to_owned();
        let organization = input.organization;
        let result = tokio::task::spawn_blocking(move || {
            key_store::generate_or_load(&config, &id, &organization).and_then(|issuer| {
                String::from_utf8(wire::to_json(issuer.public()).map_err(|_| Error::unavailable())?)
                    .map_err(|_| Error::unavailable())
            })
        })
        .await
        .map_err(|_| Error::unavailable())?;
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let state = Initialization::get_by_id(&mut tx, &1).await?;
        require_draft(&state, draft_id, None)?;
        let mut candidate = InitializationKey::get_by_id(&mut tx, &key.to_owned()).await?;
        if candidate.owner != owner {
            return Err(revision());
        }
        let error = result.as_ref().err().map(|_| {
            Error::new(
                503,
                "KEY_STORE_UNAVAILABLE",
                "密钥尚未安全保存，请检查存储并重试原操作",
            )
        });
        candidate
            .update()
            .status(if result.is_ok() { "READY" } else { "FAILED" })
            .lease_until(0)
            .public_json(result.ok())
            .exec(&mut tx)
            .await?;
        let view = key_view(candidate).await?;
        tx.commit().await?;
        if let Some(error) = error {
            return Err(error);
        }
        Ok(view)
    }

    pub async fn setup_key(&self, draft_id: &str, id: &str) -> Result<SetupKeyView> {
        self.setup_draft(draft_id)
            .await?
            .key
            .filter(|k| k.operation_id == id)
            .ok_or_else(Error::not_found)
    }

    /// 提交前设置只读回执 Cookie，响应丢失后仍能核对同一次事务结果。
    pub async fn setup_begin_submission(
        &self,
        draft_id: &str,
        id: &str,
    ) -> Result<(SetupOperation, String)> {
        if !valid_token(id) {
            return Err(Error::invalid());
        }
        let secret = self.setup_mac("receipt", id.as_bytes())?;
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let state = Initialization::get_by_id(&mut tx, &1).await?;
        require_draft(&state, draft_id, None)?;
        let row = if let Some(row) = InitializationSubmission::filter_by_id(id.to_owned())
            .first()
            .exec(&mut tx)
            .await?
        {
            if row.draft_id != draft_id || row.expires_at <= now() {
                return Err(Error::not_found());
            }
            row
        } else {
            toasty::create!(InitializationSubmission {
                id: id.to_owned(),
                draft_id: draft_id.to_owned(),
                receipt_hash: digest(&secret),
                expires_at: now() + 600,
                content_digest: "",
                result: None::<SetupReceipt>
            })
            .exec(&mut tx)
            .await?
        };
        tx.commit().await?;
        Ok((
            SetupOperation {
                operation_id: row.id,
                completed: row.result.is_some(),
                result: row.result.0,
            },
            format!("{id}.{secret}"),
        ))
    }

    pub async fn setup_operation(&self, id: &str, cookie: &str) -> Result<SetupOperation> {
        let (cookie_id, secret) = cookie.split_once('.').ok_or_else(Error::not_found)?;
        if (id != "current" && cookie_id != id) || !valid_token(cookie_id) || !valid_token(secret) {
            return Err(Error::not_found());
        }
        let row =
            InitializationSubmission::get_by_id(&mut self.store.database(), &cookie_id.to_owned())
                .await?;
        if row.expires_at <= now() || !equal(&row.receipt_hash, &digest(secret)) {
            return Err(Error::not_found());
        }
        Ok(SetupOperation {
            operation_id: row.id,
            completed: row.result.is_some(),
            result: row.result.0,
        })
    }

    pub async fn setup_complete(
        &self,
        draft_id: &str,
        input: SetupCompleteInput,
    ) -> Result<SetupReceipt> {
        if !input.identity_confirmed || input.draft_id != draft_id {
            return Err(validation());
        }
        let hash = self.setup_mac("complete", &serde_json::to_vec(&input)?)?;
        let config = self.config.clone();
        let key = input.key_id.clone();
        let issuer = tokio::task::spawn_blocking(move || key_store::load(&config, &key))
            .await
            .map_err(|_| Error::unavailable())??;
        let public =
            String::from_utf8(wire::to_json(issuer.public()).map_err(|_| Error::unavailable())?)
                .map_err(|_| Error::unavailable())?;
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let mut state = Initialization::get_by_id(&mut tx, &1).await?;
        let mut op = InitializationSubmission::get_by_id(&mut tx, &input.operation_id).await?;
        if op.draft_id != draft_id || op.expires_at <= now() {
            return Err(Error::not_found());
        }
        if let Some(result) = op.result.0.clone() {
            if !equal(&op.content_digest, &hash) {
                return Err(revision());
            }
            return Ok(result);
        }
        require_draft(&state, draft_id, Some(input.expected_revision))?;
        let data = state.draft.0.clone();
        if data.username.is_empty()
            || data.password_hash.is_empty()
            || data.key_id.as_deref() != Some(input.key_id.as_str())
        {
            return Err(validation());
        }
        let candidate = InitializationKey::get_by_id(&mut tx, &input.key_id).await?;
        if candidate.status != "READY"
            || candidate.draft_id != draft_id
            || candidate.public_json.as_deref() != Some(&public)
        {
            return Err(revision());
        }
        let service = data.service.ok_or_else(validation)?;
        let result = SetupReceipt {
            username: data.username.clone(),
            service: service.clone(),
            organization: issuer.public().organization().into(),
            issuer_id: issuer.issuer_id().as_str().into(),
        };
        // 先竞争同一控制行，令并发提交在创建正式账号前以序列化冲突退出。
        // 此更新与后续记录处于同一事务；任何失败都会恢复未初始化状态。
        let mut cleaned = state.draft.0.clone();
        cleaned.password_hash.clear();
        state
            .update()
            .completed(true)
            .code_used(true)
            .code_hash("")
            .code_expires_at(0)
            .draft(cleaned)
            .exec(&mut tx)
            .await?;
        toasty::create!(Administrator {
            id: 1,
            username: data.username,
            password_hash: data.password_hash
        })
        .exec(&mut tx)
        .await?;
        toasty::create!(IssuerMetadata {
            id: 1,
            issuer_id: result.issuer_id.clone()
        })
        .exec(&mut tx)
        .await?;
        create_settings(&mut tx, &service).await?;
        op.update()
            .content_digest(hash)
            .result(Some(result.clone()))
            .exec(&mut tx)
            .await?;
        audit(
            &mut tx,
            "deployment-maintainer",
            "CENTER_INITIALIZED",
            &result.issuer_id,
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }
}
