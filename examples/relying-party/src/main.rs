use issuer_service::config::{number, required};
use salvo::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    crypto_sso_telemetry::init()?;
    tracing::info!("starting relying party");
    // SAFETY: process startup, before constructing the verifier or accepting requests.
    unsafe {
        crypto_sso_verifier::set_claims_limits(crypto_sso_verifier::ClaimsLimits {
            max_keys: number("SDK_MAX_CLAIMS")?,
            max_key_bytes: number("SDK_MAX_KEY_BYTES")?,
            max_value_bytes: number("SDK_MAX_VALUE_BYTES")?,
        })?;
    }
    let app = crypto_sso_relying_party::App::from_env().await?;
    let secret = required("RP_SESSION_SECRET")?;
    anyhow::ensure!(
        secret.len() >= 64,
        "RP_SESSION_SECRET must contain at least 64 bytes"
    );
    let csrf_key = hex::decode(required("RP_CSRF_KEY")?)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("RP_CSRF_KEY must be 32 hex bytes"))?;
    let service = crypto_sso_relying_party::service(
        app,
        secret.as_bytes(),
        csrf_key,
        number("COOKIE_SECURE")?,
    );
    let bind = required("RP_BIND")?;
    let server = Server::new(TcpListener::new(bind.clone()).bind().await);
    tracing::info!(%bind, "relying party ready");
    let handle = server.handle();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("relying party shutdown requested");
        handle.stop_graceful(None);
    });
    server.serve(service).await;
    tracing::info!("relying party stopped");
    Ok(())
}
