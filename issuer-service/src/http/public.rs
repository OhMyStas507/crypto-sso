use super::{app, bearer, body, header, parameter};
use crate::error::Result;
use crypto_sso_identity_api::SubmitEnrollment;
use salvo::{http::StatusCode, prelude::*};
use serde::Deserialize;
use serde_json::{Value, json};

#[handler]
pub async fn service_info(depot: &Depot) -> Result<Json<Value>> {
    Ok(Json(serde_json::to_value(app(depot).info().await?)?))
}

#[handler]
pub async fn claim_hints(req: &Request, depot: &Depot, res: &mut Response) -> Result<()> {
    let info = app(depot).info().await?;
    let etag = format!(
        "\"{}-{}\"",
        info.issuer_id, info.settings.claim_hints_revision
    );
    res.headers_mut().insert("etag", etag.parse().unwrap());
    if header(req, "if-none-match") == etag {
        res.status_code(StatusCode::NOT_MODIFIED);
        return Ok(());
    }
    res.render(Json(json!({"api_version":"v1","issuer_id":info.issuer_id,"revision":info.settings.claim_hints_revision,"updated_at":info.settings.updated_at,
        "mode":"advisory","allow_custom_claims":true,"claims":info.settings.claim_hints,"effective_claims_limits":info.effective_claims_limits})));
    Ok(())
}

#[handler]
pub async fn submit_enrollment(
    req: &mut Request,
    depot: &Depot,
    res: &mut Response,
) -> Result<Json<Value>> {
    let app = app(depot);
    let access = bearer(req);
    let input: SubmitEnrollment = body(req, app.config.body_limit).await?;
    let (created, view) = app.submit(input, &access).await?;
    res.status_code(if created {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    });
    Ok(Json(serde_json::to_value(view)?))
}
#[handler]
pub async fn enrollment_status(req: &Request, depot: &Depot) -> Result<Json<Value>> {
    Ok(Json(
        app(depot)
            .wallet_action(&parameter(req, "request_id")?, &bearer(req), "status", None)
            .await?,
    ))
}
#[handler]
pub async fn credential(req: &Request, depot: &Depot) -> Result<Json<Value>> {
    Ok(Json(
        app(depot)
            .wallet_action(
                &parameter(req, "request_id")?,
                &bearer(req),
                "credential",
                None,
            )
            .await?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cancel {
    expected_revision: u32,
}

#[handler]
pub async fn cancel_enrollment(req: &mut Request, depot: &Depot) -> Result<Json<Value>> {
    let app = app(depot);
    let id = parameter(req, "request_id")?;
    let access = bearer(req);
    let input: Cancel = body(req, app.config.body_limit).await?;
    Ok(Json(
        app.wallet_action(&id, &access, "cancel", Some(input.expected_revision))
            .await?,
    ))
}
