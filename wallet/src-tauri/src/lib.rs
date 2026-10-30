pub mod dto;
pub mod engine;
mod model;
pub mod storage;
use engine::Engine;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::Emitter;

#[tauri::command]
async fn wallet_command(
    engine: tauri::State<'_, Arc<Engine>>,
    command: String,
    args: Value,
) -> Result<Value, String> {
    tracing::debug!("wallet command received");
    let result = engine.command(&command, args).await;
    if result.is_err() {
        tracing::warn!("wallet command rejected or failed");
    }
    result
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = dotenvy::dotenv();
    crypto_sso_telemetry::init().expect("cannot initialize wallet tracing");
    tracing::info!("starting wallet");
    let required = |name: &str| {
        std::env::var(name).unwrap_or_else(|_| panic!("missing environment variable {name}"))
    };
    // SAFETY: configured once before constructing wallet state or starting Tauri tasks.
    unsafe {
        crypto_sso_holder::set_claims_limits(crypto_sso_holder::ClaimsLimits {
            max_keys: required("SDK_MAX_CLAIMS")
                .parse()
                .expect("invalid SDK_MAX_CLAIMS"),
            max_key_bytes: required("SDK_MAX_KEY_BYTES")
                .parse()
                .expect("invalid SDK_MAX_KEY_BYTES"),
            max_value_bytes: required("SDK_MAX_VALUE_BYTES")
                .parse()
                .expect("invalid SDK_MAX_VALUE_BYTES"),
        })
        .expect("invalid SDK limits");
    }
    let engine = Arc::new(
        Engine::new(
            required("WALLET_DATA_DIR").into(),
            required("WALLET_HTTP_TIMEOUT_SECONDS")
                .parse()
                .expect("invalid WALLET_HTTP_TIMEOUT_SECONDS"),
            required("WALLET_HTTP_BODY_LIMIT_BYTES")
                .parse()
                .expect("invalid WALLET_HTTP_BODY_LIMIT_BYTES"),
        )
        .expect("cannot open protected wallet"),
    );
    let sync_interval: u64 = required("WALLET_SYNC_INTERVAL_SECONDS")
        .parse()
        .expect("invalid WALLET_SYNC_INTERVAL_SECONDS");
    assert!(sync_interval > 0);
    tauri::Builder::default()
        .manage(engine.clone())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut due: BTreeMap<String, (Instant, u32)> = BTreeMap::new();
                let mut previous_status = Value::Null;
                loop {
                    let status = engine.status();
                    if status != previous_status {
                        tracing::debug!(
                            unlocked = status["unlocked"].as_bool(),
                            "wallet lock state changed"
                        );
                        let _ = handle.emit("wallet://lock-changed", &status);
                        previous_status = status;
                    }
                    for (id, poll) in engine.pending_sync_ids() {
                        let entry = due.entry(id.clone()).or_insert((Instant::now(), 0));
                        if entry.0 > Instant::now() {
                            continue;
                        }
                        let result = engine.sync(&id).await;
                        let delay = match &result {
                            Ok(()) => {
                                tracing::trace!("wallet enrollment synchronized");
                                entry.1 = 0;
                                u64::from(poll)
                            }
                            Err(error) if error.starts_with("RATE_LIMITED:") => error
                                .split(':')
                                .nth(1)
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(60),
                            Err(_) => {
                                tracing::warn!("wallet synchronization failed; scheduling retry");
                                entry.1 = entry.1.saturating_add(1).min(6);
                                u64::from(poll).saturating_mul(1u64 << entry.1).min(300)
                            }
                        };
                        entry.0 = Instant::now() + Duration::from_secs(delay.max(1));
                        // Events invalidate public views; no claims, tokens or proof bodies are broadcast.
                        let _ = handle
                            .emit("wallet://enrollment-changed", serde_json::json!({"id":id}));
                    }
                    tokio::time::sleep(Duration::from_secs(sync_interval)).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![wallet_command])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
