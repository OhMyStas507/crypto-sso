use crate::{
    error::{Error, Result},
    models::Idempotency,
    security::{digest, valid_token},
};
use serde_json::Value;

pub(super) async fn idempotency_result<T: serde::de::DeserializeOwned>(
    tx: &mut impl toasty::Executor,
    key: &str,
    input: &Value,
) -> Result<Option<T>> {
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
        return Ok(Some(serde_json::from_value(previous.result)?));
    }
    Ok(None)
}
