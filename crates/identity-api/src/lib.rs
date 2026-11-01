//! HTTP business contracts. SDK messages remain raw until the role's strict wire parser.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// 属性集合模型，以有序键值映射保存申请人声明的属性。
pub type Claims = BTreeMap<String, String>;

/// 属性提示模型，描述建议属性及默认值、示例值。
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

/// 服务设置 ViewModel，承载配置读取与保存的完整接口数据。
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

/// 属性限制模型，声明属性数量及键值字节上限。
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ClaimsLimits {
    pub max_keys: u32,
    pub max_key_bytes: u32,
    pub max_value_bytes: u32,
}

/// 服务信息 ViewModel，提供中心身份、公钥、设置和有效属性限制。
#[derive(Clone, Serialize, Deserialize, TS)]
pub struct ServiceInfo {
    pub api_version: String,
    pub issuer_id: String,
    #[ts(type = "unknown")]
    pub issuer_public: Box<serde_json::value::RawValue>,
    pub settings: ServiceSettings,
    pub effective_claims_limits: ClaimsLimits,
}

/// 申请状态模型，定义审核及签发流程的所有公开状态。
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

/// 申请状态 ViewModel，向钱包公开版本、状态、期限和处理原因。
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

/// 提交申请 ViewModel，承载 SDK 申请原文、提示版本与申请备注。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitEnrollment {
    pub enrollment_request: Box<serde_json::value::RawValue>,
    pub claim_hints_revision: Option<u32>,
    pub application_note: Option<String>,
}

/// 审核决定 ViewModel，承载乐观锁版本、核验依据及批准属性。
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

/// 审计事件模型，描述操作主体、动作、目标与时间。
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct AuditEvent {
    pub id: String,
    pub at: u64,
    pub actor: String,
    pub action: String,
    pub target: String,
}

/// 接口错误 ViewModel，提供稳定错误码、中文消息与追踪标识。
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub trace_id: String,
}

/// 分页响应 ViewModel，以明确的元素类型返回结果、总数和后续游标。
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub total: usize,
}
