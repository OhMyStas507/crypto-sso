use super::{app, body, header, parameter};
use crate::{
    domain::{AdminSession, audit},
    error::{Error, Result},
    security::{equal, now},
    session_store::PgSessionStore,
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use crypto_sso_identity_api::{ReviewDecision, ServiceSettings};
use salvo::{
    prelude::*,
    session::{SessionDepotExt, SessionStore},
};
use salvo_csrf::CsrfDepotExt;
use serde::Deserialize;
use serde_json::{Value, json};

#[handler]
pub async fn csrf_token(depot: &Depot) -> Json<Value> {
    Json(json!({"csrf_token":depot.csrf_token()}))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    username: String,
    password: String,
}

#[handler]
pub async fn login(req: &mut Request, depot: &mut Depot) -> Result<Json<Value>> {
    let app = app(depot);
    let input: Login = body(req, 4096).await?;
    let username = input.username;
    let password = zeroize::Zeroizing::new(input.password);
    let hash = app.config.admin_password_hash.clone();
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false);
    if !valid || !equal(&username, &app.config.admin_username) {
        return Err(Error::new(401, "INVALID_CREDENTIALS", "账号或密码错误"));
    }
    let admin = AdminSession {
        username,
        csrf_token: depot.csrf_token().unwrap_or_default().into(),
        expires_at: now() + app.config.session_ttl,
    };
    // Revoke the previous server-side record; Salvo rotates and signs the new cookie.
    if let Some(old) = depot.session().cloned() {
        PgSessionStore {
            store: app.store.clone(),
            namespace: "session".into(),
        }
        .destroy_session(old)
        .await
        .map_err(|_| Error::unavailable())?;
    }
    let session = depot.session_mut().ok_or_else(Error::unavailable)?;
    session.regenerate();
    session.insert("admin", &admin)?;
    audit(
        &mut app.store.database(),
        &admin.username,
        "ADMIN_LOGIN",
        "session",
    )
    .await?;
    tracing::info!("administrator authenticated");
    Ok(Json(json!(admin)))
}

#[handler]
pub async fn current_session(depot: &Depot) -> Json<Value> {
    let mut admin = depot.get_typed::<AdminSession>().unwrap().clone();
    admin.csrf_token = depot.csrf_token().unwrap_or_default().into();
    Json(json!(admin))
}
#[handler]
pub async fn logout(depot: &mut Depot) -> Result<Json<Value>> {
    depot
        .session_mut()
        .ok_or_else(Error::unavailable)?
        .destroy();
    Ok(Json(json!({"logged_out":true})))
}

async fn query(
    req: &Request,
    depot: &Depot,
    resource: &str,
    id: Option<String>,
) -> Result<Json<Value>> {
    Ok(Json(
        app(depot)
            .query(
                resource,
                id.as_deref(),
                req.query::<String>("status").as_deref(),
                req.query::<String>("q").as_deref(),
                req.query::<usize>("cursor").unwrap_or(0),
            )
            .await?,
    ))
}
#[handler]
pub async fn overview(req: &Request, depot: &Depot) -> Result<Json<Value>> {
    query(req, depot, "overview", None).await
}
#[handler]
pub async fn get_settings(req: &Request, depot: &Depot) -> Result<Json<Value>> {
    query(req, depot, "settings", None).await
}
#[handler]
pub async fn list_enrollments(req: &Request, depot: &Depot) -> Result<Json<Value>> {
    query(req, depot, "enrollments", None).await
}
#[handler]
pub async fn get_enrollment(req: &Request, depot: &Depot) -> Result<Json<Value>> {
    query(
        req,
        depot,
        "enrollments",
        Some(parameter(req, "request_id")?),
    )
    .await
}
#[handler]
pub async fn list_members(req: &Request, depot: &Depot) -> Result<Json<Value>> {
    query(req, depot, "members", None).await
}
#[handler]
pub async fn get_member(req: &Request, depot: &Depot) -> Result<Json<Value>> {
    query(req, depot, "members", Some(parameter(req, "member_ref")?)).await
}
#[handler]
pub async fn list_issuances(req: &Request, depot: &Depot) -> Result<Json<Value>> {
    query(req, depot, "issuances", None).await
}
#[handler]
pub async fn list_audit(req: &Request, depot: &Depot) -> Result<Json<Value>> {
    query(req, depot, "audit-events", None).await
}

#[handler]
pub async fn save_settings(req: &mut Request, depot: &Depot) -> Result<Json<Value>> {
    let app = app(depot);
    let admin = depot.get_typed::<AdminSession>().unwrap();
    let key = header(req, "idempotency-key");
    let input: ServiceSettings = body(req, app.config.body_limit).await?;
    Ok(Json(app.save_settings(&admin.username, &key, input).await?))
}
async fn review(req: &mut Request, depot: &Depot, action: &str) -> Result<Json<Value>> {
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
pub async fn approve(req: &mut Request, depot: &Depot) -> Result<Json<Value>> {
    review(req, depot, "approve").await
}
#[handler]
pub async fn request_changes(req: &mut Request, depot: &Depot) -> Result<Json<Value>> {
    review(req, depot, "request-changes").await
}
#[handler]
pub async fn reject(req: &mut Request, depot: &Depot) -> Result<Json<Value>> {
    review(req, depot, "reject").await
}
#[handler]
pub async fn retry_issuance(req: &mut Request, depot: &Depot) -> Result<Json<Value>> {
    review(req, depot, "retry-issuance").await
}
