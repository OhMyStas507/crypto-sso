//! 领域校验、状态迁移与审计规则。
pub use crate::models::{Enrollment, Member, ReviewHistory};
use crate::{
    error::{Error, Result},
    models::AuditRecord,
    security,
};
use crypto_sso_identity_api::*;
use serde::{Deserialize, Serialize};

/// 管理员会话模型，保存登录身份、CSRF 令牌和绝对过期时间。
#[derive(Clone, Serialize, Deserialize)]
pub struct AdminSession {
    pub username: String,
    pub csrf_token: String,
    pub expires_at: u64,
}

pub async fn audit(
    tx: &mut impl toasty::Executor,
    actor: &str,
    action: &str,
    target: &str,
) -> Result<()> {
    toasty::create!(AuditRecord {
        id: security::token(),
        at: security::now(),
        actor: actor.to_owned(),
        action: action.to_owned(),
        target: target.to_owned(),
    })
    .exec(tx)
    .await?;
    Ok(())
}

pub fn validate_settings(s: &ServiceSettings) -> Result<()> {
    if s.display_name.trim().is_empty()
        || s.display_name.len() > 128
        || s.description.len() > 4096
        || s.contact.len() > 1024
        || !(60..=31_536_000).contains(&s.review_ttl_seconds)
        || !(1..=3600).contains(&s.poll_interval_seconds)
    {
        return Err(Error::invalid());
    }
    crate::config::canonical_origin(&s.public_base_url).map_err(|_| Error::invalid())?;
    // Constructing Claims checks duplicates, syntax and byte limits. Suggestions are never a whitelist.
    crypto_sso_issuer::Claims::new(
        s.claim_hints
            .iter()
            .map(|h| (&h.key, h.default_value.as_deref().unwrap_or(""))),
    )
    .map_err(|_| Error::invalid())?;
    for h in &s.claim_hints {
        if h.label.len() > 128 || h.description.len() > 2048 {
            return Err(Error::invalid());
        }
        if let Some(example) = &h.example_value {
            crypto_sso_issuer::Claims::new([(&h.key, example)]).map_err(|_| Error::invalid())?;
        }
    }
    Ok(())
}

pub fn require_revision(enrollment: &Enrollment, expected: u32) -> Result<()> {
    if enrollment.view.revision != expected {
        Err(Error::conflict())
    } else {
        Ok(())
    }
}

pub fn transition(enrollment: &mut Enrollment, status: EnrollmentStatus) {
    enrollment.view.status = status;
    enrollment.view.revision += 1;
}
