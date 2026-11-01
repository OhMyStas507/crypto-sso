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
use salvo_oapi::endpoint;

#[endpoint(
    tags("管理员"),
    summary = "取得管理员会话的 CSRF 令牌",
    responses((status_code = 503, description = "中心尚未初始化或服务不可用", body = crypto_sso_identity_api::ApiError))
)]
pub async fn csrf_token_get(depot: &Depot) -> Json<CsrfTokenResponse> {
    Json(CsrfTokenResponse {
        csrf_token: depot.csrf_token().map(str::to_owned),
    })
}

#[endpoint(
    tags("管理员"),
    summary = "登录并轮换管理员会话 Cookie",
    request_body = LoginRequest,
    parameters(("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"))
)]
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

#[endpoint(
    tags("管理员"),
    summary = "读取当前管理员会话",
    security(["admin_session" = []]),
    responses(
        (status_code = 401, description = "管理员会话无效或过期", body = crypto_sso_identity_api::ApiError),
        (status_code = 503, description = "中心尚未初始化或服务不可用", body = crypto_sso_identity_api::ApiError)
    )
)]
pub async fn admin_session_get(depot: &Depot) -> Json<AdminSession> {
    let mut admin = depot.get_typed::<AdminSession>().unwrap().clone();
    admin.csrf_token = depot.csrf_token().unwrap_or_default().into();
    Json(admin)
}

#[endpoint(
    tags("管理员"),
    summary = "注销管理员会话",
    parameters(("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送")),
    security(["admin_session" = []])
)]
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

#[endpoint(
    tags("管理员"),
    summary = "读取管理总览",
    security(["admin_session" = []])
)]
pub async fn overview_get(depot: &Depot) -> Result<Json<OverviewView>> {
    Ok(Json(app(depot).overview_get().await?))
}

#[endpoint(
    tags("管理员"),
    summary = "读取服务设置",
    security(["admin_session" = []])
)]
pub async fn settings_get(depot: &Depot) -> Result<Json<ServiceSettings>> {
    Ok(Json(app(depot).settings_get().await?))
}

#[endpoint(
    tags("管理员"),
    summary = "查询申请列表",
    parameters(("status" = Option<String>, Query, description = "状态筛选"), ("q" = Option<String>, Query, description = "搜索关键词"), ("cursor" = Option<usize>, Query, description = "非负整数偏移；默认 0，后续使用 next_cursor")),
    security(["admin_session" = []])
)]
pub async fn enrollments_list(
    req: &Request,
    depot: &Depot,
) -> Result<Json<Page<AdminEnrollmentView>>> {
    Ok(Json(app(depot).enrollments_list(list_query(req)).await?))
}

#[endpoint(
    tags("管理员"),
    summary = "读取申请详情与审核历史",
    parameters(("request_id" = String, Path, description = "申请 ID")),
    security(["admin_session" = []])
)]
pub async fn enrollment_get(req: &Request, depot: &Depot) -> Result<Json<AdminEnrollmentView>> {
    Ok(Json(
        app(depot)
            .enrollment_get(&parameter(req, "request_id")?)
            .await?,
    ))
}

#[endpoint(
    tags("管理员"),
    summary = "查询成员列表",
    parameters(("q" = Option<String>, Query, description = "搜索关键词"), ("cursor" = Option<usize>, Query, description = "非负整数偏移；默认 0，后续使用 next_cursor")),
    security(["admin_session" = []])
)]
pub async fn members_list(req: &Request, depot: &Depot) -> Result<Json<Page<MemberView>>> {
    Ok(Json(app(depot).members_list(list_query(req)).await?))
}

#[endpoint(
    tags("管理员"),
    summary = "读取成员及关联申请详情",
    parameters(("member_ref" = String, Path, description = "内部成员标识")),
    security(["admin_session" = []])
)]
pub async fn member_get(req: &Request, depot: &Depot) -> Result<Json<MemberDetailView>> {
    Ok(Json(
        app(depot)
            .member_get(&parameter(req, "member_ref")?)
            .await?,
    ))
}

#[endpoint(
    tags("管理员"),
    summary = "查询签发列表",
    parameters(("status" = Option<String>, Query, description = "状态筛选"), ("q" = Option<String>, Query, description = "搜索关键词"), ("cursor" = Option<usize>, Query, description = "非负整数偏移；默认 0，后续使用 next_cursor")),
    security(["admin_session" = []])
)]
pub async fn issuances_list(
    req: &Request,
    depot: &Depot,
) -> Result<Json<Page<AdminEnrollmentView>>> {
    Ok(Json(app(depot).issuances_list(list_query(req)).await?))
}

#[endpoint(
    tags("管理员"),
    summary = "查询审计事件",
    parameters(("cursor" = Option<usize>, Query, description = "非负整数偏移；默认 0，后续使用 next_cursor")),
    security(["admin_session" = []])
)]
pub async fn audit_events_list(req: &Request, depot: &Depot) -> Result<Json<Page<AuditEventView>>> {
    Ok(Json(app(depot).audit_events_list(list_query(req)).await?))
}

#[endpoint(
    tags("管理员"),
    summary = "保存服务设置",
    request_body = ServiceSettings,
    parameters(("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"), ("idempotency-key" = String, Header, description = "同一次操作的重试必须使用相同幂等键")),
    security(["admin_session" = []])
)]
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

#[endpoint(
    tags("管理员"),
    summary = "批准申请",
    request_body = ReviewDecision,
    parameters(("request_id" = String, Path, description = "申请 ID"), ("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"), ("idempotency-key" = String, Header, description = "同一次操作的重试必须使用相同幂等键")),
    security(["admin_session" = []])
)]
pub async fn enrollment_approve(req: &mut Request, depot: &Depot) -> Result<Json<EnrollmentView>> {
    enrollment_review(req, depot, ReviewAction::Approve).await
}

#[endpoint(
    tags("管理员"),
    summary = "要求修改申请",
    request_body = ReviewDecision,
    parameters(("request_id" = String, Path, description = "申请 ID"), ("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"), ("idempotency-key" = String, Header, description = "同一次操作的重试必须使用相同幂等键")),
    security(["admin_session" = []])
)]
pub async fn enrollment_request_changes(
    req: &mut Request,
    depot: &Depot,
) -> Result<Json<EnrollmentView>> {
    enrollment_review(req, depot, ReviewAction::RequestChanges).await
}

#[endpoint(
    tags("管理员"),
    summary = "拒绝申请",
    request_body = ReviewDecision,
    parameters(("request_id" = String, Path, description = "申请 ID"), ("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"), ("idempotency-key" = String, Header, description = "同一次操作的重试必须使用相同幂等键")),
    security(["admin_session" = []])
)]
pub async fn enrollment_reject(req: &mut Request, depot: &Depot) -> Result<Json<EnrollmentView>> {
    enrollment_review(req, depot, ReviewAction::Reject).await
}

#[endpoint(
    tags("管理员"),
    summary = "重试签发",
    request_body = ReviewDecision,
    parameters(("request_id" = String, Path, description = "申请 ID"), ("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"), ("idempotency-key" = String, Header, description = "同一次操作的重试必须使用相同幂等键")),
    security(["admin_session" = []])
)]
pub async fn issuance_retry(req: &mut Request, depot: &Depot) -> Result<Json<EnrollmentView>> {
    enrollment_review(req, depot, ReviewAction::RetryIssuance).await
}
