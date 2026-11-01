use crypto_sso_identity_api::{ClaimHint, ServiceSettings};
use toasty::Json;

/// 签发中心身份模型，固定数据库所属的签发者，防止误用其他密钥。
#[derive(toasty::Model)]
#[table = "issuer_metadata"]
pub struct IssuerMetadata {
    #[key]
    pub id: i32,
    pub issuer_id: String,
}

/// 服务配置模型，持久化开放状态、审核期限和属性提示等可修订设置。
#[derive(toasty::Model)]
#[table = "service_settings"]
pub struct Settings {
    #[key]
    pub id: i32,
    pub revision: u32,
    pub claim_hints_revision: u32,
    pub display_name: String,
    pub description: String,
    pub contact: String,
    pub public_base_url: String,
    pub registration_enabled: bool,
    pub issuance_enabled: bool,
    pub review_ttl_seconds: u32,
    pub poll_interval_seconds: u32,
    pub updated_at: u64,
    #[column(type = jsonb)]
    pub claim_hints: Json<Vec<ClaimHint>>,
}

// A DTO conversion is independent of persistence: CRUD stays in Toasty's builders.
impl From<Settings> for ServiceSettings {
    fn from(value: Settings) -> Self {
        Self {
            revision: value.revision,
            claim_hints_revision: value.claim_hints_revision,
            display_name: value.display_name,
            description: value.description,
            contact: value.contact,
            public_base_url: value.public_base_url,
            registration_enabled: value.registration_enabled,
            issuance_enabled: value.issuance_enabled,
            review_ttl_seconds: value.review_ttl_seconds,
            poll_interval_seconds: value.poll_interval_seconds,
            updated_at: value.updated_at,
            claim_hints: value.claim_hints.0,
        }
    }
}

/// 属性提示版本模型，保留申请提交时所引用的历史提示。
#[derive(toasty::Model)]
#[table = "claim_hint_revisions"]
pub struct ClaimHintRevision {
    #[key]
    pub revision: u32,
    #[column(type = jsonb)]
    pub hints: Json<Vec<ClaimHint>>,
}
