use serde::Serialize;

/// 幂等记录模型，保存请求摘要及历史响应，保证重复操作结果一致。
#[derive(toasty::Model)]
#[table = "operation_dedup"]
pub struct Idempotency {
    #[key]
    pub key: String,
    pub digest: String,
    #[column(type = jsonb)]
    pub result: serde_json::Value,
}

/// 审计事件模型，持久化操作主体、动作、目标与发生时间。
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
