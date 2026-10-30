//! Official Toasty schema CLI. Run from the workspace root so Toasty.toml is found.
use issuer_service::{config::required, store::connect_database};
use toasty_cli::{Config, ToastyCli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    crypto_sso_telemetry::init()?;
    tracing::info!(target: "issuer_service::schema", "starting database schema command");
    let config = Config::load()?;
    let db = connect_database(&required("DATABASE_URL")?).await?;
    ToastyCli::with_config(db, config).parse_and_run().await
}
