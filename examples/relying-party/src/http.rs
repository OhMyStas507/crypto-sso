use crate::{App, StoredChallenge};
use crypto_sso_verifier::wire;
use issuer_service::{
    error::{Error, Result},
    models::ApplicationSession,
    security::{digest, now},
};
use rand::rngs::OsRng;
use salvo::{
    http::cookie::SameSite,
    prelude::*,
    session::{SessionDepotExt, SessionHandler},
};
use salvo_csrf::{CsrfDepotExt, HeaderFinder};
use serde_json::{Value, json};

pub fn service(app: App, secret: &[u8], csrf_key: [u8; 32], secure: bool) -> Service {
    issuer_service::views::with_access_logging(router(app, secret, csrf_key, secure))
}

pub fn router(app: App, secret: &[u8], csrf_key: [u8; 32], secure: bool) -> Router {
    let sessions = SessionHandler::builder(app.http_sessions.with_namespace("rp/session"), secret)
        .cookie_name("crypto_sso_rp")
        .cookie_path("/api/v1/login")
        .same_site_policy(SameSite::Strict)
        .secure_cookie(secure)
        .session_ttl(Some(std::time::Duration::from_secs(app.session_ttl)))
        .build()
        .expect("validated RP session key");
    let login = Router::with_path("api/v1/login")
        .hoop(sessions)
        .hoop(check_origin)
        .hoop(salvo_csrf::hmac_session_csrf(
            csrf_key,
            HeaderFinder::new("x-csrf-token"),
        ))
        .push(Router::with_path("context").get(login_context))
        .push(Router::with_path("challenges").post(create_challenge))
        .push(Router::with_path("proofs").post(submit_proof))
        .push(
            Router::with_path("session")
                .get(current_session)
                .delete(logout),
        );
    Router::new()
        .hoop(affix_state::inject(app))
        .hoop(no_cache)
        .push(login)
}

fn app(depot: &Depot) -> App {
    depot
        .get_typed::<App>()
        .expect("application middleware")
        .clone()
}

#[handler]
async fn no_cache(res: &mut Response) {
    res.headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
}

#[handler]
async fn check_origin(req: &mut Request, depot: &Depot, res: &mut Response, ctrl: &mut FlowCtrl) {
    if !matches!(req.method().as_str(), "GET" | "HEAD" | "OPTIONS")
        && req.headers().get("origin").and_then(|v| v.to_str().ok())
            != Some(app(depot).verifier.origin().as_str())
    {
        res.render(Error::new(403, "ORIGIN_REJECTED", "应用请求来源不匹配"));
        ctrl.skip_rest();
    }
}

#[handler]
async fn login_context(depot: &Depot) -> Json<Value> {
    let app = app(depot);
    Json(
        json!({"csrf_token":depot.csrf_token(),"origin":app.verifier.origin().as_str(),"issuer_id":app.verifier.issuer_id().as_str(),"required_claims":app.required.iter().collect::<Vec<_>>() }),
    )
}

#[handler]
async fn create_challenge(depot: &mut Depot) -> Result<Json<Value>> {
    let app = app(depot);
    let (challenge, binding) = app
        .verifier
        .create_challenge(&app.required, now(), &mut OsRng)
        .map_err(|_| Error::invalid())?;
    let raw = String::from_utf8(wire::to_json(&challenge).map_err(|_| Error::invalid())?)
        .map_err(|_| Error::invalid())?;
    toasty::create!(StoredChallenge {
        id: challenge.nonce().as_str().to_owned(),
        raw: raw.clone(),
        consumed: false,
    })
    .exec(&mut app.store.database())
    .await?;
    depot
        .session_mut()
        .ok_or_else(Error::unavailable)?
        .insert("binding", binding.expose_cookie_value())?;
    Ok(Json(json!({"challenge_json":raw})))
}

#[handler]
async fn submit_proof(req: &mut Request, depot: &mut Depot) -> Result<Json<Value>> {
    let app = app(depot);
    // The verifier's strict wire parser receives the untouched body, including duplicate fields.
    let raw = req
        .payload_with_max_size(app.body_limit)
        .await
        .map_err(|_| Error::new(413, "BODY_TOO_LARGE", "证明超过请求体限制"))?
        .to_vec();
    let binding = depot
        .session()
        .and_then(|s| s.get::<String>("binding"))
        .ok_or_else(|| {
            Error::new(
                401,
                "BROWSER_BINDING_REQUIRED",
                "请在当前浏览器重新获取挑战",
            )
        })?;
    let result = app.verify_and_consume(&raw, &binding).await?;
    depot.session_mut().ok_or_else(Error::unavailable)?.insert(
        "auth_ticket",
        result["ticket"].as_str().ok_or_else(Error::unavailable)?,
    )?;
    Ok(Json(result["session"].clone()))
}

#[handler]
async fn current_session(depot: &Depot) -> Result<Json<Value>> {
    let ticket = depot
        .session()
        .and_then(|s| s.get::<String>("auth_ticket"))
        .ok_or_else(|| Error::new(401, "LOGIN_REQUIRED", "尚未登录应用"))?;
    let session = ApplicationSession::filter_by_id(digest(&ticket))
        .first()
        .exec(&mut app(depot).store.database())
        .await?
        .filter(|s| s.expires_at > now())
        .ok_or_else(|| Error::new(401, "LOGIN_REQUIRED", "应用会话已过期"))?;
    Ok(Json(serde_json::to_value(session)?))
}

#[handler]
async fn logout(depot: &mut Depot) -> Result<Json<Value>> {
    if let Some(ticket) = depot.session().and_then(|s| s.get::<String>("auth_ticket")) {
        ApplicationSession::filter_by_id(digest(&ticket))
            .delete()
            .exec(&mut app(depot).store.database())
            .await?;
    }
    depot
        .session_mut()
        .ok_or_else(Error::unavailable)?
        .destroy();
    Ok(Json(json!({"logged_out":true})))
}
