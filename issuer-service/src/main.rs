use anyhow::Context;
use issuer_service::{config::Config, viewmodels::Application, views, worker};
use salvo::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    crypto_sso_telemetry::init()?;
    tracing::info!("starting identity center");
    let config = Config::from_env()?;
    // 安全性：仅在启动阶段设置限制，此时请求与签发任务尚未调用 SDK。
    unsafe {
        crypto_sso_issuer::set_claims_limits(crypto_sso_issuer::ClaimsLimits {
            max_keys: issuer_service::config::number("SDK_MAX_CLAIMS")?,
            max_key_bytes: issuer_service::config::number("SDK_MAX_KEY_BYTES")?,
            max_value_bytes: issuer_service::config::number("SDK_MAX_VALUE_BYTES")?,
        })?;
    }
    let bind = config.bind.clone();
    let app = Application::bootstrap(config).await?;
    let listener = TcpListener::new(bind.clone()).try_bind().await.with_context(|| format!("身份中心无法监听 {bind}。请检查端口占用；调整端口时同步修改 ISSUER_BIND 和 ISSUER_API_TARGET"))?;
    tracing::info!(%bind, "identity center ready");
    let worker = tokio::spawn(worker::run(app.clone()));
    let server = Server::new(listener);
    let handle = server.handle();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("identity center shutdown requested");
        handle.stop_graceful(None);
    });
    server.serve(views::service(app)).await;
    worker.abort();
    tracing::info!("identity center stopped");
    Ok(())
}
