//! Durable signing leases fence out stale workers. No response is exposed until its commit.
use crate::{
    error::{Error, Result},
    models::domain::*,
    models::{Settings, WebSession},
    security::now,
    store::IsolationLevel,
    viewmodels::Application,
};
use crypto_sso_identity_api::EnrollmentStatus as Status;
use crypto_sso_issuer::{Claims, EnrollmentRequest, wire};
use rand::rngs::OsRng;

async fn lease(app: &Application) -> Result<Option<Enrollment>> {
    let mut db = app.store.database();
    let mut tx = db
        .transaction_builder()
        .isolation(IsolationLevel::Serializable)
        .begin()
        .await?;
    let entries = Enrollment::all().exec(&mut tx).await?;
    let enabled = Settings::get_by_id(&mut tx, &1).await?.issuance_enabled;
    let mut candidate = None;
    for mut e in entries {
        let claimed = if e.view.status == Status::PendingReview && e.view.review_deadline <= now() {
            transition(&mut e, Status::Expired);
            audit(&mut tx, "worker", "ENROLLMENT_EXPIRED", &e.request_id).await?;
            false
        } else if e.view.status == Status::Signing && e.lease_until <= now() {
            tracing::warn!("recovering an expired signing lease");
            e.generation += 1;
            e.failure_code = Some("LEASE_EXPIRED".into());
            transition(&mut e, Status::IssueFailed);
            false
        } else if candidate.is_none() && enabled && e.view.status == Status::Approved {
            e.generation += 1;
            e.lease_until = now() + app.config.lease_seconds;
            transition(&mut e, Status::Signing);
            true
        } else {
            continue;
        };
        Enrollment::filter_by_request_id(e.request_id.clone())
            .update()
            .view(e.view.clone())
            .generation(e.generation)
            .lease_until(e.lease_until)
            .failure_code(e.failure_code.clone())
            .exec(&mut tx)
            .await?;
        if claimed {
            candidate = Some(e);
        }
    }
    for session in WebSession::all().exec(&mut tx).await? {
        if session.data.is_expired() {
            session.delete().exec(&mut tx).await?;
        }
    }
    // No signing may start before the lease has actually committed.
    tx.commit().await?;
    Ok(candidate)
}

#[tracing::instrument(skip_all)]
pub async fn tick(app: &Application) -> Result<bool> {
    let leased = match lease(app).await {
        Ok(task) => task,
        // Another worker won the serializable transaction. Try again next tick.
        Err(error) if error.code == "REVISION_CONFLICT" => {
            tracing::debug!("signing lease contention; retrying next tick");
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    let Some(task) = leased else {
        tracing::trace!("no signing job available");
        return Ok(false);
    };
    tracing::debug!("signing lease committed");
    let issuer = app.issuer.clone();
    let raw = task.raw_request.clone();
    let approved = task
        .decision
        .as_ref()
        .and_then(|d| d.approved_claims.clone())
        .ok_or_else(Error::unavailable)?;
    let result = tokio::task::spawn_blocking(move || {
        let request: EnrollmentRequest =
            wire::from_json(raw.as_bytes()).map_err(|_| "INVALID_REQUEST")?;
        let claims = Claims::new(approved).map_err(|_| "APPROVAL_MISMATCH")?;
        match issuer.issue_blind(&request, &claims, &mut OsRng) {
            Ok(response) => wire::to_json(&response).map_err(|_| "SIGNING_FAILED"),
            Err(crypto_sso_issuer::Error::InvalidEnrollmentProof) => Err("INVALID_REQUEST"),
            Err(crypto_sso_issuer::Error::ClaimsMismatch) => Err("APPROVAL_MISMATCH"),
            Err(_) => Err("SIGNING_FAILED"),
        }
    })
    .await
    .unwrap_or(Err("WORKER_FAILED"));
    let mut db = app.store.database();
    let mut tx = db
        .transaction_builder()
        .isolation(IsolationLevel::Serializable)
        .begin()
        .await?;
    let mut e = Enrollment::get_by_request_id(&mut tx, &task.request_id).await?;
    if e.view.status != Status::Signing || e.generation != task.generation || e.lease_until <= now()
    {
        return Ok(false);
    }
    let mut current_settings = Settings::get_by_id(&mut tx, &1).await?;
    if !current_settings.issuance_enabled {
        transition(&mut e, Status::Approved);
    } else {
        // A Serializable read alone allows this transaction to be ordered before
        // a concurrent settings change, even if it commits afterward. Updating
        // the same settings row through Toasty makes that change conflict: once
        // disabling issuance commits, a stale signing result cannot commit.
        // This is scoped to result submission, not a global transaction guard.
        let enabled = current_settings.issuance_enabled;
        current_settings
            .update()
            .issuance_enabled(enabled)
            .exec(&mut tx)
            .await?;
        match result {
            Ok(bytes) => {
                if e.view.request_hash != task.view.request_hash {
                    return Err(Error::conflict());
                }
                let member_ref = e
                    .decision
                    .as_ref()
                    .and_then(|d| d.member_ref.as_ref())
                    .ok_or_else(Error::unavailable)?;
                let mut member = Member::get_by_member_ref(&mut tx, member_ref).await?;
                if member.request_id != e.view.request_id || member.issued {
                    return Err(Error::conflict());
                }
                member.update().issued(true).exec(&mut tx).await?;
                e.blind_credential =
                    Some(String::from_utf8(bytes).map_err(|_| Error::unavailable())?);
                e.issued_at = Some(now());
                transition(&mut e, Status::Issued);
                audit(&mut tx, "worker", "CREDENTIAL_ISSUED", &e.view.request_id).await?;
            }
            Err(code) => {
                tracing::warn!(code, "blind signing failed");
                e.failure_code = Some(code.into());
                transition(
                    &mut e,
                    if code == "INVALID_REQUEST" {
                        Status::InvalidRequest
                    } else {
                        Status::IssueFailed
                    },
                );
                if e.view.status == Status::InvalidRequest
                    && let Some(member_ref) =
                        e.decision.as_ref().and_then(|d| d.member_ref.as_ref())
                {
                    Member::filter_by_member_ref(member_ref.clone())
                        .delete()
                        .exec(&mut tx)
                        .await?;
                }
                audit(&mut tx, "worker", code, &e.view.request_id).await?;
            }
        }
    }
    e.lease_until = 0;
    Enrollment::filter_by_request_id(e.request_id.clone())
        .update()
        .view(e.view.clone())
        .blind_credential(e.blind_credential.clone())
        .issued_at(e.issued_at)
        .failure_code(e.failure_code.clone())
        .lease_until(0)
        .exec(&mut tx)
        .await?;
    tx.commit().await?;
    tracing::info!(status = ?e.view.status, "signing result committed");
    Ok(true)
}

pub async fn run(app: Application) {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(
        app.config.worker_interval_ms,
    ));
    loop {
        interval.tick().await;
        if let Err(error) = tick(&app).await {
            tracing::error!(code = error.code, "signing worker tick failed");
        }
    }
}
