use super::{Application, idempotency::idempotency_result};
use crate::{
    error::{Error, Result},
    models::domain::*,
    models::{ClaimHintRevision, Idempotency, Settings},
    security::*,
    store::IsolationLevel,
};
use crypto_sso_identity_api::*;
use serde_json::json;

impl Application {
    #[tracing::instrument(skip_all)]
    pub async fn save_settings(
        &self,
        username: &str,
        key: &str,
        mut incoming: ServiceSettings,
    ) -> Result<ServiceSettings> {
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
        let result = incoming;
        toasty::create!(Idempotency {
            key: key.to_owned(),
            digest: digest(&content.to_string()),
            result: serde_json::to_value(&result)?,
        })
        .exec(&mut tx)
        .await?;
        tx.commit().await?;
        tracing::info!("settings update committed");
        Ok(result)
    }
}
