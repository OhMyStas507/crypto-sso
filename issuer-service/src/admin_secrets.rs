//! 管理会话与 CSRF 密钥只在首次使用时生成，由 Redis 供所有实例共享。
use crate::redis_store::RedisStore;
use anyhow::{Context, Result};
use rand::{RngCore, rngs::OsRng};

pub(crate) struct AdminSecrets {
    pub session_secret: [u8; 64],
    pub csrf_key: [u8; 32],
}

impl AdminSecrets {
    pub async fn load(redis: &RedisStore) -> Result<Self> {
        Self::load_from(redis, "issuer_service").await
    }

    async fn load_from(redis: &RedisStore, prefix: &str) -> Result<Self> {
        Ok(Self {
            session_secret: load_or_generate(redis, &format!("{prefix}:admin_session_secret"))
                .await?,
            csrf_key: load_or_generate(redis, &format!("{prefix}:admin_csrf_key")).await?,
        })
    }
}

async fn load_or_generate<const N: usize>(redis: &RedisStore, key: &str) -> Result<[u8; N]> {
    let existing: Option<Vec<u8>> = redis
        .query(redis::cmd("GET").arg(key))
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
        let _: Option<String> = redis
            .query(
                redis::cmd("SET")
                    .arg(key)
                    .arg(candidate.as_slice())
                    .arg("NX"),
            )
            .await
            .with_context(|| format!("cannot initialize Redis secret {key}"))?;
        // 无论本实例是否赢得写入，都读取 Redis 中的最终值。
        redis
            .query(redis::cmd("GET").arg(key))
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
        let url = std::env::var("TEST_REDIS_URL").expect("TEST_REDIS_URL is required");
        let redis = RedisStore::connect(&url).await.unwrap();
        let prefix = format!("issuer_service:test:{}", crate::security::token());
        let session_key = format!("{prefix}:admin_session_secret");
        let csrf_key = format!("{prefix}:admin_csrf_key");
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..16 {
            let url = url.clone();
            let prefix = prefix.clone();
            tasks.spawn(async move {
                let redis = RedisStore::connect(&url).await.unwrap();
                AdminSecrets::load_from(&redis, &prefix).await.unwrap()
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
        let restarted = RedisStore::connect(&url).await.unwrap();
        let reloaded = AdminSecrets::load_from(&restarted, &prefix).await.unwrap();
        assert!(reloaded.session_secret == first.session_secret);
        assert!(reloaded.csrf_key == first.csrf_key);
        for (key, expected) in [
            (&session_key, first.session_secret.as_slice()),
            (&csrf_key, first.csrf_key.as_slice()),
        ] {
            let stored: Vec<u8> = redis.query(redis::cmd("GET").arg(key)).await.unwrap();
            assert!(stored == expected);
            let ttl: i64 = redis.query(redis::cmd("TTL").arg(key)).await.unwrap();
            assert_eq!(ttl, -1);
        }

        // 损坏的密钥必须报错且保留原值，不能静默覆盖成新密钥。
        for key in [&csrf_key, &session_key] {
            let _: () = redis
                .query(redis::cmd("SET").arg(key).arg("invalid"))
                .await
                .unwrap();
            assert!(AdminSecrets::load_from(&restarted, &prefix).await.is_err());
            let stored: String = redis.query(redis::cmd("GET").arg(key)).await.unwrap();
            assert_eq!(stored, "invalid");
        }
        let _: usize = redis
            .query(redis::cmd("DEL").arg(&session_key).arg(&csrf_key))
            .await
            .unwrap();
    }
}
