use super::{app, bearer, body, header, parameter};
use crate::{
    error::Result,
    viewmodels::contracts::{CancelEnrollmentRequest, ClaimHintsResponse, CredentialResponse},
};
use crypto_sso_identity_api::{EnrollmentView, ServiceInfo, SubmitEnrollment};
use salvo::{http::StatusCode, prelude::*};
use salvo_oapi::endpoint;

/// 属性提示 HTTP 响应，明确区分带数据的响应和无响应体的缓存命中。
pub enum ClaimHintsReply {
    Modified(ClaimHintsResponse),
    NotModified,
}

impl salvo_oapi::EndpointOutRegister for ClaimHintsReply {
    fn register(components: &mut salvo_oapi::Components, operation: &mut salvo_oapi::Operation) {
        Json::<ClaimHintsResponse>::register(components, operation);
        operation
            .responses
            .insert("304", salvo_oapi::Response::new("Not Modified"));
    }
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

#[endpoint(tags("公开接口"), summary = "读取中心身份、公钥和服务设置")]
pub async fn service_info_get(depot: &Depot) -> Result<Json<ServiceInfo>> {
    Ok(Json(app(depot).info().await?))
}

#[endpoint(
    tags("公开接口"),
    summary = "读取属性建议，支持 ETag 缓存",
    parameters(("if-none-match" = Option<String>, Header, description = "上次响应的 ETag")),
    responses((status_code = 200, body = ClaimHintsResponse, headers(("etag" = String, description = "建议版本标识"))), (status_code = 304, description = "建议未修改，无响应体", headers(("etag" = String))))
)]
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

#[endpoint(
    tags("公开接口"),
    summary = "提交申请；相同申请可幂等重试",
    request_body = SubmitEnrollment,
    security(["enrollment_access" = []]),
    responses((status_code = 202, description = "新申请已受理", body = EnrollmentView), (status_code = 200, description = "返回已有申请", body = EnrollmentView))
)]
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

#[endpoint(
    tags("公开接口"),
    summary = "查询申请状态",
    parameters(("request_id" = String, Path, description = "申请 ID")),
    security(["enrollment_access" = []])
)]
pub async fn enrollment_status_get(req: &Request, depot: &Depot) -> Result<Json<EnrollmentView>> {
    Ok(Json(
        app(depot)
            .enrollment_status(&parameter(req, "request_id")?, &bearer(req))
            .await?,
    ))
}

#[endpoint(
    tags("公开接口"),
    summary = "领取已签发的盲凭证",
    parameters(("request_id" = String, Path, description = "申请 ID")),
    security(["enrollment_access" = []])
)]
pub async fn credential_get(req: &Request, depot: &Depot) -> Result<Json<CredentialResponse>> {
    Ok(Json(
        app(depot)
            .credential_get(&parameter(req, "request_id")?, &bearer(req))
            .await?,
    ))
}

#[endpoint(
    tags("公开接口"),
    summary = "取消申请",
    request_body = CancelEnrollmentRequest,
    parameters(("request_id" = String, Path, description = "申请 ID")),
    security(["enrollment_access" = []])
)]
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
