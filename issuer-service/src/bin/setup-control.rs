//! 部署终端轮换初始化码；码值由后端日志输出。
use issuer_service::{config::Config, viewmodels::Application};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    crypto_sso_telemetry::init()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        args.len() <= 1
            && args.iter().all(|arg| matches!(
                arg.as_str(),
                "--discard-expired-draft" | "--cleanup-expired-keys"
            )),
        "usage: setup-control [--discard-expired-draft | --cleanup-expired-keys]"
    );
    let app = Application::bootstrap(Config::from_env()?).await?;
    if args.iter().any(|a| a == "--cleanup-expired-keys") {
        println!("已回收候选记录：{}", app.cleanup_setup_keys().await?);
        return Ok(());
    }
    app.ensure_setup_code(true, args.iter().any(|a| a == "--discard-expired-draft"))
        .await?;
    Ok(())
}
