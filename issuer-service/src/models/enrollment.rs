use crypto_sso_identity_api::{ClaimHint, Claims, EnrollmentView, ReviewDecision};
use serde::{Deserialize, Serialize};
use toasty::{Deferred, Json};

/// 申请聚合模型，保存申请内容、审核状态、签发租约和凭证结果。
#[derive(Clone, Serialize, toasty::Model)]
#[table = "enrollments"]
pub struct Enrollment {
    #[key]
    pub request_id: String,
    // Preserve the versioned public API state as a typed JSONB snapshot;
    // request secrets and signing state remain separate fields.
    #[column(type = jsonb)]
    pub view: Json<EnrollmentView>,
    pub raw_request: String,
    #[column(type = jsonb)]
    pub claims: Json<Claims>,
    pub metadata_hash: String,
    pub access_hash: String,
    pub claim_hints_revision: Option<u32>,
    #[column(type = jsonb)]
    pub hints_snapshot: Option<Json<Vec<ClaimHint>>>,
    pub application_note: Option<String>,
    #[column(type = jsonb)]
    pub decision: Option<Json<ReviewDecision>>,
    pub reviewer: Option<String>,
    pub reviewed_at: Option<u64>,
    pub blind_credential: Option<String>,
    pub issued_at: Option<u64>,
    pub generation: u32,
    pub lease_until: u64,
    pub failure_code: Option<String>,
    #[column(type = jsonb)]
    pub history: Json<Vec<ReviewHistory>>,
}

/// 审核历史模型，记录操作人、时间、动作及当次审核决定。
#[derive(Clone, Serialize, Deserialize)]
pub struct ReviewHistory {
    pub actor: String,
    pub at: u64,
    pub action: String,
    pub decision: ReviewDecision,
}

/// 成员模型，将已核验成员与申请关联，并保留唯一签发名额。
#[derive(Clone, Serialize, toasty::Model)]
#[table = "members"]
pub struct Member {
    #[key]
    pub member_ref: String,
    #[index]
    pub request_id: String,
    // Loading a public member must not implicitly serialize a private application.
    #[belongs_to(key = request_id, references = request_id)]
    #[serde(skip)]
    pub enrollment: Deferred<Enrollment>,
    pub evidence_source: String,
    pub verification_note: String,
    pub verified_at: u64,
    pub issued: bool,
}
