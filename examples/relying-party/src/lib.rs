//! Independent verifier application. No login request calls the identity center.
use crypto_sso_verifier::{
    ClaimKeys, ClientBindingSecret, IssuerPublic, Origin, Presentation, Verifier, wire,
};
use issuer_service::{
    config::{number, required},
    error::{Error, Result},
    models::ApplicationSession,
    security::{digest, now, token},
    session_store::RedisSessionStore,
    store::{IsolationLevel, Store},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub struct App {
    pub verifier: Arc<Verifier>,
    pub store: Store,
    pub http_sessions: RedisSessionStore,
    pub required: ClaimKeys,
    pub policy: BTreeMap<String, String>,
    pub session_ttl: u64,
    pub body_limit: usize,
}

pub use issuer_service::models::StoredChallenge;

impl App {
    pub async fn from_env() -> anyhow::Result<Self> {
        let public: IssuerPublic =
            wire::from_json(&std::fs::read(required("RP_ISSUER_PUBLIC_PATH")?)?)?;
        let origin = Origin::parse(&required("RP_ORIGIN")?)?;
        let required_keys: Vec<String> =
            serde_json::from_str(&required("RP_REQUIRED_CLAIMS_JSON")?)?;
        Ok(Self {
            verifier: Arc::new(Verifier::new(public, origin)?),
            store: Store::connect(&required("RP_DATABASE_URL")?).await?,
            http_sessions: RedisSessionStore::connect(&required("REDIS_URL")?).await?,
            required: ClaimKeys::new(required_keys)?,
            policy: serde_json::from_str(&required("RP_CLAIM_POLICY_JSON")?)?,
            session_ttl: number("RP_SESSION_TTL_SECONDS")?,
            body_limit: number("HTTP_BODY_LIMIT_BYTES")?,
        })
    }

    #[tracing::instrument(skip_all)]
    pub async fn verify_and_consume(&self, raw: &[u8], binding: &str) -> Result<Value> {
        let proof: Presentation = wire::from_json(raw).map_err(|_| Error::invalid())?;
        let key = proof.challenge().nonce().as_str().to_owned();
        let stored = StoredChallenge::get_by_id(&mut self.store.database(), &key).await?;
        if stored.consumed {
            return Err(Error::conflict());
        }
        let challenge = wire::from_json(stored.raw.as_bytes()).map_err(|_| Error::invalid())?;
        let binding =
            ClientBindingSecret::from_cookie_value(binding).map_err(|_| Error::invalid())?;
        let verifier = self.verifier.clone();
        let verified = tokio::task::spawn_blocking(move || {
            verifier.verify_proof(&proof, &challenge, &binding, now())
        })
        .await
        .map_err(|_| Error::unavailable())?
        .map_err(|_| Error::new(401, "INVALID_PROOF", "证明无效、挑战过期或浏览器绑定不匹配"))?;
        let disclosed = verified.disclosed_claims();
        if self
            .policy
            .iter()
            .any(|(k, v)| disclosed.get(k) != Some(v.as_str()))
        {
            return Err(Error::new(
                403,
                "CLAIM_POLICY_REJECTED",
                "已验证属性不满足应用访问策略",
            ));
        }
        let subject = verified.subject().as_str().to_owned();
        let claims: BTreeMap<String, String> = disclosed
            .iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        let ticket = token();
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        // Verification ran outside the transaction. Recheck the exact challenge
        // before consuming it; concurrent submissions cannot both commit.
        let mut current = StoredChallenge::get_by_id(&mut tx, &key).await?;
        if current.consumed
            || current.raw != stored.raw
            || verified.challenge().expires_at() <= now()
        {
            return Err(Error::conflict());
        }
        current.update().consumed(true).exec(&mut tx).await?;
        // Challenge consumption and the actual application session commit together.
        let session = toasty::create!(ApplicationSession {
            id: digest(&ticket),
            subject,
            issuer_id: self.verifier.issuer_id().as_str().to_owned(),
            origin: self.verifier.origin().as_str().to_owned(),
            claims,
            expires_at: now() + self.session_ttl,
        })
        .exec(&mut tx)
        .await?;
        tx.commit().await?;
        tracing::info!("challenge consumed and application session committed");
        Ok(json!({"ticket":ticket,"session":session}))
    }
}

mod http;
pub use http::{router, service};
