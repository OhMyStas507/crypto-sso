//! HTTP business contracts. SDK messages remain raw until the role's strict wire parser.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// 属性集合模型，以有序键值映射保存申请人声明的属性。
pub type Claims = BTreeMap<String, String>;

/// 对外仅公开中心是否需要初始化，不包含草稿或账号信息。
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SetupState {
    SetupRequired,
    SetupInProgress,
    Initialized,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct SetupStatus {
    pub state: SetupState,
}

/// 初始化中可编辑的公开服务信息，不包含部署秘密或业务开关。
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetupService {
    pub display_name: String,
    pub public_base_url: String,
    pub description: String,
    pub contact: String,
}

/// 候选签发身份的公开视图，不返回私钥、文件路径或解密材料。
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct SetupKeyView {
    pub operation_id: String,
    pub organization: String,
    pub status: String,
    pub issuer_id: Option<String>,
    #[ts(type = "Record<string, unknown> | null")]
    pub issuer_public: Option<serde_json::Value>,
}

/// 已验证初始化会话可读取的草稿摘要；密码只返回是否已设置。
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct SetupDraft {
    pub draft_id: String,
    pub revision: u32,
    pub expires_at: u64,
    pub username: String,
    pub password_configured: bool,
    pub service: Option<SetupService>,
    pub key: Option<SetupKeyView>,
}

/// 一次性初始化码仅用于部署权限验证，不作为管理员密码。
#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetupVerify {
    pub code: String,
}

/// 管理员草稿写入；密码原文只用于服务端即时计算哈希。
#[derive(Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetupAdminInput {
    pub expected_revision: u32,
    pub username: String,
    pub password: Option<String>,
}

/// 带期望修订的公开服务信息保存请求。
#[derive(Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetupServiceInput {
    pub expected_revision: u32,
    pub service: SetupService,
}

/// 生成操作绑定不可变的组织身份，重试时继续同一候选密钥。
#[derive(Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetupKeyInput {
    pub expected_revision: u32,
    pub organization: String,
}

/// 最终确认只引用已保存的草稿和密钥，不再次传输密码或私钥。
#[derive(Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetupCompleteInput {
    pub operation_id: String,
    pub draft_id: String,
    pub expected_revision: u32,
    pub key_id: String,
    pub identity_confirmed: bool,
}

/// 初始化提交成功后的有限摘要，不能用于调用管理接口。
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct SetupReceipt {
    pub username: String,
    pub service: SetupService,
    pub organization: String,
    pub issuer_id: String,
}

/// 只读回执用于恢复结果未知的最终提交，不代表管理员会话。
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct SetupOperation {
    pub operation_id: String,
    pub completed: bool,
    pub result: Option<SetupReceipt>,
}

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
