//! Redis persistence for Salvo HTTP sessions. Salvo owns cookies and expiry;
//! Redis expires records and atomically refuses updates to deleted sessions.
use crate::error::Error;
use anyhow::{Context, ensure};
use redis::aio::ConnectionManager;
use salvo::{
    async_trait,
    session::{Session, SessionStore},
};

#[derive(Clone)]
pub struct RedisSessionStore {
    connection: ConnectionManager,
    prefix: String,
}

impl std::fmt::Debug for RedisSessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RedisSessionStore")
    }
}

impl RedisSessionStore {
    pub async fn connect(redis_url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url).context("invalid REDIS_URL")?;
        let connection = ConnectionManager::new(client)
            .await
            .context("cannot connect to Redis for HTTP sessions")?;
        Ok(Self {
            connection,
            prefix: format!("issuer_service:http_sessions:{}:", hex::encode("session")),
        })
    }

    /// Namespaces share a reconnecting connection, but never each other's keys.
    pub fn with_namespace(&self, namespace: &str) -> Self {
        // Encode arbitrary names so glob metacharacters and separators cannot
        // escape the namespace used by clear_store.
        Self {
            connection: self.connection.clone(),
            prefix: format!("issuer_service:http_sessions:{}:", hex::encode(namespace)),
        }
    }

    fn key(&self, id: &str) -> String {
        format!("{}{id}", self.prefix)
    }
}

#[async_trait]
impl SessionStore for RedisSessionStore {
    #[tracing::instrument(skip_all)]
    async fn load_session(&self, cookie_value: String) -> saysion::Result<Option<Session>> {
        let id = Session::id_from_cookie_value(&cookie_value)?;
        let data: Option<String> = redis::cmd("GET")
            .arg(self.key(&id))
            .query_async(&mut self.connection.clone())
            .await?;
        let Some(data) = data else {
            return Ok(None);
        };
        let session: Session = serde_json::from_str(&data)?;
        ensure!(session.id() == id, "Redis session ID mismatch");
        Ok(session.validate())
    }

    #[tracing::instrument(skip_all)]
    async fn store_session(&self, session: Session) -> saysion::Result<Option<String>> {
        // All HTTP handlers configure a finite TTL. Refuse immortal records.
        let expiry = session
            .expiry()
            .context("HTTP session requires an expiry")?;
        ensure!(
            !session.is_expired(),
            "cannot store an expired HTTP session"
        );
        let expires_at_ms = expiry.unix_timestamp_nanos().div_euclid(1_000_000);
        let key = self.key(session.id());
        let data = serde_json::to_string(&session)?;
        // Only a fresh or regenerated session owns a cookie value. Loaded
        // sessions use XX: logout, rotation, expiry and clear_store delete the
        // key, so a late request cannot recreate it. NX fences new IDs.
        let cookie_value = session.into_cookie_value();
        let saved: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(data)
            .arg("PXAT")
            .arg(expires_at_ms.to_string())
            .arg(if cookie_value.is_some() { "NX" } else { "XX" })
            .query_async(&mut self.connection.clone())
            .await?;
        if saved.is_none() {
            tracing::debug!("refused to restore a missing or replaced session");
            return Err(Error::conflict().into());
        }
        Ok(cookie_value)
    }

    #[tracing::instrument(skip_all)]
    async fn destroy_session(&self, session: Session) -> saysion::Result {
        let _: usize = redis::cmd("DEL")
            .arg(self.key(session.id()))
            .query_async(&mut self.connection.clone())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn clear_store(&self) -> saysion::Result {
        let mut connection = self.connection.clone();
        let mut cursor = 0_u64;
        loop {
            let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(format!("{}*", self.prefix))
                .arg("COUNT")
                .arg(100)
                .query_async(&mut connection)
                .await?;
            if !keys.is_empty() {
                let _: usize = redis::cmd("DEL")
                    .arg(keys)
                    .query_async(&mut connection)
                    .await?;
            }
            cursor = next;
            if cursor == 0 {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    async fn fixture() -> (String, RedisSessionStore) {
        let _ = dotenvy::dotenv();
        let url = std::env::var("TEST_REDIS_URL").expect("TEST_REDIS_URL is required");
        let store = RedisSessionStore::connect(&url)
            .await
            .unwrap()
            .with_namespace(&format!("test:{}", crate::security::token()));
        (url, store)
    }

    fn session(ttl: Duration) -> Session {
        let mut session = Session::new();
        session.expire_in(ttl);
        session.insert("admin", "tester").unwrap();
        session
    }

    #[tokio::test]
    #[ignore = "requires TEST_REDIS_URL"]
    async fn persistence_updates_ttl_and_namespace_isolation() {
        let (url, store) = fixture().await;
        let initial = session(Duration::from_secs(60));
        let key = store.key(initial.id());
        let cookie = store.store_session(initial).await.unwrap().unwrap();
        // A fresh connection, like another process or a restarted service.
        let mut restarted = RedisSessionStore::connect(&url).await.unwrap();
        restarted.prefix = store.prefix.clone();
        let mut loaded = restarted
            .load_session(cookie.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.get::<String>("admin").as_deref(), Some("tester"));
        assert!(!loaded.data_changed());
        let ttl: i64 = redis::cmd("PTTL")
            .arg(&key)
            .query_async(&mut store.connection.clone())
            .await
            .unwrap();
        assert!(ttl > 0 && ttl <= 60_000);
        loaded.insert("csrf", "token").unwrap();
        loaded.expire_in(Duration::from_secs(120));
        assert!(restarted.store_session(loaded).await.unwrap().is_none());
        assert_eq!(
            store
                .load_session(cookie.clone())
                .await
                .unwrap()
                .unwrap()
                .get::<String>("csrf")
                .as_deref(),
            Some("token")
        );
        let ttl: i64 = redis::cmd("PTTL")
            .arg(&key)
            .query_async(&mut store.connection.clone())
            .await
            .unwrap();
        assert!(ttl > 60_000 && ttl <= 120_000);

        let other = store.with_namespace(&format!("test:*:[{}]", crate::security::token()));
        assert!(other.load_session(cookie.clone()).await.unwrap().is_none());
        let other_cookie = other
            .store_session(session(Duration::from_secs(60)))
            .await
            .unwrap()
            .unwrap();
        store.clear_store().await.unwrap();
        assert!(store.load_session(cookie).await.unwrap().is_none());
        assert!(other.load_session(other_cookie).await.unwrap().is_some());
        other.clear_store().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "requires TEST_REDIS_URL"]
    async fn logout_rotation_and_clear_prevent_late_writes() {
        let (_, store) = fixture().await;
        for _ in 0..16 {
            let cookie = store
                .store_session(session(Duration::from_secs(60)))
                .await
                .unwrap()
                .unwrap();
            let mut stale = store.load_session(cookie.clone()).await.unwrap().unwrap();
            let revoked = store.load_session(cookie.clone()).await.unwrap().unwrap();
            stale.insert("admin", "late update").unwrap();
            // Either Redis command may win the race; the final state is absent.
            let (_, destroyed) =
                tokio::join!(store.store_session(stale), store.destroy_session(revoked));
            destroyed.unwrap();
            assert!(store.load_session(cookie).await.unwrap().is_none());
        }

        let old_cookie = store
            .store_session(session(Duration::from_secs(60)))
            .await
            .unwrap()
            .unwrap();
        let stale = store
            .load_session(old_cookie.clone())
            .await
            .unwrap()
            .unwrap();
        let mut rotated = store
            .load_session(old_cookie.clone())
            .await
            .unwrap()
            .unwrap();
        store.destroy_session(rotated.clone()).await.unwrap();
        rotated.regenerate();
        let new_cookie = store.store_session(rotated).await.unwrap().unwrap();
        assert_ne!(new_cookie, old_cookie);
        assert!(store.store_session(stale).await.is_err());
        assert!(store.load_session(old_cookie).await.unwrap().is_none());
        let stale = store
            .load_session(new_cookie.clone())
            .await
            .unwrap()
            .unwrap();
        store.clear_store().await.unwrap();
        assert!(store.store_session(stale).await.is_err());
        assert!(store.load_session(new_cookie).await.unwrap().is_none());
    }

    #[tokio::test]
    #[ignore = "requires TEST_REDIS_URL"]
    async fn expired_sessions_are_deleted_and_invalid_records_are_rejected() {
        let (_, store) = fixture().await;
        assert!(store.store_session(Session::new()).await.is_err());
        let mut expired = session(Duration::ZERO);
        expired.set_expiry(*expired.expiry().unwrap() - Duration::from_secs(1));
        assert!(store.store_session(expired).await.is_err());
        assert!(store.load_session("invalid cookie!".into()).await.is_err());

        let initial = session(Duration::from_millis(100));
        let key = store.key(initial.id());
        let cookie = store.store_session(initial).await.unwrap().unwrap();
        let mut stale = store.load_session(cookie.clone()).await.unwrap().unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
        let exists: bool = redis::cmd("EXISTS")
            .arg(&key)
            .query_async(&mut store.connection.clone())
            .await
            .unwrap();
        assert!(!exists);
        assert!(store.load_session(cookie.clone()).await.unwrap().is_none());
        // Even if a request refreshed its local TTL before the old key expired,
        // an update must not recreate an expired record.
        stale.expire_in(Duration::from_secs(60));
        assert!(store.store_session(stale).await.is_err());

        let _: () = redis::cmd("SET")
            .arg(&key)
            .arg("corrupt JSON")
            .query_async(&mut store.connection.clone())
            .await
            .unwrap();
        assert!(store.load_session(cookie.clone()).await.is_err());
        let _: () = redis::cmd("SET")
            .arg(&key)
            .arg(serde_json::to_string(&session(Duration::from_secs(60))).unwrap())
            .query_async(&mut store.connection.clone())
            .await
            .unwrap();
        assert!(store.load_session(cookie.clone()).await.is_err());
        let mut expired = session(Duration::ZERO);
        // A record's embedded expiry is checked even if Redis has no TTL.
        expired.set_expiry(*expired.expiry().unwrap() - Duration::from_secs(1));
        let expired_cookie = expired.clone();
        let expired_key = store.key(expired.id());
        let expired_cookie_value = expired.into_cookie_value().unwrap();
        let _: () = redis::cmd("SET")
            .arg(expired_key)
            .arg(serde_json::to_string(&expired_cookie).unwrap())
            .query_async(&mut store.connection.clone())
            .await
            .unwrap();
        assert!(
            store
                .load_session(expired_cookie_value)
                .await
                .unwrap()
                .is_none()
        );
        store.clear_store().await.unwrap();
    }
}
