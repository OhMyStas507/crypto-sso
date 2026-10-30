//! Wallet use cases. Every returned JSON value is an explicit public view, never a vault dump.
use crate::{model::*, storage::Vault};
use crypto_sso_holder::{
    BlindCredential, ClaimKeys, Claims, Holder, IssuerPublic, LoginChallenge, Origin,
    PendingEnrollment, wire,
};
use crypto_sso_identity_api::{EnrollmentStatus, EnrollmentView, ServiceInfo, SubmitEnrollment};
use rand::{RngCore, rngs::OsRng};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, String>;

struct Inner {
    vault: Vault,
    document: Option<Document>,
    epoch: u64,
    last_activity: Instant,
}

pub struct Engine {
    inner: Mutex<Inner>,
    client: reqwest::Client,
    body_limit: usize,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn token() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    hex::encode(b)
}
fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key].as_str().ok_or_else(|| "INVALID_INPUT".into())
}
fn raw<T: crypto_sso_holder::wire::WireMessage>(value: &T) -> Result<String> {
    String::from_utf8(wire::to_json(value).map_err(|_| "INVALID_SDK_MESSAGE")?)
        .map_err(|_| "INVALID_SDK_MESSAGE".into())
}
fn public(center: &Center) -> Result<IssuerPublic> {
    wire::from_json(center.info.issuer_public.get().as_bytes()).map_err(|_| "INVALID_ISSUER".into())
}

impl Engine {
    pub fn new(directory: PathBuf, timeout: u64, body_limit: usize) -> Result<Self> {
        if timeout == 0 || body_limit == 0 {
            return Err("INVALID_NETWORK_LIMITS".into());
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "NETWORK_UNAVAILABLE")?;
        Ok(Self {
            inner: Mutex::new(Inner {
                vault: Vault::open(directory)?,
                document: None,
                epoch: 0,
                last_activity: Instant::now(),
            }),
            client,
            body_limit,
        })
    }

    fn snapshot(&self, touch: bool) -> Result<(u64, Document)> {
        let mut inner = self.inner.lock().map_err(|_| "WALLET_UNAVAILABLE")?;
        if let Some(doc) = &inner.document
            && inner.last_activity.elapsed().as_secs() >= u64::from(doc.auto_lock_seconds)
        {
            inner.document = None;
            inner.vault.lock();
            inner.epoch += 1;
            tracing::info!("wallet automatically locked");
        }
        if touch {
            inner.last_activity = Instant::now();
        }
        Ok((
            inner.epoch,
            inner.document.as_ref().ok_or("WALLET_LOCKED")?.clone(),
        ))
    }

    fn update<T>(
        &self,
        epoch: u64,
        operation: impl FnOnce(&mut Document) -> Result<T>,
    ) -> Result<T> {
        let mut inner = self.inner.lock().map_err(|_| "WALLET_UNAVAILABLE")?;
        if inner.epoch != epoch || !inner.vault.unlocked() {
            return Err("WALLET_LOCKED".into());
        }
        let mut document = inner.document.as_ref().ok_or("WALLET_LOCKED")?.clone();
        let result = operation(&mut document)?;
        inner.vault.save(&document).inspect_err(|_| {
            tracing::error!("encrypted wallet write failed");
        })?;
        inner.document = Some(document);
        Ok(result)
    }

    pub fn status(&self) -> Value {
        let _ = self.snapshot(false);
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        json!(crate::dto::WalletStatus {
            initialized: inner.vault.initialized(),
            unlocked: inner.document.is_some(),
            epoch: inner.epoch,
            version: 1
        })
    }

    async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        access: Option<&str>,
        body: Option<&str>,
    ) -> Result<Vec<u8>> {
        let mut request = self.client.request(method, url);
        if let Some(access) = access {
            request = request.bearer_auth(access);
        }
        if let Some(body) = body {
            request = request
                .header("content-type", "application/json")
                .body(body.to_owned());
        }
        let mut response = request.send().await.map_err(|_| "NETWORK_UNAVAILABLE")?;
        let status = response.status();
        if status.as_u16() == 429 {
            let retry = response
                .headers()
                .get("retry-after")
                .and_then(|s| s.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(format!("RATE_LIMITED:{}", retry.max(1)));
        }
        if response
            .content_length()
            .is_some_and(|n| n > self.body_limit as u64)
        {
            return Err("RESPONSE_TOO_LARGE".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| "NETWORK_UNAVAILABLE")? {
            if bytes.len() + chunk.len() > self.body_limit {
                return Err("RESPONSE_TOO_LARGE".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            let error: Value = serde_json::from_slice(&bytes).unwrap_or_default();
            return Err(error["code"]
                .as_str()
                .filter(|s| s.len() < 80 && s.bytes().all(|c| c.is_ascii_uppercase() || c == b'_'))
                .unwrap_or("CENTER_REQUEST_FAILED")
                .into());
        }
        Ok(bytes)
    }

    async fn probe(&self, url: &str) -> Result<ServiceInfo> {
        let origin = Origin::parse(url).map_err(|_| "INVALID_CENTER_URL")?;
        let bytes = self
            .request(
                reqwest::Method::GET,
                &format!("{}/api/v1/service-info", origin.as_str()),
                None,
                None,
            )
            .await?;
        let info: ServiceInfo =
            serde_json::from_slice(&bytes).map_err(|_| "INVALID_CENTER_RESPONSE")?;
        let issuer: IssuerPublic =
            wire::from_json(info.issuer_public.get().as_bytes()).map_err(|_| "INVALID_ISSUER")?;
        if issuer.issuer_id().map_err(|_| "INVALID_ISSUER")?.as_str() != info.issuer_id {
            return Err("ISSUER_MISMATCH".into());
        }
        Ok(info)
    }

    async fn check_center(&self, center: &Center) -> Result<ServiceInfo> {
        let info = self.probe(&center.base_url).await?;
        if info.issuer_id != center.info.issuer_id {
            return Err("ISSUER_MISMATCH".into());
        }
        Ok(info)
    }

    pub async fn command(&self, name: &str, args: Value) -> Result<Value> {
        let protected = !matches!(
            name,
            "get_wallet_status" | "initialize_wallet" | "unlock_wallet" | "lock_wallet"
        );
        let epoch = if protected {
            Some(self.snapshot(false)?.0)
        } else {
            None
        };
        let result = self.execute(name, args).await?;
        // Also fence read-only responses: a lock during an HTTP/CPU operation revokes its output.
        if let Some(epoch) = epoch
            && self.snapshot(false)?.0 != epoch
        {
            return Err("WALLET_LOCKED".into());
        }
        Ok(result)
    }

    async fn execute(&self, name: &str, args: Value) -> Result<Value> {
        match name {
            "get_wallet_status" => return Ok(self.status()),
            "initialize_wallet" => {
                let mut inner = self.inner.lock().map_err(|_| "WALLET_UNAVAILABLE")?;
                let doc = Document::default();
                inner.vault.initialize(&doc)?;
                inner.document = Some(doc);
                inner.epoch += 1;
                return Ok(json!({"unlocked":true}));
            }
            "unlock_wallet" => {
                let mut inner = self.inner.lock().map_err(|_| "WALLET_UNAVAILABLE")?;
                let mut doc: Document = inner.vault.unlock()?;
                if doc.version != 1 {
                    inner.vault.lock();
                    return Err("UNSUPPORTED_VAULT_VERSION".into());
                }
                // Consent is never restored after a restart/unlock. Importing a fresh challenge recreates the plan.
                for task in doc.tasks.values_mut() {
                    task.revision += 1;
                    task.state = "NEEDS_CHALLENGE".into();
                }
                inner.vault.save(&doc)?;
                inner.document = Some(doc);
                inner.last_activity = Instant::now();
                inner.epoch += 1;
                return Ok(json!({"unlocked":true}));
            }
            "lock_wallet" => {
                let mut inner = self.inner.lock().map_err(|_| "WALLET_UNAVAILABLE")?;
                inner.document = None;
                inner.vault.lock();
                inner.epoch += 1;
                return Ok(json!({"unlocked":false}));
            }
            _ => {}
        }
        // Query refreshes and background events are not user activity.
        let touch = !name.starts_with("get_") && !name.starts_with("list_");
        let (epoch, doc) = self.snapshot(touch)?;
        match name {
            "touch_wallet" => Ok(Value::Null),
            "probe_center" => Ok(
                serde_json::to_value(self.probe(field(&args, "url")?).await?)
                    .map_err(|_| "INVALID_CENTER_RESPONSE")?,
            ),
            "confirm_center" => {
                let origin =
                    Origin::parse(field(&args, "url")?).map_err(|_| "INVALID_CENTER_URL")?;
                let info = self.probe(origin.as_str()).await?;
                if info.issuer_id != field(&args, "expected_issuer_id")? {
                    return Err("ISSUER_MISMATCH".into());
                }
                let center = Center {
                    id: token(),
                    base_url: origin.as_str().into(),
                    info,
                };
                self.update(epoch, |doc| {
                    if doc
                        .centers
                        .values()
                        .any(|c| c.info.issuer_id == center.info.issuer_id)
                    {
                        return Err("CENTER_ALREADY_EXISTS".into());
                    }
                    doc.centers.insert(center.id.clone(), center.clone());
                    Ok(json!(center))
                })
            }
            "list_centers" => Ok(json!(doc.centers.values().collect::<Vec<_>>())),
            "get_center" => Ok(json!(
                doc.centers.get(field(&args, "id")?).ok_or("NOT_FOUND")?
            )),
            "refresh_claim_hints" => {
                let id = field(&args, "id")?;
                let center = doc.centers.get(id).ok_or("NOT_FOUND")?;
                let info = self.check_center(center).await?;
                self.update(epoch, |doc| {
                    let center = doc.centers.get_mut(id).ok_or("NOT_FOUND")?;
                    center.info = info;
                    Ok(json!(center))
                })
            }
            "create_enrollment_draft" => {
                let center_id = field(&args, "center_id")?;
                let center = doc.centers.get(center_id).ok_or("NOT_FOUND")?;
                let draft = Draft {
                    id: token(),
                    center_id: center_id.into(),
                    application_note: String::new(),
                    claim_hints_revision: Some(center.info.settings.claim_hints_revision),
                    rows: center
                        .info
                        .settings
                        .claim_hints
                        .iter()
                        .filter(|h| h.include_by_default)
                        .map(|h| (h.key.clone(), h.default_value.clone().unwrap_or_default()))
                        .collect(),
                };
                self.update(epoch, |doc| {
                    doc.drafts.insert(draft.id.clone(), draft.clone());
                    Ok(json!(draft))
                })
            }
            "get_enrollment_draft" => Ok(json!(
                doc.drafts.get(field(&args, "id")?).ok_or("NOT_FOUND")?
            )),
            "save_enrollment_draft" => {
                let draft: Draft = serde_json::from_value(args).map_err(|_| "INVALID_INPUT")?;
                if !doc.drafts.contains_key(&draft.id)
                    || !doc.centers.contains_key(&draft.center_id)
                    || draft.application_note.len() > 4096
                {
                    return Err("INVALID_INPUT".into());
                }
                Claims::new(draft.rows.clone()).map_err(|_| "INVALID_CLAIMS")?;
                self.update(epoch, |doc| {
                    doc.drafts.insert(draft.id.clone(), draft.clone());
                    Ok(json!(draft))
                })
            }
            "submit_enrollment" => {
                let draft_id = field(&args, "draft_id")?;
                let draft = doc.drafts.get(draft_id).ok_or("NOT_FOUND")?;
                let center = doc.centers.get(&draft.center_id).ok_or("NOT_FOUND")?;
                let current_info = self.check_center(center).await?;
                let claims = Claims::new(draft.rows.clone()).map_err(|_| "INVALID_CLAIMS")?;
                let limits = &current_info.effective_claims_limits;
                if claims.len() > limits.max_keys as usize
                    || claims.iter().any(|(k, v)| {
                        k.len() > limits.max_key_bytes as usize
                            || v.len() > limits.max_value_bytes as usize
                    })
                {
                    return Err("CENTER_CLAIMS_LIMIT_EXCEEDED".into());
                }
                let issuer = public(center)?;
                let claims_copy = claims.clone();
                let (pending, request) = tokio::task::spawn_blocking(move || {
                    Holder::begin_enrollment(&issuer, &claims_copy, &mut OsRng)
                })
                .await
                .map_err(|_| "CRYPTO_FAILED")?
                .map_err(|_| "INVALID_CLAIMS")?;
                let submission = serde_json::to_string(&SubmitEnrollment {
                    enrollment_request: serde_json::value::RawValue::from_string(raw(&request)?)
                        .map_err(|_| "INVALID_INPUT")?,
                    claim_hints_revision: draft.claim_hints_revision,
                    application_note: Some(draft.application_note.clone()),
                })
                .map_err(|_| "INVALID_INPUT")?;
                let entry = LocalEnrollment {
                    id: request.request_id().as_str().into(),
                    center_id: center.id.clone(),
                    pending: Some(Zeroizing::new(
                        String::from_utf8(
                            pending
                                .export_secret()
                                .map_err(|_| "CRYPTO_FAILED")?
                                .expose_secret()
                                .to_vec(),
                        )
                        .map_err(|_| "CRYPTO_FAILED")?,
                    )),
                    access: Zeroizing::new(token()),
                    submission,
                    claims: claims.iter().map(|(k, v)| (k.into(), v.into())).collect(),
                    view: None,
                    local_state: "SAVED_PENDING_SUBMISSION".into(),
                    response: None,
                    error: None,
                    synced_at: None,
                };
                // No HTTP submission until this durable commit succeeds.
                self.update(epoch, |doc| {
                    doc.enrollments.insert(entry.id.clone(), entry.clone());
                    doc.drafts.remove(draft_id);
                    Ok(())
                })?;
                let _ = self.sync(&entry.id).await;
                let (_, current) = self.snapshot(false)?;
                Ok(enrollment_view(
                    current.enrollments.get(&entry.id).ok_or("NOT_FOUND")?,
                ))
            }
            "list_enrollments" => Ok(json!(
                doc.enrollments
                    .values()
                    .map(enrollment_view)
                    .collect::<Vec<_>>()
            )),
            "get_enrollment" => Ok(enrollment_view(
                doc.enrollments
                    .get(field(&args, "id")?)
                    .ok_or("NOT_FOUND")?,
            )),
            "refresh_enrollment" | "retry_credential_save" => {
                let id = field(&args, "id")?;
                self.sync(id).await?;
                let (_, doc) = self.snapshot(false)?;
                Ok(enrollment_view(doc.enrollments.get(id).ok_or("NOT_FOUND")?))
            }
            "cancel_enrollment" => {
                let id = field(&args, "id")?;
                let entry = doc.enrollments.get(id).ok_or("NOT_FOUND")?;
                let center = doc.centers.get(&entry.center_id).ok_or("NOT_FOUND")?;
                self.check_center(center).await?;
                let view: EnrollmentView = serde_json::from_slice(&self.request(reqwest::Method::POST, &format!("{}/api/v1/enrollments/{id}/cancel", center.base_url), Some(&entry.access), Some(&json!({"expected_revision": entry.view.as_ref().ok_or("NOT_SUBMITTED")?.revision}).to_string())).await?).map_err(|_| "INVALID_CENTER_RESPONSE")?;
                self.update(epoch, |doc| {
                    let e = doc.enrollments.get_mut(id).ok_or("NOT_FOUND")?;
                    e.view = Some(view);
                    Ok(enrollment_view(e))
                })
            }
            "list_credentials" => Ok(json!(
                doc.credentials
                    .values()
                    .map(credential_view)
                    .collect::<Vec<_>>()
            )),
            "get_credential" => Ok(credential_view(
                doc.credentials
                    .get(field(&args, "id")?)
                    .ok_or("NOT_FOUND")?,
            )),
            "delete_credential" => self.update(epoch, |doc| {
                let id = field(&args, "id")?;
                doc.credentials.remove(id).ok_or("NOT_FOUND")?;
                if let Some(enrollment) = doc.enrollments.get_mut(id) {
                    enrollment.local_state = "CREDENTIAL_DELETED".into();
                }
                for task in doc.tasks.values_mut() {
                    task.revision += 1;
                    task.state = "NEEDS_CHALLENGE".into();
                }
                Ok(Value::Null)
            }),
            "list_trusted_apps" => Ok(json!(doc.apps.values().collect::<Vec<_>>())),
            "save_trusted_app" => {
                let origin =
                    Origin::parse(field(&args, "origin")?).map_err(|_| "INVALID_ORIGIN")?;
                let issuer = field(&args, "issuer_id")?;
                if !doc.centers.values().any(|c| c.info.issuer_id == issuer) {
                    return Err("UNTRUSTED_ISSUER".into());
                }
                let name = field(&args, "name")?;
                if name.trim().is_empty() || name.len() > 128 {
                    return Err("INVALID_INPUT".into());
                }
                let id = args["id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(token);
                let revision = doc.apps.get(&id).map_or(1, |a| a.revision + 1);
                let app = TrustedApp {
                    id,
                    name: name.into(),
                    origin: origin.as_str().into(),
                    issuer_id: issuer.into(),
                    revision,
                };
                self.update(epoch, |doc| {
                    for task in doc.tasks.values_mut().filter(|t| t.app_id == app.id) {
                        task.state = "NEEDS_CHALLENGE".into();
                        task.revision += 1;
                    }
                    doc.apps.insert(app.id.clone(), app.clone());
                    Ok(json!(app))
                })
            }
            "remove_trusted_app" => self.update(epoch, |doc| {
                let id = field(&args, "id")?;
                doc.apps.remove(id).ok_or("NOT_FOUND")?;
                doc.tasks.retain(|_, t| t.app_id != id);
                Ok(Value::Null)
            }),
            "list_login_tasks" => Ok(json!(doc.tasks.values().collect::<Vec<_>>())),
            "import_login_challenge" => {
                let app = doc
                    .apps
                    .get(field(&args, "app_id")?)
                    .ok_or("UNTRUSTED_APP")?;
                let input = field(&args, "challenge_json")?;
                let challenge: LoginChallenge =
                    wire::from_json(input.as_bytes()).map_err(|_| "INVALID_CHALLENGE")?;
                if challenge.origin().as_str() != app.origin
                    || challenge.issuer_id().as_str() != app.issuer_id
                {
                    return Err("CHALLENGE_CONTEXT_MISMATCH".into());
                }
                if challenge.expires_at() <= now() || challenge.issued_at() > now() + 30 {
                    return Err("CHALLENGE_EXPIRED".into());
                }
                let id = args["task_id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(token);
                if let Some(previous) = doc.tasks.get(&id)
                    && previous.app_id != app.id
                {
                    return Err("CHALLENGE_CONTEXT_MISMATCH".into());
                }
                let revision = doc.tasks.get(&id).map_or(1, |t| t.revision + 1);
                let task = LoginTask {
                    id,
                    app_id: app.id.clone(),
                    app_revision: app.revision,
                    revision,
                    challenge: raw(&challenge)?,
                    state: "REVIEW_DISCLOSURE".into(),
                };
                self.update(epoch, |doc| {
                    doc.tasks.insert(task.id.clone(), task.clone());
                    Ok(json!(task))
                })
            }
            "get_disclosure_plan" => disclosure_plan(&doc, field(&args, "id")?),
            "create_presentation" => {
                let id = field(&args, "id")?;
                let task = doc.tasks.get(id).ok_or("NOT_FOUND")?;
                if args["expected_revision"].as_u64() != Some(u64::from(task.revision))
                    || task.state != "REVIEW_DISCLOSURE"
                {
                    return Err("STALE_DISCLOSURE_PLAN".into());
                }
                let app = doc.apps.get(&task.app_id).ok_or("UNTRUSTED_APP")?;
                if app.revision != task.app_revision {
                    return Err("STALE_DISCLOSURE_PLAN".into());
                }
                let credential_id = field(&args, "credential_id")?;
                let credential = doc
                    .credentials
                    .get(credential_id)
                    .filter(|c| c.issuer_id == app.issuer_id)
                    .ok_or("NO_CREDENTIAL")?;
                let issuer = public(doc.centers.get(&credential.center_id).ok_or("NOT_FOUND")?)?;
                let holder = Holder::import_secret(credential.holder.as_bytes(), &issuer)
                    .map_err(|_| "CREDENTIAL_CORRUPT")?;
                let challenge: LoginChallenge =
                    wire::from_json(task.challenge.as_bytes()).map_err(|_| "INVALID_CHALLENGE")?;
                let origin = Origin::parse(&app.origin).map_err(|_| "INVALID_ORIGIN")?;
                let keys: Vec<String> = serde_json::from_value(args["disclose_keys"].clone())
                    .map_err(|_| "INVALID_INPUT")?;
                let keys = ClaimKeys::new(keys).map_err(|_| "INVALID_DISCLOSURE")?;
                let result = tokio::task::spawn_blocking(move || {
                    holder.present(&challenge, &origin, &keys, now(), &mut OsRng)
                })
                .await
                .map_err(|_| "CRYPTO_FAILED")?
                .map_err(|_| "INVALID_DISCLOSURE_OR_EXPIRED")?;
                // Check epoch and task revision again after CPU work; late results cannot escape a lock.
                self.update(epoch, |doc| {
                    let current = doc.tasks.get_mut(id).ok_or("NOT_FOUND")?;
                    if current.revision != task.revision {
                        return Err("STALE_DISCLOSURE_PLAN".into());
                    }
                    if !doc.credentials.contains_key(credential_id)
                        || doc
                            .apps
                            .get(&current.app_id)
                            .is_none_or(|a| a.revision != current.app_revision)
                    {
                        return Err("STALE_DISCLOSURE_PLAN".into());
                    }
                    let challenge: LoginChallenge = wire::from_json(current.challenge.as_bytes())
                        .map_err(|_| "INVALID_CHALLENGE")?;
                    if challenge.expires_at() <= now() {
                        return Err("CHALLENGE_EXPIRED".into());
                    }
                    current.revision += 1;
                    current.state = "PROOF_GENERATED".into();
                    Ok(json!({"presentation_json": raw(&result)?, "state":"PROOF_GENERATED"}))
                })
            }
            "refuse_disclosure" => self.update(epoch, |doc| {
                let task = doc.tasks.get_mut(field(&args, "id")?).ok_or("NOT_FOUND")?;
                task.state = "REFUSED".into();
                task.revision += 1;
                Ok(Value::Null)
            }),
            "get_preferences" => {
                Ok(json!({"auto_lock_seconds":doc.auto_lock_seconds, "version":doc.version}))
            }
            "save_preferences" => self.update(epoch, |doc| {
                let seconds = args["auto_lock_seconds"]
                    .as_u64()
                    .filter(|v| (30..=3600).contains(v))
                    .ok_or("INVALID_INPUT")?;
                doc.auto_lock_seconds = seconds as u32;
                Ok(json!({"auto_lock_seconds":seconds}))
            }),
            _ => Err("UNKNOWN_COMMAND".into()),
        }
    }

    pub async fn sync(&self, id: &str) -> Result<()> {
        let (epoch, doc) = self.snapshot(false)?;
        let entry = doc.enrollments.get(id).ok_or("NOT_FOUND")?;
        if entry.local_state == "CREDENTIAL_DELETED" {
            return Err("CREDENTIAL_DELETED".into());
        }
        if entry.local_state == "CREDENTIAL_READY" {
            if entry.pending.is_some() && doc.credentials.contains_key(id) {
                self.update(epoch, |doc| {
                    let e = doc.enrollments.get_mut(id).ok_or("NOT_FOUND")?;
                    e.pending = None;
                    e.response = None;
                    Ok(())
                })?;
            }
            return Ok(());
        }
        let center = doc.centers.get(&entry.center_id).ok_or("NOT_FOUND")?;
        // Once a response is durably saved, local verification and recovery work offline.
        if let Some(response) = &entry.response {
            return self
                .finish_response(epoch, center, entry, response.clone())
                .await;
        }
        if let Err(error) = self.check_center(center).await {
            self.record_sync_error(epoch, id, &error)?;
            return Err(error);
        }
        let status = if entry.view.is_none() {
            self.request(
                reqwest::Method::POST,
                &format!("{}/api/v1/enrollments", center.base_url),
                Some(&entry.access),
                Some(&entry.submission),
            )
            .await
        } else {
            self.request(
                reqwest::Method::GET,
                &format!("{}/api/v1/enrollments/{id}", center.base_url),
                Some(&entry.access),
                None,
            )
            .await
        };
        let bytes = match status {
            Ok(b) => b,
            Err(error) => {
                self.record_sync_error(epoch, id, &error)?;
                return Err(error);
            }
        };
        let view: EnrollmentView =
            serde_json::from_slice(&bytes).map_err(|_| "INVALID_CENTER_RESPONSE")?;
        if view.request_id != entry.id {
            return Err("REQUEST_MISMATCH".into());
        }
        let saved = self.update(epoch, |doc| {
            let e = doc.enrollments.get_mut(id).ok_or("NOT_FOUND")?;
            if e.view.as_ref().is_some_and(|v| v.revision > view.revision) {
                return Err("STALE_RESPONSE".into());
            }
            e.view = Some(view.clone());
            e.synced_at = Some(now());
            e.error = None;
            e.local_state = "WAITING_CENTER".into();
            Ok(e.clone())
        })?;
        if view.status != EnrollmentStatus::Issued {
            return Ok(());
        }
        let response = if let Some(response) = &saved.response {
            response.clone()
        } else {
            #[derive(serde::Deserialize)]
            struct Response {
                blind_credential: Box<serde_json::value::RawValue>,
            }
            let bytes = self
                .request(
                    reqwest::Method::GET,
                    &format!("{}/api/v1/enrollments/{id}/credential", center.base_url),
                    Some(&entry.access),
                    None,
                )
                .await?;
            let response: Response =
                serde_json::from_slice(&bytes).map_err(|_| "INVALID_CENTER_RESPONSE")?;
            let response = response.blind_credential.get().to_owned();
            self.update(epoch, |doc| {
                let e = doc.enrollments.get_mut(id).ok_or("NOT_FOUND")?;
                e.response = Some(response.clone());
                e.local_state = "RESPONSE_SAVED".into();
                Ok(())
            })?;
            response
        };
        self.finish_response(epoch, center, &saved, response).await
    }

    async fn finish_response(
        &self,
        epoch: u64,
        center: &Center,
        saved: &LocalEnrollment,
        response: String,
    ) -> Result<()> {
        let result = self
            .verify_and_store_response(epoch, center, saved, response)
            .await;
        if let Err(error) = &result {
            let _ = self.record_sync_error(epoch, &saved.id, error);
        }
        result
    }

    async fn verify_and_store_response(
        &self,
        epoch: u64,
        center: &Center,
        saved: &LocalEnrollment,
        response: String,
    ) -> Result<()> {
        let id = saved.id.as_str();
        let issuer = public(center)?;
        let pending = PendingEnrollment::import_secret(
            saved.pending.as_ref().ok_or("PENDING_MISSING")?.as_bytes(),
            &issuer,
        )
        .map_err(|_| "PENDING_CORRUPT")?;
        let response: BlindCredential =
            wire::from_json(response.as_bytes()).map_err(|_| "INVALID_CREDENTIAL_RESPONSE")?;
        let holder = tokio::task::spawn_blocking(move || pending.finish(&response))
            .await
            .map_err(|_| "CRYPTO_FAILED")?
            .map_err(|_| "CREDENTIAL_VERIFICATION_FAILED")?;
        let credential = Credential {
            id: id.into(),
            center_id: center.id.clone(),
            issuer_id: center.info.issuer_id.clone(),
            claims: holder
                .claims()
                .iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            saved_at: now(),
            holder: Zeroizing::new(
                String::from_utf8(
                    holder
                        .export_secret()
                        .map_err(|_| "CRYPTO_FAILED")?
                        .expose_secret()
                        .to_vec(),
                )
                .map_err(|_| "CRYPTO_FAILED")?,
            ),
        };
        if self
            .update(epoch, |doc| {
                doc.credentials.entry(id.into()).or_insert(credential);
                doc.enrollments.get_mut(id).ok_or("NOT_FOUND")?.local_state =
                    "CREDENTIAL_READY".into();
                Ok(())
            })
            .is_err()
        {
            let _ = self.update(epoch, |doc| {
                doc.enrollments.get_mut(id).ok_or("NOT_FOUND")?.local_state =
                    "CREDENTIAL_SAVE_FAILED".into();
                Ok(())
            });
            return Err("CREDENTIAL_SAVE_FAILED".into());
        }
        // Second commit removes pending only after the verified holder is recoverably stored.
        self.update(epoch, |doc| {
            let e = doc.enrollments.get_mut(id).ok_or("NOT_FOUND")?;
            e.pending = None;
            e.response = None;
            Ok(())
        })
    }

    fn record_sync_error(&self, epoch: u64, id: &str, error: &str) -> Result<()> {
        self.update(epoch, |doc| {
            let e = doc.enrollments.get_mut(id).ok_or("NOT_FOUND")?;
            e.error = Some(error.into());
            if e.view.is_none() {
                e.local_state = "SUBMISSION_UNKNOWN".into();
            }
            Ok(())
        })
    }

    pub fn pending_sync_ids(&self) -> Vec<(String, u32)> {
        self.snapshot(false)
            .map(|(_, doc)| {
                doc.enrollments
                    .values()
                    .filter(|e| {
                        !matches!(
                            e.local_state.as_str(),
                            "CREDENTIAL_READY" | "CREDENTIAL_DELETED"
                        ) && e.view.as_ref().is_none_or(|v| {
                            matches!(
                                v.status,
                                EnrollmentStatus::PendingReview
                                    | EnrollmentStatus::Approved
                                    | EnrollmentStatus::Signing
                                    | EnrollmentStatus::IssueFailed
                                    | EnrollmentStatus::Issued
                            )
                        })
                    })
                    .map(|e| {
                        (
                            e.id.clone(),
                            e.view.as_ref().map_or(5, |v| v.poll_interval_seconds),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn enrollment_view(e: &LocalEnrollment) -> Value {
    json!(crate::dto::LocalEnrollmentView {
        id: e.id.clone(),
        center_id: e.center_id.clone(),
        claims: e.claims.clone(),
        view: e.view.clone(),
        local_state: e.local_state.clone(),
        error: e.error.clone(),
        synced_at: e.synced_at,
        recovery_saved: e.pending.is_some() || e.local_state == "CREDENTIAL_READY"
    })
}
fn credential_view(c: &Credential) -> Value {
    json!(crate::dto::CredentialView {
        id: c.id.clone(),
        center_id: c.center_id.clone(),
        issuer_id: c.issuer_id.clone(),
        claims: c.claims.clone(),
        saved_at: c.saved_at
    })
}

fn disclosure_plan(doc: &Document, id: &str) -> Result<Value> {
    let task = doc.tasks.get(id).ok_or("NOT_FOUND")?;
    let app = doc.apps.get(&task.app_id).ok_or("UNTRUSTED_APP")?;
    let challenge: LoginChallenge =
        wire::from_json(task.challenge.as_bytes()).map_err(|_| "INVALID_CHALLENGE")?;
    let credential = doc
        .credentials
        .values()
        .find(|c| c.issuer_id == app.issuer_id);
    let required = challenge.required_claims().iter().collect::<Vec<_>>();
    let missing = required
        .iter()
        .filter(|k| credential.is_some_and(|c| !c.claims.contains_key(**k)))
        .copied()
        .collect::<Vec<_>>();
    let state = if matches!(task.state.as_str(), "REFUSED" | "PROOF_GENERATED") {
        task.state.as_str()
    } else if credential.is_none() {
        if doc.enrollments.values().any(|e| {
            e.local_state == "CREDENTIAL_DELETED"
                && doc
                    .centers
                    .get(&e.center_id)
                    .is_some_and(|c| c.info.issuer_id == app.issuer_id)
        }) {
            "CREDENTIAL_DELETED"
        } else {
            "NO_CREDENTIAL"
        }
    } else if task.state == "NEEDS_CHALLENGE"
        || challenge.expires_at() <= now()
        || task.app_revision != app.revision
    {
        "NEEDS_CHALLENGE"
    } else if !missing.is_empty() {
        "MISSING_CLAIMS"
    } else {
        "REVIEW_DISCLOSURE"
    };
    Ok(
        json!({"task":task,"app":app,"state":state,"expires_at":challenge.expires_at(),"required_claims":required,"missing_claims":missing,
        "credential":credential.map(credential_view),"center_id":doc.centers.values().find(|c| c.info.issuer_id == app.issuer_id).map(|c|c.id.clone())}),
    )
}
