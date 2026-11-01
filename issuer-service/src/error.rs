use crypto_sso_identity_api::ApiError;

#[derive(Debug)]
pub struct Error {
    pub status: u16,
    pub code: &'static str,
    pub message: &'static str,
}

pub type Result<T> = std::result::Result<T, Error>;

impl salvo_oapi::EndpointOutRegister for Error {
    fn register(components: &mut salvo_oapi::Components, operation: &mut salvo_oapi::Operation) {
        use salvo_oapi::ToSchema;
        // All API groups can be rejected by policy, authentication or storage.
        for (status, description) in [
            ("400", "输入格式或字段不合法"),
            ("401", "访问令牌或会话无效、过期"),
            ("403", "来源、CSRF 校验或业务策略拒绝"),
            ("404", "记录不存在或无权访问"),
            ("409", "修订或当前业务状态冲突"),
            ("410", "初始化草稿已过期，需维护者显式处理"),
            ("413", "请求体超过限制或无法读取"),
            ("422", "字段未通过业务校验"),
            ("429", "请求频率超过限制"),
            ("503", "服务不可用或中心尚未初始化"),
        ] {
            operation.responses.insert(
                status,
                salvo_oapi::Response::new(description)
                    .add_content("application/json", ApiError::to_schema(components)),
            );
        }
    }
}

impl Error {
    pub fn new(status: u16, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
        }
    }
    pub fn invalid() -> Self {
        Self::new(400, "INVALID_REQUEST", "输入格式或字段不合法")
    }
    pub fn conflict() -> Self {
        Self::new(
            409,
            "REVISION_CONFLICT",
            "内容已更新或当前状态不允许此操作，请刷新后重试",
        )
    }
    pub fn not_found() -> Self {
        Self::new(404, "NOT_FOUND", "记录不存在或无权访问")
    }
    pub fn unavailable() -> Self {
        Self::new(
            503,
            "SERVICE_UNAVAILABLE",
            "服务暂时不可用，请保留原记录并稍后重试",
        )
    }
    pub fn body(&self) -> ApiError {
        ApiError {
            code: self.code.into(),
            message: self.message.into(),
            trace_id: crate::security::token(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)
    }
}
impl std::error::Error for Error {}
impl From<toasty::Error> for Error {
    fn from(error: toasty::Error) -> Self {
        // A PostgreSQL serialization failure aborts the entire native transaction.
        // Clients may refresh and retry the use case with the same idempotency key.
        if error.is_record_not_found() {
            Self::not_found()
        } else if error.is_serialization_failure() {
            tracing::debug!("database rejected a concurrent transaction");
            Self::conflict()
        } else {
            // Driver errors may contain SQL parameters or connection secrets.
            tracing::error!("database operation failed");
            Self::unavailable()
        }
    }
}
impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Self::invalid()
    }
}

impl salvo::Scribe for Error {
    fn render(self, res: &mut salvo::Response) {
        let body = self.body();
        // Log only the public error code and the same correlation ID returned
        // to the caller; request bodies, cookies and database errors stay out.
        if self.status >= 500 {
            tracing::error!(status = self.status, code = self.code, trace_id = %body.trace_id, "API request failed");
        } else if self.status == 409 {
            tracing::debug!(code = self.code, trace_id = %body.trace_id, "API state conflict");
        } else {
            tracing::warn!(status = self.status, code = self.code, trace_id = %body.trace_id, "API request rejected");
        }
        res.status_code(
            salvo::http::StatusCode::from_u16(self.status)
                .unwrap_or(salvo::http::StatusCode::INTERNAL_SERVER_ERROR),
        );
        res.render(salvo::prelude::Json(body));
    }
}
