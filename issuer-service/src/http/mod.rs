//! Salvo owns route selection, middleware execution and response writing.
mod admin;
mod middleware;
mod public;

use crate::{
    application::Application,
    error::{Error, Result},
    session_store::PgSessionStore,
};
use salvo::{http::cookie::SameSite, prelude::*, session::SessionHandler};
use salvo_csrf::HeaderFinder;
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
        PgSessionStore {
            store: app.store.clone(),
            namespace: "session".into(),
        },
        app.config.session_secret.as_bytes(),
    )
    .cookie_name("crypto_sso_admin")
    .cookie_path("/api/v1/admin")
    .session_ttl(Some(std::time::Duration::from_secs(app.config.session_ttl)))
    .same_site_policy(SameSite::Strict)
    .secure_cookie(app.config.secure_cookie)
    .build()
    .expect("validated session configuration");
    let csrf =
        salvo_csrf::hmac_session_csrf(app.config.csrf_key, HeaderFinder::new("x-csrf-token"));

    let protected = Router::new()
        .hoop(middleware::require_admin)
        .push(
            Router::with_path("session")
                .get(admin::current_session)
                .delete(admin::logout),
        )
        .push(Router::with_path("overview").get(admin::overview))
        .push(
            Router::with_path("settings")
                .get(admin::get_settings)
                .put(admin::save_settings),
        )
        .push(
            Router::with_path("enrollments")
                .get(admin::list_enrollments)
                .push(
                    Router::with_path("{request_id}")
                        .get(admin::get_enrollment)
                        .push(Router::with_path("approve").post(admin::approve))
                        .push(Router::with_path("request-changes").post(admin::request_changes))
                        .push(Router::with_path("reject").post(admin::reject))
                        .push(Router::with_path("retry-issuance").post(admin::retry_issuance)),
                ),
        )
        .push(
            Router::with_path("members")
                .get(admin::list_members)
                .push(Router::with_path("{member_ref}").get(admin::get_member)),
        )
        .push(Router::with_path("issuances").get(admin::list_issuances))
        .push(Router::with_path("audit-events").get(admin::list_audit));

    let api = Router::with_path("api/v1")
        .hoop(middleware::request_policy)
        .push(Router::with_path("service-info").get(public::service_info))
        .push(Router::with_path("claim-hints").get(public::claim_hints))
        .push(
            Router::with_path("enrollments")
                .post(public::submit_enrollment)
                .push(
                    Router::with_path("{request_id}")
                        .get(public::enrollment_status)
                        .push(Router::with_path("credential").get(public::credential))
                        .push(Router::with_path("cancel").post(public::cancel_enrollment)),
                ),
        )
        .push(
            Router::with_path("admin")
                .hoop(sessions)
                .hoop(middleware::check_origin)
                .hoop(csrf)
                .push(Router::with_path("csrf").get(admin::csrf_token))
                .push(Router::with_path("sessions").post(admin::login))
                .push(protected),
        );

    Router::new()
        .hoop(affix_state::inject(app))
        .hoop(affix_state::inject(middleware::RateMap::default()))
        .push(Router::with_path("health").get(health))
        .push(api)
}

#[handler]
async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok"}))
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
