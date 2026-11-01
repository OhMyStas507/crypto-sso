//! 受限部署终端的初始化码轮换；只输出码文件位置，不打印码或私钥。
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
    let path = app
        .ensure_setup_code(true, args.iter().any(|a| a == "--discard-expired-draft"))
        .await?;
    println!("一次性初始化码文件：{}", path.display());
    Ok(())
}
