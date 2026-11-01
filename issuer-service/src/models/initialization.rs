use crypto_sso_identity_api::{SetupReceipt, SetupService};
use serde::{Deserialize, Serialize};
use toasty::Json;

/// 中心单例为所有初始化操作提供持久化的并发边界。
#[derive(toasty::Model)]
#[table = "center_initialization"]
pub struct Initialization {
    #[key]
    pub id: i32,
    pub completed: bool,
    pub revision: u32,
    pub draft_id: String,
    pub draft_expires_at: u64,
    pub code_hash: String,
    pub code_expires_at: u64,
    pub code_used: bool,
    #[column(type = jsonb)]
    pub draft: Json<InitializationDraft>,
}

/// 仅供服务端使用的草稿，禁止直接序列化为 HTTP 响应。
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct InitializationDraft {
    pub username: String,
    pub password_hash: String,
    pub service: Option<SetupService>,
    pub key_id: Option<String>,
}

/// 首位正式管理员，账号与密码哈希仅在最终完成事务中建立。
#[derive(toasty::Model)]
#[table = "administrator_accounts"]
pub struct Administrator {
    #[key]
    pub id: i32,
    pub username: String,
    pub password_hash: String,
}

/// 持久化租约防止不同进程同时为同一操作生成密钥。
#[derive(toasty::Model)]
#[table = "initialization_keys"]
pub struct InitializationKey {
    #[key]
    pub id: String,
    pub draft_id: String,
    pub organization: String,
    pub status: String,
    pub owner: String,
    pub lease_until: u64,
    pub public_json: Option<String>,
    /// SDK 私钥备份的十六进制编码，不额外加密，禁止导出到 HTTP 或日志。
    pub secret_hex: Option<String>,
}

/// 最终提交的只读回执和事务结果，凭据只保存校验摘要。
#[derive(toasty::Model)]
#[table = "initialization_submissions"]
pub struct InitializationSubmission {
    #[key]
    pub id: String,
    pub draft_id: String,
    pub receipt_hash: String,
    pub expires_at: u64,
    pub content_digest: String,
    #[column(type = jsonb)]
    pub result: Json<Option<SetupReceipt>>,
}
