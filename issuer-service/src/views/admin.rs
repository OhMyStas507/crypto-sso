use super::{app, body, header, parameter};
use crate::{
    error::{Error, Result},
    models::domain::AdminSession,
    viewmodels::contracts::*,
};
use crypto_sso_identity_api::{EnrollmentView, Page, ReviewDecision, ServiceSettings};
use salvo::{
    prelude::*,
    session::{SessionDepotExt, SessionStore},
};
use salvo_csrf::CsrfDepotExt;

#[handler]
pub async fn csrf_token_get(depot: &Depot) -> Json<CsrfTokenResponse> {
    Json(CsrfTokenResponse {
        csrf_token: depot.csrf_token().map(str::to_owned),
    })
}

#[handler]
pub async fn admin_session_create(
    req: &mut Request,
    depot: &mut Depot,
) -> Result<Json<AdminSession>> {
    let app = app(depot);
    let input: LoginRequest = body(req, 4096).await?;
    let admin = app
        .admin_authenticate(input, depot.csrf_token().unwrap_or_default().into())
        .await?;
    // 撤销旧服务端会话后，由 Salvo 轮换并签署新 Cookie。
    if let Some(old) = depot.session().cloned() {
        app.http_sessions
            .with_namespace("session")
            .destroy_session(old)
            .await
            .map_err(|_| Error::unavailable())?;
    }
    let session = depot.session_mut().ok_or_else(Error::unavailable)?;
    session.regenerate();
    session.insert("admin", &admin)?;
    app.admin_session_record(&admin).await?;
    Ok(Json(admin))
}

#[handler]
pub async fn admin_session_get(depot: &Depot) -> Json<AdminSession> {
    let mut admin = depot.get_typed::<AdminSession>().unwrap().clone();
    admin.csrf_token = depot.csrf_token().unwrap_or_default().into();
    Json(admin)
}

#[handler]
pub async fn admin_session_delete(depot: &mut Depot) -> Result<Json<LogoutResponse>> {
    depot
        .session_mut()
        .ok_or_else(Error::unavailable)?
        .destroy();
    Ok(Json(LogoutResponse { logged_out: true }))
}

fn list_query(req: &Request) -> ListQuery {
    ListQuery {
        status: req.query("status"),
        search: req.query("q"),
        cursor: req.query("cursor").unwrap_or(0),
    }
}

#[handler]
pub async fn overview_get(depot: &Depot) -> Result<Json<OverviewView>> {
    Ok(Json(app(depot).overview_get().await?))
}

#[handler]
pub async fn settings_get(depot: &Depot) -> Result<Json<ServiceSettings>> {
    Ok(Json(app(depot).settings_get().await?))
}

#[handler]
pub async fn enrollments_list(
    req: &Request,
    depot: &Depot,
) -> Result<Json<Page<AdminEnrollmentView>>> {
    Ok(Json(app(depot).enrollments_list(list_query(req)).await?))
}

#[handler]
pub async fn enrollment_get(req: &Request, depot: &Depot) -> Result<Json<AdminEnrollmentView>> {
    Ok(Json(
        app(depot)
            .enrollment_get(&parameter(req, "request_id")?)
            .await?,
    ))
}

#[handler]
pub async fn members_list(req: &Request, depot: &Depot) -> Result<Json<Page<MemberView>>> {
    Ok(Json(app(depot).members_list(list_query(req)).await?))
}

#[handler]
pub async fn member_get(req: &Request, depot: &Depot) -> Result<Json<MemberDetailView>> {
    Ok(Json(
        app(depot)
            .member_get(&parameter(req, "member_ref")?)
            .await?,
    ))
}

#[handler]
pub async fn issuances_list(
    req: &Request,
    depot: &Depot,
) -> Result<Json<Page<AdminEnrollmentView>>> {
    Ok(Json(app(depot).issuances_list(list_query(req)).await?))
}

#[handler]
pub async fn audit_events_list(req: &Request, depot: &Depot) -> Result<Json<Page<AuditEventView>>> {
    Ok(Json(app(depot).audit_events_list(list_query(req)).await?))
}

#[handler]
pub async fn settings_save(req: &mut Request, depot: &Depot) -> Result<Json<ServiceSettings>> {
    let app = app(depot);
    let admin = depot.get_typed::<AdminSession>().unwrap();
    let key = header(req, "idempotency-key");
    let input: ServiceSettings = body(req, app.config.body_limit).await?;
    Ok(Json(app.save_settings(&admin.username, &key, input).await?))
}

async fn enrollment_review(
    req: &mut Request,
    depot: &Depot,
    action: ReviewAction,
) -> Result<Json<EnrollmentView>> {
    let app = app(depot);
    let admin = depot.get_typed::<AdminSession>().unwrap();
    let id = parameter(req, "request_id")?;
    let key = header(req, "idempotency-key");
    let input: ReviewDecision = body(req, app.config.body_limit).await?;
    Ok(Json(
        app.review(&id, &admin.username, &key, action, input)
            .await?,
    ))
}

#[handler]
pub async fn enrollment_approve(req: &mut Request, depot: &Depot) -> Result<Json<EnrollmentView>> {
    enrollment_review(req, depot, ReviewAction::Approve).await
}

#[handler]
pub async fn enrollment_request_changes(
    req: &mut Request,
    depot: &Depot,
) -> Result<Json<EnrollmentView>> {
    enrollment_review(req, depot, ReviewAction::RequestChanges).await
}

#[handler]
pub async fn enrollment_reject(req: &mut Request, depot: &Depot) -> Result<Json<EnrollmentView>> {
    enrollment_review(req, depot, ReviewAction::Reject).await
}

#[handler]
pub async fn issuance_retry(req: &mut Request, depot: &Depot) -> Result<Json<EnrollmentView>> {
    enrollment_review(req, depot, ReviewAction::RetryIssuance).await
}
