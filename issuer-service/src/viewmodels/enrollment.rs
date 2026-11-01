use super::Application;
use super::contracts::CredentialResponse;
use crate::{
    error::{Error, Result},
    models::domain::*,
    models::{ClaimHintRevision, Settings},
    security::*,
    store::IsolationLevel,
};
use crypto_sso_identity_api::*;
use crypto_sso_issuer::{EnrollmentRequest, wire};
use serde_json::json;

impl Application {
    pub async fn enrollment_status(&self, id: &str, access: &str) -> Result<EnrollmentView> {
        let enrollment = wallet_enrollment(&mut self.store.database(), id, access).await?;
        Ok(enrollment.view.0)
    }

    pub async fn credential_get(&self, id: &str, access: &str) -> Result<CredentialResponse> {
        let enrollment = wallet_enrollment(&mut self.store.database(), id, access).await?;
        let raw = enrollment
            .blind_credential
            .filter(|_| enrollment.view.status == EnrollmentStatus::Issued)
            .ok_or_else(|| Error::new(409, "CREDENTIAL_NOT_READY", "凭证尚未签发"))?;
        Ok(CredentialResponse {
            blind_credential: serde_json::from_str(&raw)?,
        })
    }

    #[tracing::instrument(skip_all)]
    pub async fn enrollment_cancel(
        &self,
        id: &str,
        access: &str,
        expected_revision: u32,
    ) -> Result<EnrollmentView> {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let mut enrollment = wallet_enrollment(&mut tx, id, access).await?;
        require_revision(&enrollment, expected_revision)?;
        if enrollment.view.status != EnrollmentStatus::PendingReview {
            return Err(Error::conflict());
        }
        transition(&mut enrollment, EnrollmentStatus::Cancelled);
        let view = enrollment.view.0.clone();
        enrollment.update().view(view.clone()).exec(&mut tx).await?;
        audit(&mut tx, "wallet", "ENROLLMENT_CANCELLED", id).await?;
        tx.commit().await?;
        tracing::info!("enrollment cancellation committed");
        Ok(view)
    }

    pub async fn info(&self) -> Result<ServiceInfo> {
        let issuer = self.issuer().await?;
        let s = Settings::get_by_id(&mut self.store.database(), &1).await?;
        let limits = crypto_sso_issuer::claims_limits();
        Ok(ServiceInfo {
            api_version: "v1".into(),
            issuer_id: issuer.issuer_id().as_str().into(),
            issuer_public: serde_json::value::RawValue::from_string(
                String::from_utf8(
                    wire::to_json(issuer.public()).map_err(|_| Error::unavailable())?,
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
        if request.issuer_id() != self.issuer().await?.issuer_id() {
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
}

async fn wallet_enrollment(
    tx: &mut impl toasty::Executor,
    id: &str,
    access: &str,
) -> Result<Enrollment> {
    let enrollment = Enrollment::get_by_request_id(tx, &id.to_owned()).await?;
    if !valid_token(access) || !equal(&enrollment.access_hash, &digest(access)) {
        return Err(Error::not_found());
    }
    Ok(enrollment)
}
