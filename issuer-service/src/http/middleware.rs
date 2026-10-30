use super::{app, header};
use crate::{domain::AdminSession, error::Error, security::now};
use salvo::{http::StatusCode, prelude::*, session::SessionDepotExt};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub type RateMap = Arc<Mutex<HashMap<String, (u64, u32)>>>;

#[handler]
pub async fn check_origin(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
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
pub async fn require_admin(depot: &mut Depot, res: &mut Response, ctrl: &mut FlowCtrl) {
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
pub async fn request_policy(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    res.headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    res.headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    let limit = app(depot).config.rate_limit_per_minute;
    let allowed = {
        let mut rates = depot
            .get_typed::<RateMap>()
            .unwrap()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let minute = now() / 60;
        rates.retain(|_, (window, _)| *window == minute);
        let ip = req
            .remote_addr()
            .ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "unknown".into());
        let entry = rates.entry(ip).or_insert((minute, 0));
        entry.1 = entry.1.saturating_add(1);
        entry.1 <= limit
    };
    if !allowed {
        res.headers_mut()
            .insert("retry-after", "60".parse().unwrap());
        res.render(Error::new(429, "RATE_LIMITED", "请求过于频繁，请稍后重试"));
        ctrl.skip_rest();
        return;
    }
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
