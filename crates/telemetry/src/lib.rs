//! Shared tracing setup for executable entry points. Libraries emit events but
//! leave subscriber installation to the process that embeds them.
use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;

pub fn init() -> Result<()> {
    let directives = std::env::var("RUST_LOG").context("missing environment variable RUST_LOG")?;
    let filter = EnvFilter::builder()
        .with_regex(false)
        .parse(directives)
        .context("invalid RUST_LOG filter")?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_target(true)
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|_| anyhow::anyhow!("could not initialize tracing subscriber"))
}
