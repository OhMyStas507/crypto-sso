//! 初始化会话与管理员会话使用独立 Cookie、持久化命名空间及权限检查。
use super::{app, body, header, parameter};
use crate::{
    error::{Error, Result},
    security::now,
};
use crypto_sso_identity_api::*;
use salvo::{
    http::cookie::{Cookie, SameSite, time::Duration},
    prelude::*,
    session::{SessionDepotExt, SessionStore},
};
use salvo_oapi::endpoint;
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

#[endpoint(tags("初始化"), summary = "查询初始化状态")]
pub async fn status(depot: &Depot) -> Result<Json<SetupStatus>> {
    Ok(Json(app(depot).setup_status().await?))
}

#[endpoint(
    tags("初始化"),
    summary = "验证部署初始化码并建立初始化会话",
    request_body = SetupVerify,
    parameters(("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"))
)]
pub async fn verify(req: &mut Request, depot: &mut Depot) -> Result<Json<SetupDraft>> {
    let input: SetupVerify = body(req, 4096).await?;
    let app = app(depot);
    let draft_id = app.setup_verify(input.code).await?;
    if let Some(old) = depot.session().cloned() {
        app.http_sessions
            .with_namespace("setup")
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

#[endpoint(
    tags("初始化"),
    summary = "注销初始化会话",
    parameters(("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"))
)]
pub async fn logout(
    depot: &mut Depot,
) -> Result<Json<crate::viewmodels::contracts::LogoutResponse>> {
    depot
        .session_mut()
        .ok_or_else(Error::unavailable)?
        .destroy();
    Ok(Json(crate::viewmodels::contracts::LogoutResponse {
        logged_out: true,
    }))
}

#[endpoint(
    tags("初始化"),
    summary = "读取已验证会话的初始化草稿",
    security(["setup_session" = []])
)]
pub async fn draft(depot: &Depot) -> Result<Json<SetupDraft>> {
    Ok(Json(app(depot).setup_draft(&grant(depot)?.draft_id).await?))
}

#[endpoint(
    tags("初始化"),
    summary = "保存管理员草稿",
    request_body = SetupAdminInput,
    parameters(("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"), ("idempotency-key" = String, Header, description = "同一次操作的重试必须使用相同幂等键")),
    security(["setup_session" = []])
)]
pub async fn admin(req: &mut Request, depot: &Depot) -> Result<Json<SetupDraft>> {
    let input: SetupAdminInput = body(req, 4096).await?;
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

#[endpoint(
    tags("初始化"),
    summary = "保存公开服务信息草稿",
    request_body = SetupServiceInput,
    parameters(("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"), ("idempotency-key" = String, Header, description = "同一次操作的重试必须使用相同幂等键")),
    security(["setup_session" = []])
)]
pub async fn service(req: &mut Request, depot: &Depot) -> Result<Json<SetupDraft>> {
    let input: SetupServiceInput = body(req, 16384).await?;
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

#[endpoint(
    tags("初始化"),
    summary = "生成候选签发身份",
    request_body = SetupKeyInput,
    parameters(("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"), ("idempotency-key" = String, Header, description = "同一次操作的重试必须使用相同幂等键")),
    security(["setup_session" = []])
)]
pub async fn generate(req: &mut Request, depot: &Depot) -> Result<Json<SetupKeyView>> {
    let input: SetupKeyInput = body(req, 4096).await?;
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

#[endpoint(
    tags("初始化"),
    summary = "读取候选密钥生成状态",
    parameters(("operation_id" = String, Path, description = "初始化操作 ID")),
    security(["setup_session" = []])
)]
pub async fn key(req: &Request, depot: &Depot) -> Result<Json<SetupKeyView>> {
    Ok(Json(
        app(depot)
            .setup_key(&grant(depot)?.draft_id, &parameter(req, "operation_id")?)
            .await?,
    ))
}

#[endpoint(
    tags("初始化"),
    summary = "创建最终提交操作并设置十分钟有效的回执 Cookie",
    parameters(("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送"), ("idempotency-key" = String, Header, description = "同一次操作的重试必须使用相同幂等键")),
    security(["setup_session" = []])
)]
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

#[endpoint(
    tags("初始化"),
    summary = "通过回执 Cookie 查询最终提交结果",
    parameters(("operation_id" = String, Path, description = "初始化操作 ID")),
    security(["setup_receipt" = []])
)]
pub async fn operation_get(req: &Request, depot: &Depot) -> Result<Json<SetupOperation>> {
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

#[endpoint(
    tags("初始化"),
    summary = "完成初始化并销毁初始化会话",
    request_body = SetupCompleteInput,
    parameters(("x-csrf-token" = String, Header, description = "先调用本组 csrf 接口取得令牌；浏览器自动携带同组 Cookie"), ("Origin" = String, Header, description = "必须属于 ADMIN_ALLOWED_ORIGINS_JSON；浏览器自动发送")),
    security(["setup_session" = []])
)]
pub async fn complete(req: &mut Request, depot: &mut Depot) -> Result<Json<SetupReceipt>> {
    let input: SetupCompleteInput = body(req, 4096).await?;
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

/// 取得初始化会话的 CSRF 令牌。
#[endpoint(tags("初始化"))]
pub async fn csrf_token_get(
    depot: &Depot,
) -> Json<crate::viewmodels::contracts::CsrfTokenResponse> {
    use salvo_csrf::CsrfDepotExt;
    Json(crate::viewmodels::contracts::CsrfTokenResponse {
        csrf_token: depot.csrf_token().map(str::to_owned),
    })
}
