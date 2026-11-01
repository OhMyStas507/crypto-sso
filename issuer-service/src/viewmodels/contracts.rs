//! ViewModel 的输入、输出契约；只暴露接口需要的字段，不序列化持久化聚合。
use crate::models::{AuditRecord, Enrollment, Member, ReviewHistory};
use crypto_sso_identity_api::{
    ClaimHint, Claims, ClaimsLimits, EnrollmentView, ReviewDecision, ServiceSettings,
};
use serde::{Deserialize, Serialize};

/// 健康检查响应 ViewModel，表示服务进程可响应请求。
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

/// 登录请求 ViewModel，仅接收管理员账号与密码。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// CSRF 响应 ViewModel，向浏览器提供当前会话的请求校验令牌。
#[derive(Serialize, Deserialize)]
pub struct CsrfTokenResponse {
    pub csrf_token: Option<String>,
}

/// 注销响应 ViewModel，确认当前管理员会话已销毁。
#[derive(Serialize, Deserialize)]
pub struct LogoutResponse {
    pub logged_out: bool,
}

/// 取消申请请求 ViewModel，用期望版本防止覆盖并发审核结果。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelEnrollmentRequest {
    pub expected_revision: u32,
}

/// 属性提示响应 ViewModel，提供建议属性、提示版本与有效属性限制。
#[derive(Serialize, Deserialize)]
pub struct ClaimHintsResponse {
    pub api_version: String,
    pub issuer_id: String,
    pub revision: u32,
    pub updated_at: u64,
    pub mode: String,
    pub allow_custom_claims: bool,
    pub claims: Vec<ClaimHint>,
    pub effective_claims_limits: ClaimsLimits,
}

/// 凭证响应 ViewModel；SDK 协议内容保持原始 JSON，交由 SDK 严格解析。
#[derive(Serialize, Deserialize)]
pub struct CredentialResponse {
    pub blind_credential: Box<serde_json::value::RawValue>,
}

/// 管理端申请详情 ViewModel，显式选择可见字段，排除访问密钥摘要和签名原文。
#[derive(Clone, Serialize, Deserialize)]
pub struct AdminEnrollmentView {
    pub view: EnrollmentView,
    pub claims: Claims,
    pub application_note: Option<String>,
    pub claim_hints_revision: Option<u32>,
    pub hints_snapshot: Option<Vec<ClaimHint>>,
    pub decision: Option<ReviewDecision>,
    pub reviewer: Option<String>,
    pub reviewed_at: Option<u64>,
    pub issued_at: Option<u64>,
    pub failure_code: Option<String>,
    pub history: Vec<ReviewHistory>,
}

impl From<&Enrollment> for AdminEnrollmentView {
    fn from(e: &Enrollment) -> Self {
        Self {
            view: e.view.0.clone(),
            claims: e.claims.0.clone(),
            application_note: e.application_note.clone(),
            claim_hints_revision: e.claim_hints_revision,
            hints_snapshot: e.hints_snapshot.as_ref().map(|h| h.0.clone()),
            decision: e.decision.as_ref().map(|d| d.0.clone()),
            reviewer: e.reviewer.clone(),
            reviewed_at: e.reviewed_at,
            issued_at: e.issued_at,
            failure_code: e.failure_code.clone(),
            history: e.history.0.clone(),
        }
    }
}

/// 成员摘要 ViewModel，公开核验与签发状态，不携带数据库关联对象。
#[derive(Serialize, Deserialize)]
pub struct MemberView {
    pub member_ref: String,
    pub request_id: String,
    pub evidence_source: String,
    pub verification_note: String,
    pub verified_at: u64,
    pub issued: bool,
}

impl From<Member> for MemberView {
    fn from(m: Member) -> Self {
        Self {
            member_ref: m.member_ref,
            request_id: m.request_id,
            evidence_source: m.evidence_source,
            verification_note: m.verification_note,
            verified_at: m.verified_at,
            issued: m.issued,
        }
    }
}

/// 成员详情 ViewModel，将成员摘要与对应申请的管理端详情组合返回。
#[derive(Serialize, Deserialize)]
pub struct MemberDetailView {
    pub member: MemberView,
    pub enrollment: AdminEnrollmentView,
}

/// 审计事件 ViewModel，只输出审计展示字段，隔离持久化实体。
#[derive(Serialize, Deserialize)]
pub struct AuditEventView {
    pub id: String,
    pub at: u64,
    pub actor: String,
    pub action: String,
    pub target: String,
}

impl From<AuditRecord> for AuditEventView {
    fn from(e: AuditRecord) -> Self {
        Self {
            id: e.id,
            at: e.at,
            actor: e.actor,
            action: e.action,
            target: e.target,
        }
    }
}

/// 总览 ViewModel，汇总中心配置、各状态数量以及最近申请。
#[derive(Serialize, Deserialize)]
pub struct OverviewView {
    pub settings: ServiceSettings,
    pub issuer_id: String,
    pub key_available: bool,
    pub pending: usize,
    pub failed: usize,
    pub issued: usize,
    pub recent: Vec<AdminEnrollmentView>,
}

/// 列表查询 ViewModel，承载状态筛选、关键词与分页位置。
#[derive(Default)]
pub struct ListQuery {
    pub status: Option<String>,
    pub search: Option<String>,
    pub cursor: usize,
}

/// 审核动作 ViewModel，限定合法操作并保持审计与幂等摘要中的协议名称。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewAction {
    Approve,
    RequestChanges,
    Reject,
    RetryIssuance,
}

impl ReviewAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::RequestChanges => "request-changes",
            Self::Reject => "reject",
            Self::RetryIssuance => "retry-issuance",
        }
    }
}
