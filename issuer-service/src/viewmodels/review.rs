use super::contracts::ReviewAction;
use super::{Application, idempotency::idempotency_result};
use crate::{
    error::{Error, Result},
    models::domain::*,
    models::{Idempotency, Settings},
    security::*,
    store::IsolationLevel,
};
use crypto_sso_identity_api::*;
use serde_json::json;

impl Application {
    #[tracing::instrument(skip_all)]
    pub async fn review(
        &self,
        id: &str,
        username: &str,
        key: &str,
        action: ReviewAction,
        decision: ReviewDecision,
    ) -> Result<EnrollmentView> {
        if decision
            .internal_note
            .as_ref()
            .is_some_and(|s| s.len() > 4096)
        {
            return Err(Error::invalid());
        }
        let content = json!([id, action.as_str(), decision]);
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
        if action == ReviewAction::Approve {
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
        } else if action == ReviewAction::RetryIssuance {
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
                || (action == ReviewAction::Reject
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
                if action == ReviewAction::RequestChanges {
                    EnrollmentStatus::ChangesRequired
                } else {
                    EnrollmentStatus::Rejected
                },
            );
        }
        e.history.push(ReviewHistory {
            actor: username.into(),
            at: now(),
            action: action.as_str().into(),
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
        audit(&mut tx, username, action.as_str(), id).await?;
        let result = e.view.0.clone();
        toasty::create!(Idempotency {
            key: key.to_owned(),
            digest: digest(&content.to_string()),
            result: serde_json::to_value(&result)?,
        })
        .exec(&mut tx)
        .await?;
        tx.commit().await?;
        tracing::info!(action = action.as_str(), "enrollment review committed");
        Ok(result)
    }
}
