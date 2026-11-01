use crypto_sso_identity_api::Claims;
use serde::Serialize;
use toasty::Json;

/// 浏览器会话模型，按命名空间保存服务端会话数据。
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

/// 会话撤销模型，阻止已失效的会话记录被重新使用。
#[derive(toasty::Model)]
#[table = "revoked_sessions"]
pub struct RevokedSession {
    #[key]
    pub id: String,
}

/// 登录挑战模型，记录协议挑战及其消费状态，防止重放。
#[derive(toasty::Model)]
#[table = "login_challenges"]
pub struct StoredChallenge {
    #[key]
    pub id: String,
    pub raw: String,
    pub consumed: bool,
}

/// 应用会话模型，保存已验证主体、来源、属性及到期时间。
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
