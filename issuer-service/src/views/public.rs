use super::{app, bearer, body, header, parameter};
use crate::{
    error::Result,
    viewmodels::contracts::{CancelEnrollmentRequest, ClaimHintsResponse, CredentialResponse},
};
use crypto_sso_identity_api::{EnrollmentView, ServiceInfo, SubmitEnrollment};
use salvo::{http::StatusCode, prelude::*};

/// 属性提示 HTTP 响应，明确区分带数据的响应和无响应体的缓存命中。
pub enum ClaimHintsReply {
    Modified(ClaimHintsResponse),
    NotModified,
}

impl Scribe for ClaimHintsReply {
    fn render(self, res: &mut Response) {
        match self {
            Self::Modified(hints) => res.render(Json(hints)),
            Self::NotModified => {
                res.status_code(StatusCode::NOT_MODIFIED);
            }
        }
    }
}

#[handler]
pub async fn service_info_get(depot: &Depot) -> Result<Json<ServiceInfo>> {
    Ok(Json(app(depot).info().await?))
}

#[handler]
pub async fn claim_hints_get(
    req: &Request,
    depot: &Depot,
    res: &mut Response,
) -> Result<ClaimHintsReply> {
    let info = app(depot).info().await?;
    let etag = format!(
        "\"{}-{}\"",
        info.issuer_id, info.settings.claim_hints_revision
    );
    res.headers_mut().insert("etag", etag.parse().unwrap());
    if header(req, "if-none-match") == etag {
        return Ok(ClaimHintsReply::NotModified);
    }
    Ok(ClaimHintsReply::Modified(ClaimHintsResponse {
        api_version: "v1".into(),
        issuer_id: info.issuer_id,
        revision: info.settings.claim_hints_revision,
        updated_at: info.settings.updated_at,
        mode: "advisory".into(),
        allow_custom_claims: true,
        claims: info.settings.claim_hints,
        effective_claims_limits: info.effective_claims_limits,
    }))
}

#[handler]
pub async fn enrollment_submit(
    req: &mut Request,
    depot: &Depot,
    res: &mut Response,
) -> Result<Json<EnrollmentView>> {
    let app = app(depot);
    let access = bearer(req);
    let input: SubmitEnrollment = body(req, app.config.body_limit).await?;
    let (created, view) = app.submit(input, &access).await?;
    res.status_code(if created {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    });
    Ok(Json(view))
}

#[handler]
pub async fn enrollment_status_get(req: &Request, depot: &Depot) -> Result<Json<EnrollmentView>> {
    Ok(Json(
        app(depot)
            .enrollment_status(&parameter(req, "request_id")?, &bearer(req))
            .await?,
    ))
}

#[handler]
pub async fn credential_get(req: &Request, depot: &Depot) -> Result<Json<CredentialResponse>> {
    Ok(Json(
        app(depot)
            .credential_get(&parameter(req, "request_id")?, &bearer(req))
            .await?,
    ))
}

#[handler]
pub async fn enrollment_cancel(req: &mut Request, depot: &Depot) -> Result<Json<EnrollmentView>> {
    let app = app(depot);
    let id = parameter(req, "request_id")?;
    let access = bearer(req);
    let input: CancelEnrollmentRequest = body(req, app.config.body_limit).await?;
    Ok(Json(
        app.enrollment_cancel(&id, &access, input.expected_revision)
            .await?,
    ))
}
