//! 初始化会话与管理员会话使用独立 Cookie、持久化命名空间及权限检查。
use super::{app, body, header, parameter};
use crate::{
    error::{Error, Result},
    security::now,
    session_store::PgSessionStore,
};
use crypto_sso_identity_api::*;
use salvo::{
    http::cookie::{Cookie, SameSite, time::Duration},
    prelude::*,
    session::{SessionDepotExt, SessionStore},
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub type VerificationRate = Arc<Mutex<(u64, u32)>>;

#[derive(Clone, Serialize, Deserialize)]
pub struct SetupGrant {
    pub draft_id: String,
    pub started_at: u64,
    pub last_seen: u64,
}

fn grant(depot: &Depot) -> Result<SetupGrant> {
    depot.get_typed::<SetupGrant>().cloned().map_err(|_| {
        Error::new(
            401,
            "SETUP_SESSION_EXPIRED",
            "初始化会话已过期，请重新验证部署权限",
        )
    })
}

#[handler]
pub async fn require_session(depot: &mut Depot, res: &mut Response, ctrl: &mut FlowCtrl) {
    match app(depot).initialized().await {
        Ok(true) => {
            if let Some(session) = depot.session_mut() {
                session.destroy();
            }
            res.render(Error::new(
                409,
                "ALREADY_INITIALIZED",
                "身份中心已完成初始化，请前往管理员登录",
            ));
            ctrl.skip_rest();
            return;
        }
        Ok(false) => (),
        Err(error) => {
            res.render(error);
            ctrl.skip_rest();
            return;
        }
    }
    let allowed = depot
        .session()
        .and_then(|s| s.get::<SetupGrant>("setup_grant"))
        .filter(|s| s.started_at + 7200 > now() && s.last_seen + 1800 > now());
    if let Some(mut grant) = allowed {
        grant.last_seen = now();
        if let Some(session) = depot.session_mut() {
            let _ = session.insert("setup_grant", &grant);
        }
        depot.insert_typed(grant);
    } else {
        if let Some(session) = depot.session_mut() {
            session.destroy();
        }
        res.render(Error::new(
            401,
            "SETUP_SESSION_EXPIRED",
            "初始化会话已过期，请重新验证部署权限",
        ));
        ctrl.skip_rest();
    }
}

#[handler]
pub async fn status(depot: &Depot) -> Result<Json<SetupStatus>> {
    Ok(Json(app(depot).setup_status().await?))
}

#[handler]
pub async fn verify(req: &mut Request, depot: &mut Depot) -> Result<Json<SetupDraft>> {
    let input: SetupVerify = body(req, 4096).await?;
    let app = app(depot);
    let draft_id = app.setup_verify(input.code).await?;
    if let Some(old) = depot.session().cloned() {
        PgSessionStore {
            store: app.store.clone(),
            namespace: "setup".into(),
        }
        .destroy_session(old)
        .await
        .map_err(|_| Error::unavailable())?;
    }
    let session = depot.session_mut().ok_or_else(Error::unavailable)?;
    session.regenerate();
    session.insert(
        "setup_grant",
        SetupGrant {
            draft_id: draft_id.clone(),
            started_at: now(),
            last_seen: now(),
        },
    )?;
    // 过期草稿仍保留验证后的会话，便于页面解释需要维护者显式处理。
    Ok(Json(app.setup_draft(&draft_id).await?))
}

#[handler]
pub async fn logout(depot: &mut Depot) -> Result<Json<serde_json::Value>> {
    depot
        .session_mut()
        .ok_or_else(Error::unavailable)?
        .destroy();
    Ok(Json(serde_json::json!({"logged_out":true})))
}

#[handler]
pub async fn draft(depot: &Depot) -> Result<Json<SetupDraft>> {
    Ok(Json(app(depot).setup_draft(&grant(depot)?.draft_id).await?))
}

#[handler]
pub async fn admin(req: &mut Request, depot: &Depot) -> Result<Json<SetupDraft>> {
    let input = body(req, 4096).await?;
    Ok(Json(
        app(depot)
            .setup_admin(
                &grant(depot)?.draft_id,
                input,
                &header(req, "idempotency-key"),
            )
            .await?,
    ))
}

#[handler]
pub async fn service(req: &mut Request, depot: &Depot) -> Result<Json<SetupDraft>> {
    let input = body(req, 16384).await?;
    Ok(Json(
        app(depot)
            .setup_service(
                &grant(depot)?.draft_id,
                input,
                &header(req, "idempotency-key"),
            )
            .await?,
    ))
}

#[handler]
pub async fn generate(req: &mut Request, depot: &Depot) -> Result<Json<SetupKeyView>> {
    let input = body(req, 4096).await?;
    Ok(Json(
        app(depot)
            .setup_generate(
                &grant(depot)?.draft_id,
                input,
                &header(req, "idempotency-key"),
            )
            .await?,
    ))
}

#[handler]
pub async fn key(req: &Request, depot: &Depot) -> Result<Json<SetupKeyView>> {
    Ok(Json(
        app(depot)
            .setup_key(&grant(depot)?.draft_id, &parameter(req, "operation_id")?)
            .await?,
    ))
}

#[handler]
pub async fn begin(
    req: &Request,
    depot: &Depot,
    res: &mut Response,
) -> Result<Json<SetupOperation>> {
    let app = app(depot);
    let (view, cookie) = app
        .setup_begin_submission(&grant(depot)?.draft_id, &header(req, "idempotency-key"))
        .await?;
    res.add_cookie(
        Cookie::build(("crypto_sso_setup_receipt", cookie))
            .path("/api/v1/setup/operations")
            .http_only(true)
            .secure(app.config.secure_cookie)
            .same_site(SameSite::Strict)
            .max_age(Duration::minutes(10))
            .build(),
    );
    Ok(Json(view))
}

#[handler]
pub async fn operation(req: &Request, depot: &Depot) -> Result<Json<SetupOperation>> {
    let cookie = req
        .cookie("crypto_sso_setup_receipt")
        .map(|c| c.value())
        .unwrap_or("");
    Ok(Json(
        app(depot)
            .setup_operation(&parameter(req, "operation_id")?, cookie)
            .await?,
    ))
}

#[handler]
pub async fn complete(req: &mut Request, depot: &mut Depot) -> Result<Json<SetupReceipt>> {
    let input = body(req, 4096).await?;
    let result = app(depot)
        .setup_complete(&grant(depot)?.draft_id, input)
        .await?;
    depot
        .session_mut()
        .ok_or_else(Error::unavailable)?
        .destroy();
    Ok(Json(result))
}

#[handler]
pub async fn verification_rate(depot: &Depot, res: &mut Response, ctrl: &mut FlowCtrl) {
    let allowed = {
        let mut rate = depot
            .get_typed::<VerificationRate>()
            .unwrap()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let minute = now() / 60;
        if rate.0 != minute {
            *rate = (minute, 0);
        }
        rate.1 = rate.1.saturating_add(1);
        rate.1 <= 100
    };
    if !allowed {
        res.headers_mut()
            .insert("retry-after", "60".parse().unwrap());
        res.render(Error::new(
            429,
            "RATE_LIMITED",
            "验证次数过多，请一分钟后重试",
        ));
        ctrl.skip_rest();
    }
}
