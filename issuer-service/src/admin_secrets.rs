//! 管理会话与 CSRF 密钥只在首次使用时生成，由 Redis 供所有实例共享。
use anyhow::{Context, Result};
use rand::{RngCore, rngs::OsRng};
use redis::aio::MultiplexedConnection;

pub(crate) struct AdminSecrets {
    pub session_secret: [u8; 64],
    pub csrf_key: [u8; 32],
}

impl AdminSecrets {
    pub async fn load(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url).context("invalid REDIS_URL")?;
        let mut connection = client
            .get_multiplexed_async_connection()
            .await
            .context("cannot connect to Redis for admin secrets")?;
        Self::load_from(&mut connection, "issuer_service").await
    }

    async fn load_from(connection: &mut MultiplexedConnection, prefix: &str) -> Result<Self> {
        Ok(Self {
            session_secret: load_or_generate(connection, &format!("{prefix}:admin_session_secret"))
                .await?,
            csrf_key: load_or_generate(connection, &format!("{prefix}:admin_csrf_key")).await?,
        })
    }
}

async fn load_or_generate<const N: usize>(
    connection: &mut MultiplexedConnection,
    key: &str,
) -> Result<[u8; N]> {
    let existing: Option<Vec<u8>> = redis::cmd("GET")
        .arg(key)
        .query_async(connection)
        .await
        .with_context(|| format!("cannot read Redis secret {key}"))?;
    let stored = if let Some(existing) = existing {
        existing
    } else {
        let mut candidate = [0u8; N];
        OsRng
            .try_fill_bytes(&mut candidate)
            .context("cannot generate admin secret")?;
        // NX 防止并发启动覆盖已有密钥；不设置过期时间。
        let _: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(candidate.as_slice())
            .arg("NX")
            .query_async(connection)
            .await
            .with_context(|| format!("cannot initialize Redis secret {key}"))?;
        // 无论本实例是否赢得写入，都读取 Redis 中的最终值。
        redis::cmd("GET")
            .arg(key)
            .query_async(connection)
            .await
            .with_context(|| format!("cannot reload Redis secret {key}"))?
    };
    stored
        .try_into()
        .map_err(|_| anyhow::anyhow!("Redis secret {key} must contain exactly {N} bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "requires TEST_REDIS_URL"]
    async fn concurrent_startup_reuses_persistent_secrets_and_rejects_corruption() {
        let _ = dotenvy::dotenv();
        let client = redis::Client::open(
            std::env::var("TEST_REDIS_URL").expect("TEST_REDIS_URL is required"),
        )
        .unwrap();
        let mut connection = client.get_multiplexed_async_connection().await.unwrap();
        let prefix = format!("issuer_service:test:{}", crate::security::token());
        let session_key = format!("{prefix}:admin_session_secret");
        let csrf_key = format!("{prefix}:admin_csrf_key");
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..16 {
            let client = client.clone();
            let prefix = prefix.clone();
            tasks.spawn(async move {
                let mut connection = client.get_multiplexed_async_connection().await.unwrap();
                AdminSecrets::load_from(&mut connection, &prefix)
                    .await
                    .unwrap()
            });
        }
        let first = tasks.join_next().await.unwrap().unwrap();
        assert!(first.session_secret != [0; 64]);
        assert!(first.csrf_key != [0; 32]);
        assert!(first.session_secret[..32] != first.csrf_key);
        while let Some(result) = tasks.join_next().await {
            let secrets = result.unwrap();
            assert!(secrets.session_secret == first.session_secret);
            assert!(secrets.csrf_key == first.csrf_key);
        }

        // 模拟重启，使用独立连接确认复用 Redis 中保存的值。
        let mut restarted = client.get_multiplexed_async_connection().await.unwrap();
        let reloaded = AdminSecrets::load_from(&mut restarted, &prefix)
            .await
            .unwrap();
        assert!(reloaded.session_secret == first.session_secret);
        assert!(reloaded.csrf_key == first.csrf_key);
        for (key, expected) in [
            (&session_key, first.session_secret.as_slice()),
            (&csrf_key, first.csrf_key.as_slice()),
        ] {
            let stored: Vec<u8> = redis::cmd("GET")
                .arg(key)
                .query_async(&mut connection)
                .await
                .unwrap();
            assert!(stored == expected);
            let ttl: i64 = redis::cmd("TTL")
                .arg(key)
                .query_async(&mut connection)
                .await
                .unwrap();
            assert_eq!(ttl, -1);
        }

        // 损坏的密钥必须报错且保留原值，不能静默覆盖成新密钥。
        for key in [&csrf_key, &session_key] {
            let _: () = redis::cmd("SET")
                .arg(key)
                .arg("invalid")
                .query_async(&mut connection)
                .await
                .unwrap();
            assert!(
                AdminSecrets::load_from(&mut restarted, &prefix)
                    .await
                    .is_err()
            );
            let stored: String = redis::cmd("GET")
                .arg(key)
                .query_async(&mut connection)
                .await
                .unwrap();
            assert_eq!(stored, "invalid");
        }
        let _: usize = redis::cmd("DEL")
            .arg(&session_key)
            .arg(&csrf_key)
            .query_async(&mut connection)
            .await
            .unwrap();
    }
}
