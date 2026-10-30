//! Deployment configuration is read only from the process environment (.env in development).
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
    pub public_origin: String,
    pub admin_allowed_origins: Vec<String>,
    pub issuer_secret_path: PathBuf,
    pub issuer_public_path: PathBuf,
    pub admin_username: String,
    pub admin_password_hash: String,
    pub session_ttl: u64,
    pub session_secret: String,
    pub csrf_key: [u8; 32],
    pub secure_cookie: bool,
    pub worker_interval_ms: u64,
    pub lease_seconds: u64,
    pub body_limit: usize,
    pub rate_limit_per_minute: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();
        let config = Self {
            bind: required("ISSUER_BIND")?,
            database_url: required("DATABASE_URL")?,
            public_origin: required("ISSUER_PUBLIC_ORIGIN")?,
            admin_allowed_origins: serde_json::from_str(&required("ADMIN_ALLOWED_ORIGINS_JSON")?)?,
            issuer_secret_path: required("ISSUER_SECRET_PATH")?.into(),
            issuer_public_path: required("ISSUER_PUBLIC_PATH")?.into(),
            admin_username: required("ADMIN_USERNAME")?,
            admin_password_hash: required("ADMIN_PASSWORD_HASH")?,
            session_ttl: number("ADMIN_SESSION_TTL_SECONDS")?,
            session_secret: required("ADMIN_SESSION_SECRET")?,
            csrf_key: hex::decode(required("ADMIN_CSRF_KEY")?)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("ADMIN_CSRF_KEY must be 32 bytes of hex"))?,
            secure_cookie: number("COOKIE_SECURE")?,
            worker_interval_ms: number("ISSUER_WORKER_INTERVAL_MS")?,
            lease_seconds: number("ISSUER_LEASE_SECONDS")?,
            body_limit: number("HTTP_BODY_LIMIT_BYTES")?,
            rate_limit_per_minute: number("HTTP_RATE_LIMIT_PER_MINUTE")?,
        };
        ensure!(
            canonical_origin(&config.public_origin)? == config.public_origin,
            "ISSUER_PUBLIC_ORIGIN must be canonical"
        );
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
        ensure!(
            config.body_limit > 0 && config.rate_limit_per_minute > 0,
            "HTTP limits must be positive"
        );
        ensure!(
            config.session_secret.len() >= 64,
            "ADMIN_SESSION_SECRET must contain at least 64 bytes"
        );
        argon2::PasswordHash::new(&config.admin_password_hash)
            .map_err(|_| anyhow::anyhow!("invalid ADMIN_PASSWORD_HASH"))?;
        Ok(config)
    }
}
