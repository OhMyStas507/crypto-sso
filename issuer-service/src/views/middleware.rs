use super::{app, header};
use crate::{error::Error, models::domain::AdminSession, security::now};
use salvo::{http::StatusCode, prelude::*, session::SessionDepotExt};

#[handler]
pub async fn center_ready(req: &Request, depot: &Depot, res: &mut Response, ctrl: &mut FlowCtrl) {
    if req.uri().path().starts_with("/api/v1/setup/") {
        return;
    }
    match app(depot).initialized().await {
        Ok(true) => (),
        Ok(false) => {
            res.render(Error::new(
                503,
                "CENTER_NOT_INITIALIZED",
                "身份中心尚未初始化",
            ));
            ctrl.skip_rest();
        }
        Err(error) => {
            res.render(error);
            ctrl.skip_rest();
        }
    }
}

#[handler]
pub async fn origin_check(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) -> () {
    if !matches!(req.method().as_str(), "GET" | "HEAD" | "OPTIONS")
        && !app(depot)
            .config
            .admin_allowed_origins
            .contains(&header(req, "origin"))
    {
        res.render(Error::new(403, "ORIGIN_REJECTED", "请求来源不匹配"));
        ctrl.skip_rest();
    }
}

#[handler]
pub async fn admin_session_require(
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) -> () {
    if let Some(admin) = depot
        .session()
        .and_then(|s| s.get::<AdminSession>("admin"))
        .filter(|s| s.expires_at > now())
    {
        depot.insert_typed(admin);
    } else {
        res.render(Error::new(
            401,
            "ADMIN_SESSION_REQUIRED",
            "请先登录管理后台",
        ));
        ctrl.skip_rest();
    }
}

#[handler]
pub async fn request_policy_apply(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) -> () {
    res.headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    res.headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    ctrl.call_next(req, depot, res).await;
    if res.status_code == Some(StatusCode::FORBIDDEN)
        && res
            .headers()
            .get("content-type")
            .is_none_or(|h| !h.to_str().unwrap_or("").contains("application/json"))
    {
        res.render(Error::new(
            403,
            "CSRF_REJECTED",
            "请求校验失败，请刷新页面后重试",
        ));
    }
}
