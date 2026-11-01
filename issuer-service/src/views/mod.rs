//! View 层：Salvo 路由、HTTP 提取、会话适配与类型化响应渲染。
mod admin;
mod docs;
mod middleware;
mod public;
mod setup;

use crate::{
    error::{Error, Result},
    viewmodels::{Application, contracts::HealthResponse},
};
use salvo::{http::cookie::SameSite, prelude::*, session::SessionHandler};
use salvo_csrf::HeaderFinder;
use salvo_oapi::endpoint;
use serde::de::DeserializeOwned;

/// Apply Salvo's native logger before routing middleware so short-circuited
/// requests and unmatched paths/methods also produce an INFO access record.
pub fn with_access_logging(router: Router) -> Service {
    Service::new(router).hoop(salvo::logging::Logger::new())
}

pub fn service(app: Application) -> Service {
    with_access_logging(router(app))
}

pub fn router(app: Application) -> Router {
    let sessions = SessionHandler::builder(
        app.http_sessions.with_namespace("session"),
        &app.admin_secrets.session_secret,
    )
    .cookie_name("crypto_sso_admin")
    .cookie_path("/api/v1/admin")
    .session_ttl(Some(std::time::Duration::from_secs(app.config.session_ttl)))
    .same_site_policy(SameSite::Strict)
    .secure_cookie(app.config.secure_cookie)
    .build()
    .expect("validated session configuration");
    let csrf = salvo_csrf::hmac_session_csrf(
        app.admin_secrets.csrf_key,
        HeaderFinder::new("x-csrf-token"),
    );
    let setup_sessions = SessionHandler::builder(
        app.http_sessions.with_namespace("setup"),
        &app.admin_secrets.session_secret,
    )
    .cookie_name("crypto_sso_setup")
    .cookie_path("/api/v1/setup")
    .session_ttl(Some(std::time::Duration::from_secs(7200)))
    .same_site_policy(SameSite::Strict)
    .secure_cookie(app.config.secure_cookie)
    .build()
    .expect("validated setup session configuration");
    let setup_csrf = salvo_csrf::hmac_session_csrf(
        app.admin_secrets.csrf_key,
        HeaderFinder::new("x-csrf-token"),
    );
    let routes = routes(
        Router::with_path("admin")
            .hoop(sessions)
            .hoop(middleware::origin_check)
            .hoop(csrf),
        Router::with_path("setup")
            .hoop(setup_sessions)
            .hoop(middleware::origin_check)
            .hoop(setup_csrf),
    )
    .hoop(affix_state::inject(app))
    .hoop(affix_state::inject(setup::VerificationRate::default()));
    docs::attach(routes)
}

// Keep a single route tree for runtime and OpenAPI coverage tests. Session/CSRF
// middleware is supplied by the production builder above.
fn routes(admin: Router, setup: Router) -> Router {
    let setup_protected = Router::new()
        .hoop(setup::require_session)
        .push(
            Router::with_path("draft")
                .get(setup::draft)
                .push(Router::with_path("admin").put(setup::admin))
                .push(Router::with_path("service").put(setup::service)),
        )
        .push(
            Router::with_path("key-generation")
                .post(setup::generate)
                .push(Router::with_path("{operation_id}").get(setup::key)),
        )
        .push(Router::with_path("operations").post(setup::begin))
        .push(Router::with_path("complete").post(setup::complete));

    let protected = Router::new()
        .hoop(middleware::admin_session_require)
        .push(
            Router::with_path("session")
                .get(admin::admin_session_get)
                .delete(admin::admin_session_delete),
        )
        .push(Router::with_path("overview").get(admin::overview_get))
        .push(
            Router::with_path("settings")
                .get(admin::settings_get)
                .put(admin::settings_save),
        )
        .push(
            Router::with_path("enrollments")
                .get(admin::enrollments_list)
                .push(
                    Router::with_path("{request_id}")
                        .get(admin::enrollment_get)
                        .push(Router::with_path("approve").post(admin::enrollment_approve))
                        .push(
                            Router::with_path("request-changes")
                                .post(admin::enrollment_request_changes),
                        )
                        .push(Router::with_path("reject").post(admin::enrollment_reject))
                        .push(Router::with_path("retry-issuance").post(admin::issuance_retry)),
                ),
        )
        .push(
            Router::with_path("members")
                .get(admin::members_list)
                .push(Router::with_path("{member_ref}").get(admin::member_get)),
        )
        .push(Router::with_path("issuances").get(admin::issuances_list))
        .push(Router::with_path("audit-events").get(admin::audit_events_list));

    let api = Router::with_path("api/v1")
        .hoop(middleware::request_policy_apply)
        .hoop(middleware::center_ready)
        .push(
            setup
                .push(Router::with_path("status").get(setup::status))
                .push(Router::with_path("csrf").get(setup::csrf_token_get))
                .push(
                    Router::with_path("session")
                        .hoop(setup::verification_rate)
                        .post(setup::verify)
                        .delete(setup::logout),
                )
                .push(Router::with_path("operations/{operation_id}").get(setup::operation_get))
                .push(setup_protected),
        )
        .push(Router::with_path("service-info").get(public::service_info_get))
        .push(Router::with_path("claim-hints").get(public::claim_hints_get))
        .push(
            Router::with_path("enrollments")
                .post(public::enrollment_submit)
                .push(
                    Router::with_path("{request_id}")
                        .get(public::enrollment_status_get)
                        .push(Router::with_path("credential").get(public::credential_get))
                        .push(Router::with_path("cancel").post(public::enrollment_cancel)),
                ),
        )
        .push(
            admin
                .push(Router::with_path("csrf").get(admin::csrf_token_get))
                .push(Router::with_path("sessions").post(admin::admin_session_create))
                .push(protected),
        );

    Router::new()
        .push(Router::with_path("health").get(health_get))
        .push(api)
}

#[endpoint(tags("健康检查"), summary = "检查服务进程是否可响应")]
async fn health_get() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
    })
}

fn app(depot: &Depot) -> Application {
    depot
        .get_typed::<Application>()
        .expect("application middleware")
        .clone()
}
fn header(req: &Request, name: &str) -> String {
    req.headers()
        .get(name)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .into()
}
fn bearer(req: &Request) -> String {
    header(req, "authorization")
        .strip_prefix("Bearer ")
        .unwrap_or("")
        .into()
}
fn parameter(req: &Request, name: &str) -> Result<String> {
    req.param::<String>(name).ok_or_else(Error::invalid)
}
async fn body<T: DeserializeOwned>(req: &mut Request, limit: usize) -> Result<T> {
    let bytes = req
        .payload_with_max_size(limit)
        .await
        .map_err(|_| Error::new(413, "BODY_TOO_LARGE", "请求体超过限制或无法读取"))?;
    serde_json::from_slice(bytes).map_err(|_| Error::invalid())
}
