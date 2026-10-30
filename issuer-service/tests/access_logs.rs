//! Exercise the same native Salvo logger used by both HTTP services, including
//! early rejections and routing misses. This test needs no database.
use issuer_service::{error::Error, http::with_access_logging};
use salvo::{prelude::*, test::TestClient};
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};
use tracing::instrument::WithSubscriber;

#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[handler]
async fn ok_response() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

#[handler]
async fn reject(res: &mut Response, ctrl: &mut FlowCtrl) {
    res.render(Error::new(401, "LOGIN_REQUIRED", "请登录"));
    ctrl.skip_rest();
}

#[handler]
async fn forbidden_response() -> Error {
    Error::new(403, "ORIGIN_REJECTED", "来源不匹配")
}

#[handler]
async fn failure() -> Error {
    Error::unavailable()
}

#[tokio::test]
async fn logs_each_api_path_at_info_without_request_secrets() {
    let logs = LogBuffer::default();
    let writer = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter("warn,salvo_extra::logging=info,issuer_service=info")
        .with_ansi(false)
        .without_time()
        .with_writer(move || writer.clone())
        .finish();

    async {
        let service = with_access_logging(
            Router::new()
                .push(Router::with_path("api/v1/service-info").get(ok_response))
                .push(
                    Router::with_path("api/v1/private")
                        .hoop(reject)
                        .get(ok_response),
                )
                .push(Router::with_path("api/v1/login/proofs").post(forbidden_response))
                .push(Router::with_path("api/v1/failure").get(failure)),
        );
        let public = TestClient::get("http://localhost/api/v1/service-info?token=QUERY_SECRET")
            .add_header("authorization", "Bearer BEARER_SECRET", true)
            .add_header("cookie", "session=COOKIE_SECRET", true)
            .send(&service);
        let denied = TestClient::get("http://localhost/api/v1/private").send(&service);
        let (public, denied) = tokio::join!(public, denied);
        assert_eq!(public.status_code, Some(StatusCode::OK));
        assert_eq!(denied.status_code, Some(StatusCode::UNAUTHORIZED));
        let forbidden = TestClient::post("http://localhost/api/v1/login/proofs")
            .body("PROOF_BODY_SECRET")
            .send(&service)
            .await;
        assert_eq!(forbidden.status_code, Some(StatusCode::FORBIDDEN));
        assert_eq!(
            TestClient::get("http://localhost/api/v1/missing")
                .send(&service)
                .await
                .status_code,
            Some(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            TestClient::post("http://localhost/api/v1/service-info")
                .send(&service)
                .await
                .status_code,
            Some(StatusCode::METHOD_NOT_ALLOWED)
        );
        assert_eq!(
            TestClient::get("http://localhost/api/v1/failure")
                .send(&service)
                .await
                .status_code,
            Some(StatusCode::SERVICE_UNAVAILABLE)
        );
    }
    .with_subscriber(subscriber)
    .await;

    let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    let access: Vec<_> = output
        .lines()
        .filter(|line| line.contains("salvo_extra::logging: Response"))
        .collect();
    assert_eq!(access.len(), 6, "{output}");
    for (method, path, status) in [
        ("GET", "/api/v1/service-info", 200),
        ("GET", "/api/v1/private", 401),
        ("POST", "/api/v1/login/proofs", 403),
        ("GET", "/api/v1/missing", 404),
        ("POST", "/api/v1/service-info", 405),
        ("GET", "/api/v1/failure", 503),
    ] {
        let line = access
            .iter()
            .find(|line| {
                line.contains(&format!("method={method}")) && line.contains(&format!("path={path}"))
            })
            .unwrap_or_else(|| panic!("missing {method} {path}: {output}"));
        assert!(
            line.contains("INFO")
                && line.contains(&format!("status={status}"))
                && line.contains("duration="),
            "{line}"
        );
    }
    assert!(output.contains("WARN") && output.contains("ERROR"));
    for secret in [
        "QUERY_SECRET",
        "BEARER_SECRET",
        "COOKIE_SECRET",
        "PROOF_BODY_SECRET",
    ] {
        assert!(!output.contains(secret), "request secret was logged");
    }
}
