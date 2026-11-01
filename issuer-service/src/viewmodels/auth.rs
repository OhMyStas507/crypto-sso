use super::{Application, contracts::LoginRequest};
use crate::{
    error::{Error, Result},
    models::domain::{AdminSession, audit},
    security::{equal, now},
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};

impl Application {
    /// 会话写入成功后记录登录审计，避免 HTTP View 直接编排业务持久化。
    pub async fn admin_session_record(&self, admin: &AdminSession) -> Result<()> {
        audit(
            &mut self.store.database(),
            &admin.username,
            "ADMIN_LOGIN",
            "session",
        )
        .await?;
        tracing::info!("administrator authenticated");
        Ok(())
    }

    /// 校验管理员凭据并生成会话模型；Cookie 与会话轮换由 View 适配。
    pub async fn admin_authenticate(
        &self,
        input: LoginRequest,
        csrf_token: String,
    ) -> Result<AdminSession> {
        let username = input.username;
        let password = zeroize::Zeroizing::new(input.password);
        let admin = crate::models::Administrator::get_by_id(&mut self.store.database(), &1).await?;
        let hash = admin.password_hash;
        let valid = tokio::task::spawn_blocking(move || {
            PasswordHash::new(&hash).is_ok_and(|h| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &h)
                    .is_ok()
            })
        })
        .await
        .unwrap_or(false);
        if !valid || !equal(&username, &admin.username) {
            return Err(Error::new(401, "INVALID_CREDENTIALS", "账号或密码错误"));
        }
        Ok(AdminSession {
            username,
            csrf_token,
            expires_at: now() + self.config.session_ttl,
        })
    }
}
