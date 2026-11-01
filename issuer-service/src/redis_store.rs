//! Shared Redis connection and command execution. Construct once at startup;
//! clones share the multiplexed connection and its reconnect state.
use anyhow::{Context, Result};
use redis::{Cmd, FromRedisValue, RedisResult, aio::ConnectionManager};

#[derive(Clone)]
pub struct RedisStore {
    connection: ConnectionManager,
}

impl std::fmt::Debug for RedisStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RedisStore")
    }
}

impl RedisStore {
    #[tracing::instrument(skip_all)]
    pub async fn connect(url: &str) -> Result<Self> {
        let client = redis::Client::open(url).context("invalid REDIS_URL")?;
        let connection = ConnectionManager::new(client)
            .await
            .context("cannot connect to Redis")?;
        Ok(Self { connection })
    }

    /// Keep commands and their arguments out of logs: they may contain secrets.
    pub async fn query<T: FromRedisValue>(&self, command: &Cmd) -> RedisResult<T> {
        command.query_async(&mut self.connection.clone()).await
    }
}
