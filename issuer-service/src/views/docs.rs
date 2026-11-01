//! Generate documentation from the same annotated handlers mounted at runtime.
use salvo::Router;
use salvo_oapi::{
    OpenApi,
    security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme},
    swagger_ui::SwaggerUi,
};

pub(super) const OPENAPI_PATH: &str = "/api/docs/openapi.json";
pub(super) const SWAGGER_PATH: &str = "/api/docs/swagger-ui";

fn document(router: &Router) -> OpenApi {
    let mut doc =
        OpenApi::new("Crypto SSO Issuer API", env!("CARGO_PKG_VERSION")).merge_router(router);
    doc.info.description = Some(
        "签发中心 API。SDK 原始 JSON 对象由 SDK v1 严格解析。\n\n\
         公开申请接口使用 Authorization: Bearer <申请访问密钥>。\n\n\
         管理员和初始化分别使用独立的 HttpOnly Cookie。先调用同组 csrf 接口，\
         再把返回的 csrf_token 填入写接口的 x-csrf-token。浏览器自动管理 Cookie 和 Origin；\
         Swagger UI 所在 Origin 必须加入 ADMIN_ALLOWED_ORIGINS_JSON。\
         登录或验证会轮换会话，之后应重新获取 CSRF 令牌。\n\n\
         管理和初始化写操作的 idempotency-key 在同一次操作重试时保持不变。\
         分页 cursor 为非负整数偏移，next_cursor 字符串可直接用于下一次查询。"
            .into(),
    );
    doc.components = doc.components.add_security_scheme(
        "enrollment_access",
        SecurityScheme::Http(
            Http::new(HttpAuthScheme::Bearer).description("钱包为申请生成的访问密钥"),
        ),
    );
    for (scheme, cookie, description) in [
        (
            "admin_session",
            "crypto_sso_admin",
            "管理员登录后由浏览器自动携带",
        ),
        (
            "setup_session",
            "crypto_sso_setup",
            "验证部署初始化码后由浏览器自动携带",
        ),
        (
            "setup_receipt",
            "crypto_sso_setup_receipt",
            "创建最终提交操作后取得，十分钟有效",
        ),
    ] {
        doc.components = doc.components.add_security_scheme(
            scheme,
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                cookie,
                description,
            ))),
        );
    }
    doc
}

pub(super) fn attach(router: Router) -> Router {
    let doc = document(&router);
    // Outside /api/v1: documentation remains available before initialization,
    // and /api/ reverse proxies serve both JSON and the embedded UI assets.
    router.push(doc.into_router(OPENAPI_PATH)).push(
        SwaggerUi::new(OPENAPI_PATH)
            .title("Issuer API · Swagger UI")
            .into_router(SWAGGER_PATH),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use salvo::{
        http::StatusCode,
        test::{ResponseExt, TestClient},
    };
    use serde_json::{Value, json};
    use std::collections::BTreeSet;

    fn routes() -> Router {
        super::super::routes(Router::with_path("admin"), Router::with_path("setup"))
    }

    fn check_goals(router: &Router) -> usize {
        let own = router.goal.as_ref().map_or(0, |goal| {
            assert!(
                salvo_oapi::EndpointRegistry::find(&goal.type_id()).is_some(),
                "endpoint is missing #[endpoint(...)]"
            );
            1
        });
        own + router.routers().iter().map(check_goals).sum::<usize>()
    }

    fn check_refs(value: &Value, doc: &Value) {
        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                    assert!(
                        reference.starts_with("#/"),
                        "unexpected reference {reference}"
                    );
                    assert!(
                        doc.pointer(&reference[1..]).is_some(),
                        "unresolved {reference}"
                    );
                }
                for value in object.values() {
                    check_refs(value, doc);
                }
            }
            Value::Array(values) => {
                for value in values {
                    check_refs(value, doc);
                }
            }
            _ => (),
        }
    }

    #[test]
    fn documents_every_mounted_endpoint_with_unique_ids_and_valid_schema_refs() {
        let router = routes();
        let expected = [
            ("/health", "get"),
            ("/api/v1/service-info", "get"),
            ("/api/v1/claim-hints", "get"),
            ("/api/v1/enrollments", "post"),
            ("/api/v1/enrollments/{request_id}", "get"),
            ("/api/v1/enrollments/{request_id}/credential", "get"),
            ("/api/v1/enrollments/{request_id}/cancel", "post"),
            ("/api/v1/admin/csrf", "get"),
            ("/api/v1/admin/sessions", "post"),
            ("/api/v1/admin/session", "get"),
            ("/api/v1/admin/session", "delete"),
            ("/api/v1/admin/overview", "get"),
            ("/api/v1/admin/settings", "get"),
            ("/api/v1/admin/settings", "put"),
            ("/api/v1/admin/enrollments", "get"),
            ("/api/v1/admin/enrollments/{request_id}", "get"),
            ("/api/v1/admin/enrollments/{request_id}/approve", "post"),
            (
                "/api/v1/admin/enrollments/{request_id}/request-changes",
                "post",
            ),
            ("/api/v1/admin/enrollments/{request_id}/reject", "post"),
            (
                "/api/v1/admin/enrollments/{request_id}/retry-issuance",
                "post",
            ),
            ("/api/v1/admin/members", "get"),
            ("/api/v1/admin/members/{member_ref}", "get"),
            ("/api/v1/admin/issuances", "get"),
            ("/api/v1/admin/audit-events", "get"),
            ("/api/v1/setup/status", "get"),
            ("/api/v1/setup/csrf", "get"),
            ("/api/v1/setup/session", "post"),
            ("/api/v1/setup/session", "delete"),
            ("/api/v1/setup/draft", "get"),
            ("/api/v1/setup/draft/admin", "put"),
            ("/api/v1/setup/draft/service", "put"),
            ("/api/v1/setup/key-generation", "post"),
            ("/api/v1/setup/key-generation/{operation_id}", "get"),
            ("/api/v1/setup/operations", "post"),
            ("/api/v1/setup/operations/{operation_id}", "get"),
            ("/api/v1/setup/complete", "post"),
        ];
        assert_eq!(check_goals(&router), expected.len());
        let doc = serde_json::to_value(document(&router)).unwrap();
        let paths = doc["paths"].as_object().unwrap();
        let actual: BTreeSet<_> = paths
            .iter()
            .flat_map(|(path, methods)| {
                methods
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(move |method| (path.as_str(), method.as_str()))
            })
            .collect();
        assert_eq!(actual, expected.into_iter().collect());
        let mut ids = BTreeSet::new();
        for (path, method) in actual {
            let operation = &paths[path][method];
            assert!(
                ids.insert(operation["operationId"].as_str().unwrap()),
                "duplicate operation ID"
            );
            assert!(!operation["tags"].as_array().unwrap().is_empty());
            assert!(operation["responses"]["200"]["content"]["application/json"].is_object());
            for segment in path.split('/') {
                if let Some(name) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    assert!(operation["parameters"].as_array().unwrap().iter().any(|p| {
                        p["name"] == name && p["in"] == "path" && p["required"] == true
                    }));
                }
            }
        }
        check_refs(&doc, &doc);
    }

    #[test]
    fn documents_bodies_authentication_pagination_and_special_responses() {
        let doc = serde_json::to_value(document(&routes())).unwrap();
        let paths = &doc["paths"];
        for (path, method, body) in [
            ("/api/v1/enrollments", "post", "SubmitEnrollment"),
            (
                "/api/v1/enrollments/{request_id}/cancel",
                "post",
                "CancelEnrollmentRequest",
            ),
            ("/api/v1/admin/sessions", "post", "LoginRequest"),
            ("/api/v1/admin/settings", "put", "ServiceSettings"),
            (
                "/api/v1/admin/enrollments/{request_id}/approve",
                "post",
                "ReviewDecision",
            ),
            ("/api/v1/setup/session", "post", "SetupVerify"),
            ("/api/v1/setup/draft/admin", "put", "SetupAdminInput"),
            ("/api/v1/setup/draft/service", "put", "SetupServiceInput"),
            ("/api/v1/setup/key-generation", "post", "SetupKeyInput"),
            ("/api/v1/setup/complete", "post", "SetupCompleteInput"),
        ] {
            let request = &paths[path][method]["requestBody"];
            assert_eq!(request["required"], true, "{path}");
            let reference = request["content"]["application/json"]["schema"]["$ref"]
                .as_str()
                .unwrap();
            assert!(reference.ends_with(body), "{reference}");
        }
        assert_eq!(
            paths["/api/v1/enrollments"]["post"]["security"],
            json!([{"enrollment_access": []}])
        );
        let hints = &paths["/api/v1/claim-hints"]["get"]["responses"];
        assert!(hints["200"]["headers"]["etag"].is_object());
        assert!(hints["304"].is_object());
        assert!(hints["304"].get("content").is_none());
        assert!(paths["/api/v1/enrollments"]["post"]["responses"]["202"].is_object());
        let settings = &paths["/api/v1/admin/settings"]["put"];
        assert_eq!(settings["security"], json!([{"admin_session": []}]));
        for header in ["x-csrf-token", "Origin", "idempotency-key"] {
            assert!(
                settings["parameters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["name"] == header && p["in"] == "header" && p["required"] == true)
            );
        }
        for name in ["status", "q", "cursor"] {
            assert!(
                paths["/api/v1/admin/enrollments"]["get"]["parameters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["name"] == name && p["in"] == "query")
            );
        }
        for (scheme, cookie) in [
            ("admin_session", "crypto_sso_admin"),
            ("setup_session", "crypto_sso_setup"),
            ("setup_receipt", "crypto_sso_setup_receipt"),
        ] {
            assert_eq!(doc["components"]["securitySchemes"][scheme]["name"], cookie);
            assert_eq!(doc["components"]["securitySchemes"][scheme]["in"], "cookie");
        }
    }

    #[tokio::test]
    async fn serves_spec_swagger_and_embedded_assets_without_application_state() {
        // No database, initialized center, login or CSRF is needed to read docs.
        let service = salvo::Service::new(attach(routes()));
        let mut spec = TestClient::get(format!("http://localhost{OPENAPI_PATH}"))
            .send(&service)
            .await;
        assert_eq!(spec.status_code, Some(StatusCode::OK));
        let doc: Value = spec.take_json().await.unwrap();
        assert!(doc["paths"]["/api/v1/admin/settings"]["put"].is_object());
        let redirect = TestClient::get(format!("http://localhost{SWAGGER_PATH}"))
            .send(&service)
            .await;
        assert!(redirect.status_code.unwrap().is_redirection());
        assert_eq!(redirect.headers()["location"], format!("{SWAGGER_PATH}/"));
        for (asset, content_type) in [
            ("", "text/html"),
            ("swagger-ui.css", "text/css"),
            ("swagger-ui-bundle.js", "javascript"),
            ("swagger-ui-standalone-preset.js", "javascript"),
        ] {
            let mut response = TestClient::get(format!("http://localhost{SWAGGER_PATH}/{asset}"))
                .send(&service)
                .await;
            assert_eq!(response.status_code, Some(StatusCode::OK), "{asset}");
            assert!(
                response.headers()["content-type"]
                    .to_str()
                    .unwrap()
                    .contains(content_type)
            );
            let body = response.take_string().await.unwrap();
            assert!(!body.is_empty());
            if asset.is_empty() {
                assert!(body.contains(OPENAPI_PATH));
                assert!(body.contains("./swagger-ui-bundle.js"));
            }
        }
    }
}
