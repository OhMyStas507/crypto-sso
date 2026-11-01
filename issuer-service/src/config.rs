//! 部署配置仅从进程环境读取，开发时可由仓库 .env 提供。
use anyhow::{Context, Result, ensure};
use std::{env, path::PathBuf};

pub fn required(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("missing environment variable {name}"))
}

pub fn number<T: std::str::FromStr>(name: &str) -> Result<T> {
    required(name)?
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid environment variable {name}"))
}

pub fn canonical_origin(value: &str) -> Result<String> {
    let url = url::Url::parse(value)?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    ensure!(
        (url.scheme() == "https" || (url.scheme() == "http" && loopback))
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none(),
        "invalid origin"
    );
    Ok(url.origin().ascii_serialization())
}

#[derive(Clone)]
pub struct Config {
    pub bind: String,
    pub database_url: String,
    pub admin_allowed_origins: Vec<String>,
    pub setup_dir: PathBuf,
    pub setup_storage_key: [u8; 32],
    pub session_ttl: u64,
    pub session_secret: String,
    pub csrf_key: [u8; 32],
    pub secure_cookie: bool,
    pub worker_interval_ms: u64,
    pub lease_seconds: u64,
    pub body_limit: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();
        let config = Self {
            bind: required("ISSUER_BIND")?,
            database_url: required("DATABASE_URL")?,
            admin_allowed_origins: serde_json::from_str(&required("ADMIN_ALLOWED_ORIGINS_JSON")?)?,
            setup_dir: env::var("SETUP_STORAGE_DIR")
                .unwrap_or_else(|_| ".dev-data/identity-center".into())
                .into(),
            setup_storage_key: hex::decode(required("SETUP_STORAGE_KEY")?)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("SETUP_STORAGE_KEY must be 32 bytes of hex"))?,
            session_ttl: number("ADMIN_SESSION_TTL_SECONDS")?,
            session_secret: required("ADMIN_SESSION_SECRET")?,
            csrf_key: hex::decode(required("ADMIN_CSRF_KEY")?)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("ADMIN_CSRF_KEY must be 32 bytes of hex"))?,
            secure_cookie: number("COOKIE_SECURE")?,
            worker_interval_ms: number("ISSUER_WORKER_INTERVAL_MS")?,
            lease_seconds: number("ISSUER_LEASE_SECONDS")?,
            body_limit: number("HTTP_BODY_LIMIT_BYTES")?,
        };
        ensure!(
            !config.admin_allowed_origins.is_empty(),
            "ADMIN_ALLOWED_ORIGINS_JSON cannot be empty"
        );
        for origin in &config.admin_allowed_origins {
            ensure!(
                canonical_origin(origin)? == *origin,
                "admin origins must be canonical"
            );
        }
        ensure!(
            config.session_ttl > 0 && config.worker_interval_ms > 0 && config.lease_seconds > 0,
            "timeouts must be positive"
        );
        ensure!(config.body_limit > 0, "HTTP body limit must be positive");
        ensure!(
            config.session_secret.len() >= 64,
            "ADMIN_SESSION_SECRET must contain at least 64 bytes"
        );
        Ok(config)
    }
}
