use crate::{
    config::Config,
    domain::*,
    error::{Error, Result},
    models::{AuditRecord, ClaimHintRevision, Idempotency, IssuerMetadata, Settings},
    security::*,
    store::{IsolationLevel, Store},
};
use crypto_sso_identity_api::*;
use crypto_sso_issuer::{EnrollmentRequest, Issuer, IssuerPublic, wire};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct Application {
    pub config: Arc<Config>,
    pub store: Store,
    pub issuer: Arc<Issuer>,
}

async fn idempotency_result(
    tx: &mut impl toasty::Executor,
    key: &str,
    input: &Value,
) -> Result<Option<Value>> {
    if !valid_token(key) {
        return Err(Error::new(
            400,
            "IDEMPOTENCY_KEY_REQUIRED",
            "请提供独立的 256 位幂等键",
        ));
    }
    if let Some(previous) = Idempotency::filter_by_key(key.to_owned())
        .first()
        .exec(tx)
        .await?
    {
        if previous.digest != digest(&input.to_string()) {
            return Err(Error::conflict());
        }
        return Ok(Some(previous.result));
    }
    Ok(None)
}

impl Application {
    #[tracing::instrument(skip_all)]
    pub async fn bootstrap(config: Config) -> anyhow::Result<Self> {
        let public: IssuerPublic = wire::from_json(&std::fs::read(&config.issuer_public_path)?)?;
        let secret = zeroize::Zeroizing::new(std::fs::read(&config.issuer_secret_path)?);
        let issuer = Arc::new(Issuer::import_secret(&secret, &public)?);
        let store = Store::connect(&config.database_url).await?;
        let mut db = store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        if let Some(id) = IssuerMetadata::filter_by_id(1)
            .first()
            .exec(&mut tx)
            .await?
        {
            if id.issuer_id != issuer.issuer_id().as_str() {
                return Err(
                    Error::new(409, "ISSUER_MISMATCH", "数据库固定的签发身份与配置不符").into(),
                );
            }
        } else {
            toasty::create!(IssuerMetadata {
                id: 1,
                issuer_id: issuer.issuer_id().as_str().to_owned()
            })
            .exec(&mut tx)
            .await?;
        }
        if Settings::filter_by_id(1)
            .first()
            .exec(&mut tx)
            .await?
            .is_none()
        {
            let initial = toasty::create!(Settings {
                id: 1,
                revision: 1,
                claim_hints_revision: 1,
                display_name: public.organization().to_owned(),
                description: "",
                contact: "",
                public_base_url: config.public_origin.clone(),
                registration_enabled: false,
                issuance_enabled: false,
                review_ttl_seconds: 7 * 86400,
                poll_interval_seconds: 5,
                updated_at: now(),
                claim_hints: Vec::<ClaimHint>::new(),
            })
            .exec(&mut tx)
            .await?;
            toasty::create!(ClaimHintRevision {
                revision: initial.claim_hints_revision,
                hints: initial.claim_hints,
            })
            .exec(&mut tx)
            .await?;
        }
        tx.commit().await?;
        Ok(Self {
            config: Arc::new(config),
            store,
            issuer,
        })
    }

    pub async fn info(&self) -> Result<ServiceInfo> {
        let s = Settings::get_by_id(&mut self.store.database(), &1).await?;
        let limits = crypto_sso_issuer::claims_limits();
        Ok(ServiceInfo {
            api_version: "v1".into(),
            issuer_id: self.issuer.issuer_id().as_str().into(),
            issuer_public: serde_json::value::RawValue::from_string(
                String::from_utf8(
                    wire::to_json(self.issuer.public()).map_err(|_| Error::unavailable())?,
                )
                .map_err(|_| Error::unavailable())?,
            )?,
            settings: s.into(),
            effective_claims_limits: ClaimsLimits {
                max_keys: limits.max_keys as u32,
                max_key_bytes: limits.max_key_bytes as u32,
                max_value_bytes: limits.max_value_bytes as u32,
            },
        })
    }

    #[tracing::instrument(skip_all)]
    pub async fn submit(
        &self,
        input: SubmitEnrollment,
        access: &str,
    ) -> Result<(bool, EnrollmentView)> {
        if !valid_token(access)
            || input
                .application_note
                .as_ref()
                .is_some_and(|v| v.len() > 4096)
        {
            return Err(Error::invalid());
        }
        let request: EnrollmentRequest = wire::from_json(input.enrollment_request.get().as_bytes())
            .map_err(|_| Error::invalid())?;
        if request.issuer_id() != self.issuer.issuer_id() {
            return Err(Error::new(400, "ISSUER_MISMATCH", "申请的中心身份不匹配"));
        }
        let request_id = request.request_id().as_str().to_owned();
        let request_hash = request
            .request_hash()
            .map_err(|_| Error::invalid())?
            .as_str()
            .to_owned();
        let metadata_hash =
            digest(&json!([input.claim_hints_revision, input.application_note]).to_string());
        let raw_request = String::from_utf8(wire::to_json(&request).map_err(|_| Error::invalid())?)
            .map_err(|_| Error::invalid())?;
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        if let Some(previous) = Enrollment::filter_by_request_id(request_id.clone())
            .first()
            .exec(&mut tx)
            .await?
        {
            if !equal(&previous.access_hash, &digest(access)) {
                return Err(Error::not_found());
            }
            if previous.view.request_hash != request_hash || previous.metadata_hash != metadata_hash
            {
                return Err(Error::conflict());
            }
            tx.commit().await?;
            return Ok((false, previous.view.0));
        }
        let s = Settings::get_by_id(&mut tx, &1).await?;
        if !s.registration_enabled {
            return Err(Error::new(
                403,
                "REGISTRATION_CLOSED",
                "中心已暂停接受新申请",
            ));
        }
        let submitted_at = now();
        let view = EnrollmentView {
            request_id,
            request_hash,
            status: EnrollmentStatus::PendingReview,
            revision: 1,
            submitted_at,
            review_deadline: submitted_at + u64::from(s.review_ttl_seconds),
            public_reason: None,
            poll_interval_seconds: s.poll_interval_seconds,
        };
        let e = toasty::create!(Enrollment {
            request_id: view.request_id.clone(),
            view: view.clone(),
            raw_request,
            claims: request
                .claims()
                .iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect::<Claims>(),
            metadata_hash,
            access_hash: digest(access),
            claim_hints_revision: input.claim_hints_revision,
            hints_snapshot: match input.claim_hints_revision {
                Some(rev) => ClaimHintRevision::filter_by_revision(rev)
                    .first()
                    .exec(&mut tx)
                    .await?
                    .map(|revision| revision.hints),
                None => None,
            },
            application_note: input.application_note,
            decision: None,
            reviewer: None,
            reviewed_at: None,
            blind_credential: None,
            issued_at: None,
            generation: 0,
            lease_until: 0,
            failure_code: None,
            history: Vec::<ReviewHistory>::new(),
        })
        .exec(&mut tx)
        .await?;
        audit(&mut tx, "wallet", "ENROLLMENT_ACCEPTED", &e.view.request_id).await?;
        tx.commit().await?;
        tracing::info!("enrollment accepted");
        Ok((true, view))
    }

    #[tracing::instrument(skip_all)]
    pub async fn wallet_action(
        &self,
        id: &str,
        access: &str,
        action: &str,
        expected: Option<u32>,
    ) -> Result<Value> {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let mut e = Enrollment::get_by_request_id(&mut tx, &id.to_owned()).await?;
        if !valid_token(access) || !equal(&e.access_hash, &digest(access)) {
            return Err(Error::not_found());
        }
        let result = if action == "credential" {
            let raw = e
                .blind_credential
                .filter(|_| e.view.status == EnrollmentStatus::Issued)
                .ok_or_else(|| Error::new(409, "CREDENTIAL_NOT_READY", "凭证尚未签发"))?;
            json!({"blind_credential": serde_json::from_str::<Box<serde_json::value::RawValue>>(&raw)?})
        } else {
            if action == "cancel" {
                require_revision(&e, expected.ok_or_else(Error::invalid)?)?;
                if e.view.status != EnrollmentStatus::PendingReview {
                    return Err(Error::conflict());
                }
                transition(&mut e, EnrollmentStatus::Cancelled);
                let next_view = e.view.clone();
                e.update().view(next_view).exec(&mut tx).await?;
                audit(&mut tx, "wallet", "ENROLLMENT_CANCELLED", id).await?;
            }
            serde_json::to_value(e.view)?
        };
        tx.commit().await?;
        if action == "cancel" {
            tracing::info!("enrollment cancellation committed");
        }
        Ok(result)
    }

    #[tracing::instrument(skip_all)]
    pub async fn save_settings(
        &self,
        username: &str,
        key: &str,
        mut incoming: ServiceSettings,
    ) -> Result<Value> {
        validate_settings(&incoming)?;
        let content = json!(["settings", incoming]);
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        if let Some(previous) = idempotency_result(&mut tx, key, &content).await? {
            tx.commit().await?;
            return Ok(previous);
        }

        let previous = Settings::get_by_id(&mut tx, &1).await?;
        if incoming.revision != previous.revision
            || incoming.claim_hints_revision != previous.claim_hints_revision
        {
            return Err(Error::conflict());
        }
        incoming.revision += 1;
        if serde_json::to_value(&incoming.claim_hints)?
            != serde_json::to_value(&previous.claim_hints)?
        {
            incoming.claim_hints_revision += 1;
            toasty::create!(ClaimHintRevision {
                revision: incoming.claim_hints_revision,
                hints: incoming.claim_hints.clone()
            })
            .exec(&mut tx)
            .await?;
        }
        incoming.updated_at = now();
        Settings::filter_by_id(1)
            .update()
            .revision(incoming.revision)
            .claim_hints_revision(incoming.claim_hints_revision)
            .display_name(incoming.display_name.clone())
            .description(incoming.description.clone())
            .contact(incoming.contact.clone())
            .public_base_url(incoming.public_base_url.clone())
            .registration_enabled(incoming.registration_enabled)
            .issuance_enabled(incoming.issuance_enabled)
            .review_ttl_seconds(incoming.review_ttl_seconds)
            .poll_interval_seconds(incoming.poll_interval_seconds)
            .claim_hints(incoming.claim_hints.clone())
            .updated_at(incoming.updated_at)
            .exec(&mut tx)
            .await?;
        audit(
            &mut tx,
            username,
            "SETTINGS_UPDATED",
            &format!("r{}", incoming.revision),
        )
        .await?;
        let result = serde_json::to_value(&incoming)?;
        toasty::create!(Idempotency {
            key: key.to_owned(),
            digest: digest(&content.to_string()),
            result: result.clone(),
        })
        .exec(&mut tx)
        .await?;
        tx.commit().await?;
        tracing::info!("settings update committed");
        Ok(result)
    }

    #[tracing::instrument(skip_all)]
    pub async fn review(
        &self,
        id: &str,
        username: &str,
        key: &str,
        action: &str,
        decision: ReviewDecision,
    ) -> Result<Value> {
        if decision
            .internal_note
            .as_ref()
            .is_some_and(|s| s.len() > 4096)
        {
            return Err(Error::invalid());
        }
        let content = json!([id, action, decision]);
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        if let Some(previous) = idempotency_result(&mut tx, key, &content).await? {
            tx.commit().await?;
            return Ok(previous);
        }

        let mut e = Enrollment::get_by_request_id(&mut tx, &id.to_owned()).await?;
        require_revision(&e, decision.expected_revision)?;
        if decision.request_hash != e.view.request_hash {
            return Err(Error::conflict());
        }
        let s = Settings::get_by_id(&mut tx, &1).await?;
        if action == "approve" {
            if !s.issuance_enabled {
                return Err(Error::new(409, "ISSUANCE_PAUSED", "签发已暂停"));
            }
            if e.view.status != EnrollmentStatus::PendingReview || e.view.review_deadline <= now() {
                return Err(Error::conflict());
            }
            let member_ref = decision
                .member_ref
                .as_deref()
                .filter(|v| !v.trim().is_empty() && v.len() <= 128)
                .ok_or_else(Error::invalid)?;
            let source = decision
                .evidence_source
                .as_deref()
                .filter(|v| !v.trim().is_empty() && v.len() <= 2048)
                .ok_or_else(Error::invalid)?;
            let note = decision
                .verification_note
                .as_deref()
                .filter(|v| !v.trim().is_empty() && v.len() <= 4096)
                .ok_or_else(Error::invalid)?;
            if !decision.claims_confirmed || decision.approved_claims.as_ref() != Some(&e.claims.0)
            {
                return Err(Error::new(
                    400,
                    "CLAIMS_MISMATCH",
                    "必须明确核准完整且相同的申请属性",
                ));
            }
            if Member::filter_by_member_ref(member_ref.to_owned())
                .first()
                .exec(&mut tx)
                .await?
                .is_some()
            {
                return Err(Error::new(
                    409,
                    "MEMBER_ALREADY_RESERVED",
                    "该成员已签发或已保留签发名额",
                ));
            }
            toasty::create!(Member {
                member_ref: member_ref.to_owned(),
                enrollment: &e,
                evidence_source: source.to_owned(),
                verification_note: note.to_owned(),
                verified_at: now(),
                issued: false,
            })
            .exec(&mut tx)
            .await?;
            transition(&mut e, EnrollmentStatus::Approved);
            e.decision = Some(toasty::Json(decision.clone()));
            e.reviewer = Some(username.into());
            e.reviewed_at = Some(now());
        } else if action == "retry-issuance" {
            if !s.issuance_enabled {
                return Err(Error::new(409, "ISSUANCE_PAUSED", "签发已暂停"));
            }
            if e.view.status != EnrollmentStatus::IssueFailed {
                return Err(Error::conflict());
            }
            transition(&mut e, EnrollmentStatus::Approved);
            e.failure_code = None;
        } else {
            let allowed = e.view.status == EnrollmentStatus::PendingReview
                || (action == "reject"
                    && matches!(
                        e.view.status,
                        EnrollmentStatus::Approved | EnrollmentStatus::IssueFailed
                    ));
            if !allowed {
                return Err(Error::conflict());
            }
            let reason = decision
                .public_reason
                .as_deref()
                .filter(|v| !v.trim().is_empty() && v.len() <= 4096)
                .ok_or_else(Error::invalid)?;
            e.view.public_reason = Some(reason.into());
            if let Some(member) = e.decision.as_ref().and_then(|d| d.member_ref.as_ref()) {
                Member::filter_by_member_ref(member.clone())
                    .delete()
                    .exec(&mut tx)
                    .await?;
            }
            e.generation += 1;
            transition(
                &mut e,
                if action == "request-changes" {
                    EnrollmentStatus::ChangesRequired
                } else {
                    EnrollmentStatus::Rejected
                },
            );
        }
        e.history.push(ReviewHistory {
            actor: username.into(),
            at: now(),
            action: action.into(),
            decision: decision.clone(),
        });
        Enrollment::filter_by_request_id(e.request_id.clone())
            .update()
            .view(e.view.clone())
            .decision(e.decision.clone())
            .reviewer(e.reviewer.clone())
            .reviewed_at(e.reviewed_at)
            .generation(e.generation)
            .failure_code(e.failure_code.clone())
            .history(e.history.clone())
            .exec(&mut tx)
            .await?;
        audit(&mut tx, username, action, id).await?;
        let result = serde_json::to_value(&e.view)?;
        toasty::create!(Idempotency {
            key: key.to_owned(),
            digest: digest(&content.to_string()),
            result: result.clone(),
        })
        .exec(&mut tx)
        .await?;
        tx.commit().await?;
        tracing::info!(action, "enrollment review committed");
        Ok(result)
    }

    #[tracing::instrument(skip_all)]
    pub async fn query(
        &self,
        resource: &str,
        id: Option<&str>,
        status: Option<&str>,
        search: Option<&str>,
        cursor: usize,
    ) -> Result<Value> {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::RepeatableRead)
            .read_only(true)
            .begin()
            .await?;
        let result = match resource {
            "overview" => {
                let enrollments = list_enrollments(&mut tx).await?;
                json!({
                    "settings": ServiceSettings::from(Settings::get_by_id(&mut tx, &1).await?),
                    "issuer_id": self.issuer.issuer_id().as_str(),
                    "key_available": true,
                    "pending": enrollments.iter().filter(|e| e.view.status == EnrollmentStatus::PendingReview).count(),
                    "failed": enrollments.iter().filter(|e| e.view.status == EnrollmentStatus::IssueFailed).count(),
                    "issued": enrollments.iter().filter(|e| e.view.status == EnrollmentStatus::Issued).count(),
                    "recent": enrollments.iter().take(6).map(admin_enrollment).collect::<Result<Vec<_>>>()?,
                })
            }
            "settings" => serde_json::to_value(ServiceSettings::from(
                Settings::get_by_id(&mut tx, &1).await?,
            ))?,
            "enrollments" | "issuances" => {
                if let Some(id) = id {
                    admin_enrollment(
                        &Enrollment::get_by_request_id(&mut tx, &id.to_owned()).await?,
                    )?
                } else {
                    let enrollments = list_enrollments(&mut tx).await?;
                    let items = enrollments
                        .iter()
                        .filter(|e| {
                            let state = serde_json::to_value(&e.view.status).unwrap_or_default();
                            (resource != "issuances" || e.decision.is_some())
                                && status.is_none_or(|s| state.as_str() == Some(s))
                                && search.is_none_or(|q| {
                                    e.view.request_id.contains(q)
                                        || e.claims.values().any(|v| v.contains(q))
                                })
                        })
                        .map(admin_enrollment)
                        .collect::<Result<Vec<_>>>()?;
                    page(items, cursor)?
                }
            }
            "members" => {
                if let Some(id) = id {
                    let member = Member::get_by_member_ref(&mut tx, &id.to_owned()).await?;
                    let entry = member.enrollment().exec(&mut tx).await?;
                    json!({"member": member, "enrollment": admin_enrollment(&entry)?})
                } else {
                    let members = Member::all().exec(&mut tx).await?;
                    let items = members
                        .into_iter()
                        .filter(|m| search.is_none_or(|q| m.member_ref.contains(q)))
                        .map(|m| json!(m))
                        .collect();
                    page(items, cursor)?
                }
            }
            "audit-events" => {
                let mut events = AuditRecord::all().exec(&mut tx).await?;
                events.sort_by_key(|e| std::cmp::Reverse((e.at, e.id.clone())));
                page(events.into_iter().map(|e| json!(e)).collect(), cursor)?
            }
            _ => return Err(Error::not_found()),
        };
        tx.commit().await?;
        Ok(result)
    }
}

async fn list_enrollments(tx: &mut impl toasty::Executor) -> Result<Vec<Enrollment>> {
    let mut entries = Enrollment::all().exec(tx).await?;
    entries.sort_by_key(|e| std::cmp::Reverse((e.view.submitted_at, e.view.request_id.clone())));
    Ok(entries)
}

pub fn admin_enrollment(e: &Enrollment) -> Result<Value> {
    // Explicit read model: never serialize enrollment access hashes or raw signing responses to the admin UI.
    Ok(
        json!({"view": e.view, "claims": e.claims, "application_note": e.application_note,
        "claim_hints_revision": e.claim_hints_revision, "hints_snapshot": e.hints_snapshot,
        "decision": e.decision, "reviewer": e.reviewer, "reviewed_at": e.reviewed_at,
        "issued_at": e.issued_at, "failure_code": e.failure_code, "history": e.history}),
    )
}

fn page(items: Vec<Value>, cursor: usize) -> Result<Value> {
    let total = items.len();
    Ok(
        json!({"items": items.into_iter().skip(cursor).take(25).collect::<Vec<_>>(), "total": total,
        "next_cursor": if cursor.saturating_add(25) < total { Some((cursor + 25).to_string()) } else { None }}),
    )
}
