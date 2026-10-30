//! HTTP business contracts. SDK messages remain raw until the role's strict wire parser.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

pub type Claims = BTreeMap<String, String>;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ClaimHint {
    pub key: String,
    pub label: String,
    pub description: String,
    pub default_value: Option<String>,
    pub example_value: Option<String>,
    pub include_by_default: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ServiceSettings {
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
    pub claim_hints: Vec<ClaimHint>,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ClaimsLimits {
    pub max_keys: u32,
    pub max_key_bytes: u32,
    pub max_value_bytes: u32,
}

#[derive(Clone, Serialize, Deserialize, TS)]
pub struct ServiceInfo {
    pub api_version: String,
    pub issuer_id: String,
    #[ts(type = "unknown")]
    pub issuer_public: Box<serde_json::value::RawValue>,
    pub settings: ServiceSettings,
    pub effective_claims_limits: ClaimsLimits,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnrollmentStatus {
    PendingReview,
    ChangesRequired,
    Rejected,
    Cancelled,
    Expired,
    Approved,
    Signing,
    Issued,
    IssueFailed,
    InvalidRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct EnrollmentView {
    pub request_id: String,
    pub request_hash: String,
    pub status: EnrollmentStatus,
    pub revision: u32,
    pub submitted_at: u64,
    pub review_deadline: u64,
    pub public_reason: Option<String>,
    pub poll_interval_seconds: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitEnrollment {
    pub enrollment_request: Box<serde_json::value::RawValue>,
    pub claim_hints_revision: Option<u32>,
    pub application_note: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecision {
    pub expected_revision: u32,
    pub request_hash: String,
    pub member_ref: Option<String>,
    pub evidence_source: Option<String>,
    pub verification_note: Option<String>,
    pub internal_note: Option<String>,
    pub approved_claims: Option<Claims>,
    pub public_reason: Option<String>,
    pub claims_confirmed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct AuditEvent {
    pub id: String,
    pub at: u64,
    pub actor: String,
    pub action: String,
    pub target: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub trace_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub total: usize,
}
