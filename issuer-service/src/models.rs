//! Typed Toasty models. Identifiers and scalar business fields have explicit
//! columns; structured snapshots use the framework's Json<T> support.
use crypto_sso_identity_api::{ClaimHint, Claims, EnrollmentView, ReviewDecision, ServiceSettings};
use serde::{Deserialize, Serialize};
use toasty::{Deferred, Json};

#[derive(toasty::Model)]
#[table = "issuer_metadata"]
pub struct IssuerMetadata {
    #[key]
    pub id: i32,
    pub issuer_id: String,
}

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

#[derive(toasty::Model)]
#[table = "claim_hint_revisions"]
pub struct ClaimHintRevision {
    #[key]
    pub revision: u32,
    #[column(type = jsonb)]
    pub hints: Json<Vec<ClaimHint>>,
}

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

#[derive(Clone, Serialize, Deserialize)]
pub struct ReviewHistory {
    pub actor: String,
    pub at: u64,
    pub action: String,
    pub decision: ReviewDecision,
}

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

#[derive(toasty::Model)]
#[table = "operation_dedup"]
pub struct Idempotency {
    #[key]
    pub key: String,
    pub digest: String,
    #[column(type = jsonb)]
    pub result: serde_json::Value,
}

#[derive(Serialize, toasty::Model)]
#[table = "audit_events"]
pub struct AuditRecord {
    #[key]
    pub id: String,
    pub at: u64,
    pub actor: String,
    pub action: String,
    pub target: String,
}

#[derive(toasty::Model)]
#[table = "web_sessions"]
pub struct WebSession {
    #[key]
    pub id: String,
    #[index]
    pub namespace: String,
    #[column(type = jsonb)]
    pub data: Json<salvo::session::Session>,
}

#[derive(toasty::Model)]
#[table = "revoked_sessions"]
pub struct RevokedSession {
    #[key]
    pub id: String,
}

#[derive(toasty::Model)]
#[table = "login_challenges"]
pub struct StoredChallenge {
    #[key]
    pub id: String,
    pub raw: String,
    pub consumed: bool,
}

#[derive(Serialize, toasty::Model)]
#[table = "application_sessions"]
pub struct ApplicationSession {
    #[key]
    #[serde(skip)]
    pub id: String,
    pub subject: String,
    pub issuer_id: String,
    pub origin: String,
    #[column(type = jsonb)]
    pub claims: Json<Claims>,
    pub expires_at: u64,
}
