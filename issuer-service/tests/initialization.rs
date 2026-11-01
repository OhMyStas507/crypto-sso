//! 使用独立 PostgreSQL schema 验收空库初始化、HTTP 权限、密钥恢复与并发事务。
use crypto_sso_identity_api::*;
use issuer_service::{
    config::Config,
    models::*,
    security::{now, token},
    store::connect_database,
    viewmodels::Application,
    views, worker,
};
use reqwest::Client;
use salvo::prelude::*;
use serde_json::{Value, json};

struct Fixture {
    app: Application,
    config: Config,
    temp: tempfile::TempDir,
    admin: toasty::Db,
    schema: String,
}
impl Fixture {
    async fn new(origin: &str) -> Self {
        let _ = dotenvy::dotenv();
        let database =
            url::Url::parse(&std::env::var("TEST_DATABASE_URL").expect("需要 TEST_DATABASE_URL"))
                .unwrap();
        assert!(database.path().ends_with("_test"));
        assert!(!database.query_pairs().any(|(key, _)| key == "dbname"));
        let mut admin = connect_database(database.as_str()).await.unwrap();
        let schema = format!("initialization_test_{}", &token()[..16]);
        toasty::sql::statement(format!("CREATE SCHEMA {schema}"))
            .exec(&mut admin)
            .await
            .unwrap();
        let mut isolated = database.clone();
        isolated
            .query_pairs_mut()
            .append_pair("options", &format!("-c search_path={schema}"));
        let temp = tempfile::tempdir().unwrap();
        let config = Config {
            bind: "127.0.0.1:0".into(),
            database_url: isolated.to_string(),
            admin_allowed_origins: vec![origin.into()],
            setup_dir: temp.path().join("protected"),
            setup_storage_key: [29; 32],
            session_ttl: 3600,
            session_secret: token() + &token(),
            csrf_key: [8; 32],
            secure_cookie: false,
            worker_interval_ms: 1000,
            lease_seconds: 60,
            body_limit: 1048576,
        };
        let app = Application::bootstrap(config.clone()).await.unwrap();
        Self {
            app,
            config,
            temp,
            admin,
            schema,
        }
    }
    fn code(&self) -> String {
        let entry = std::fs::read_dir(&self.config.setup_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("initialization-code-")
            })
            .unwrap();
        std::fs::read_to_string(entry).unwrap()
    }
    async fn ready(&self) -> SetupDraft {
        let draft_id = self.app.setup_verify(self.code()).await.unwrap();
        let first = self.app.setup_draft(&draft_id).await.unwrap();
        let second = self
            .app
            .setup_admin(
                &draft_id,
                SetupAdminInput {
                    expected_revision: first.revision,
                    username: "admin".into(),
                    password: Some("correct password 123".into()),
                },
                &token(),
            )
            .await
            .unwrap();
        let third = self
            .app
            .setup_service(
                &draft_id,
                SetupServiceInput {
                    expected_revision: second.revision,
                    service: service(),
                },
                &token(),
            )
            .await
            .unwrap();
        self.app
            .setup_generate(
                &draft_id,
                SetupKeyInput {
                    expected_revision: third.revision,
                    organization: "验收组织".into(),
                },
                &token(),
            )
            .await
            .unwrap();
        self.app.setup_draft(&draft_id).await.unwrap()
    }
    async fn cleanup(mut self) {
        drop(self.app);
        toasty::sql::statement(format!("DROP SCHEMA {} CASCADE", self.schema))
            .exec(&mut self.admin)
            .await
            .unwrap();
        drop(self.temp);
    }
}
fn service() -> SetupService {
    SetupService {
        display_name: "验收中心".into(),
        public_base_url: "https://identity.example.org".into(),
        description: "初始化验收".into(),
        contact: "admin@example.org".into(),
    }
}
fn complete(draft: &SetupDraft, operation_id: String) -> SetupCompleteInput {
    SetupCompleteInput {
        operation_id,
        draft_id: draft.draft_id.clone(),
        expected_revision: draft.revision,
        key_id: draft.key.as_ref().unwrap().operation_id.clone(),
        identity_confirmed: true,
    }
}
async fn csrf(client: &Client, base: &str, namespace: &str) -> String {
    client
        .get(format!("{base}/api/v1/{namespace}/csrf"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["csrf_token"]
        .as_str()
        .unwrap()
        .into()
}
fn write(client: &Client, base: &str, path: &str, csrf: &str) -> reqwest::RequestBuilder {
    client
        .post(format!("{base}/api/v1/setup/{path}"))
        .header("origin", base)
        .header("x-csrf-token", csrf)
        .header("idempotency-key", token())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "需要独立 PostgreSQL TEST_DATABASE_URL"]
async fn empty_database_http_setup_and_normal_login() {
    let acceptor = TcpListener::new("127.0.0.1:0").bind().await;
    let base = format!("http://{}", acceptor.inner().local_addr().unwrap());
    let fixture = Fixture::new(&base).await;
    assert!(!fixture.app.initialized().await.unwrap());
    assert!(!worker::tick(&fixture.app).await.unwrap());
    let server = Server::new(acceptor);
    let handle = server.handle();
    let task = tokio::spawn(server.serve(views::service(fixture.app.clone())));
    let client = Client::builder().cookie_store(true).build().unwrap();
    assert_eq!(
        client
            .get(format!("{base}/api/v1/service-info"))
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    let status = client
        .get(format!("{base}/api/v1/setup/status"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(status.contains("setup_required"));
    assert!(!status.contains("username"));
    assert_eq!(
        client
            .get(format!("{base}/api/v1/setup/draft"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let mut protection = csrf(&client, &base, "setup").await;
    let code = fixture.code();
    assert_eq!(
        client
            .post(format!("{base}/api/v1/setup/session"))
            .header("x-csrf-token", &protection)
            .json(&json!({"code":code}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        write(&client, &base, "session", "wrong")
            .json(&json!({"code":code}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        write(&client, &base, "session", &protection)
            .json(&json!({"code":"wrong"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let draft = write(&client, &base, "session", &protection)
        .json(&json!({"code":code}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<SetupDraft>()
        .await
        .unwrap();
    // 将服务端会话设为过期，旧 Cookie 必须失去权限；轮换码后恢复同一份草稿。
    for mut row in WebSession::filter_by_namespace("setup")
        .exec(&mut fixture.app.store.database())
        .await
        .unwrap()
    {
        let mut session = row.data.0.clone();
        if session.get::<Value>("setup_grant").is_some() {
            session
                .insert(
                    "setup_grant",
                    json!({"draft_id":draft.draft_id,"started_at":now()-7201,"last_seen":now()}),
                )
                .unwrap();
            row.update()
                .data(session)
                .exec(&mut fixture.app.store.database())
                .await
                .unwrap();
        }
    }
    assert_eq!(
        client
            .get(format!("{base}/api/v1/setup/draft"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let path = fixture.app.ensure_setup_code(true, false).await.unwrap();
    let rotated = std::fs::read_to_string(path).unwrap();
    protection = csrf(&client, &base, "setup").await;
    assert_eq!(
        write(&client, &base, "session", &protection)
            .json(&json!({"code":code}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let restored = write(&client, &base, "session", &protection)
        .json(&json!({"code":rotated}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<SetupDraft>()
        .await
        .unwrap();
    assert_eq!(restored.draft_id, draft.draft_id);
    protection = csrf(&client, &base, "setup").await;
    let password = "correct password 123";
    let admin_key = token();
    let input = json!({"expected_revision":draft.revision,"username":"admin","password":password});
    let save = || {
        client
            .put(format!("{base}/api/v1/setup/draft/admin"))
            .header("origin", &base)
            .header("x-csrf-token", &protection)
            .header("idempotency-key", &admin_key)
            .json(&input)
    };
    let response = save()
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!response.contains(password));
    assert!(!response.contains("password_hash"));
    let second: SetupDraft = serde_json::from_str(&response).unwrap();
    assert_eq!(
        save()
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<SetupDraft>()
            .await
            .unwrap()
            .revision,
        second.revision
    );
    assert_eq!(client.put(format!("{base}/api/v1/setup/draft/admin")).header("origin",&base).header("x-csrf-token",&protection).header("idempotency-key",admin_key).json(&json!({"expected_revision":draft.revision,"username":"different","password":password})).send().await.unwrap().status(),409);
    let third = fixture
        .app
        .setup_service(
            &draft.draft_id,
            SetupServiceInput {
                expected_revision: second.revision,
                service: service(),
            },
            &token(),
        )
        .await
        .unwrap();
    let key_id = token();
    let key = fixture
        .app
        .setup_generate(
            &draft.draft_id,
            SetupKeyInput {
                expected_revision: third.revision,
                organization: "验收组织".into(),
            },
            &key_id,
        )
        .await
        .unwrap();
    assert_eq!(key.status, "READY");
    let ready = fixture.app.setup_draft(&draft.draft_id).await.unwrap();
    let second_instance = Application::bootstrap(fixture.config.clone())
        .await
        .unwrap();
    assert_eq!(
        second_instance
            .setup_draft(&draft.draft_id)
            .await
            .unwrap()
            .key
            .unwrap()
            .issuer_id,
        key.issuer_id
    );
    let pending = write(&client, &base, "operations", &protection)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<SetupOperation>()
        .await
        .unwrap();
    // 丢弃最终响应，单靠事先建立的只读回执仍能获取已提交结果。
    let response = write(&client, &base, "complete", &protection)
        .json(&complete(&ready, pending.operation_id))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    drop(response);
    let result = client
        .get(format!("{base}/api/v1/setup/operations/current"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<SetupOperation>()
        .await
        .unwrap();
    assert!(result.completed);
    assert_eq!(result.result.unwrap().username, "admin");
    assert_eq!(
        client
            .get(format!("{base}/api/v1/setup/draft"))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert!(second_instance.initialized().await.unwrap());
    assert_eq!(
        second_instance.info().await.unwrap().issuer_id,
        key.issuer_id.unwrap()
    );
    assert!(fixture.app.ensure_setup_code(true, false).await.is_err());
    assert_eq!(fixture.app.cleanup_setup_keys().await.unwrap(), 0);
    let info = fixture.app.info().await.unwrap();
    assert!(!info.settings.registration_enabled && !info.settings.issuance_enabled);
    let restarted = Application::bootstrap(fixture.config.clone())
        .await
        .unwrap();
    assert_eq!(restarted.info().await.unwrap().issuer_id, info.issuer_id);
    let admin_csrf = csrf(&client, &base, "admin").await;
    let login = client
        .post(format!("{base}/api/v1/admin/sessions"))
        .header("origin", &base)
        .header("x-csrf-token", &admin_csrf)
        .json(&json!({"username":"admin","password":password}))
        .send()
        .await
        .unwrap();
    assert!(login.status().is_success());
    assert_eq!(
        client
            .get(format!("{base}/api/v1/admin/session"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        Administrator::all()
            .exec(&mut fixture.app.store.database())
            .await
            .unwrap()
            .len(),
        1
    );
    let internal = Initialization::get_by_id(&mut fixture.app.store.database(), &1)
        .await
        .unwrap();
    assert!(internal.draft.password_hash.is_empty());
    handle.stop_graceful(None);
    task.await.unwrap();
    drop(restarted);
    drop(second_instance);
    fixture.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "需要独立 PostgreSQL TEST_DATABASE_URL"]
async fn key_failure_recovery_and_concurrent_completion() {
    let fixture = Fixture::new("http://localhost").await;
    let ready = fixture.ready().await;
    let key = ready.key.as_ref().unwrap();
    let path = fixture
        .config
        .setup_dir
        .join(format!("key-{}.enc", key.operation_id));
    let encrypted = std::fs::read(&path).unwrap();
    assert!(serde_json::from_slice::<Value>(&encrypted).is_err());
    let (first_op, _) = fixture
        .app
        .setup_begin_submission(&ready.draft_id, &token())
        .await
        .unwrap();
    let mut damaged = encrypted.clone();
    damaged[15] ^= 1;
    std::fs::write(&path, &damaged).unwrap();
    assert_eq!(
        fixture
            .app
            .setup_complete(
                &ready.draft_id,
                complete(&ready, first_op.operation_id.clone())
            )
            .await
            .unwrap_err()
            .code,
        "KEY_STORE_UNAVAILABLE"
    );
    assert!(!fixture.app.initialized().await.unwrap());
    assert!(
        Administrator::all()
            .exec(&mut fixture.app.store.database())
            .await
            .unwrap()
            .is_empty()
    );
    std::fs::write(&path, &encrypted).unwrap();
    // 模拟文件已写入、READY 尚未提交便崩溃，重试必须恢复原指纹。
    let mut row =
        InitializationKey::get_by_id(&mut fixture.app.store.database(), &key.operation_id)
            .await
            .unwrap();
    row.update()
        .status("FAILED")
        .lease_until(0)
        .public_json(None::<String>)
        .exec(&mut fixture.app.store.database())
        .await
        .unwrap();
    let recovered = fixture
        .app
        .setup_generate(
            &ready.draft_id,
            SetupKeyInput {
                expected_revision: 1,
                organization: key.organization.clone(),
            },
            &key.operation_id,
        )
        .await
        .unwrap();
    assert_eq!(recovered.issuer_id, key.issuer_id);
    assert_eq!(std::fs::read(&path).unwrap(), encrypted);
    let (second_op, _) = fixture
        .app
        .setup_begin_submission(&ready.draft_id, &token())
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        fixture
            .app
            .setup_complete(&ready.draft_id, complete(&ready, first_op.operation_id)),
        fixture
            .app
            .setup_complete(&ready.draft_id, complete(&ready, second_op.operation_id))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(
        a.as_ref()
            .err()
            .or(b.as_ref().err())
            .is_some_and(|e| e.status == 409)
    );
    assert_eq!(
        Administrator::all()
            .exec(&mut fixture.app.store.database())
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        Settings::all()
            .exec(&mut fixture.app.store.database())
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        AuditRecord::all()
            .exec(&mut fixture.app.store.database())
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(fixture.app.setup_draft(&ready.draft_id).await.is_err());
    std::fs::remove_file(&path).unwrap();
    assert!(
        Application::bootstrap(fixture.config.clone())
            .await
            .is_err()
    );
    assert!(
        Initialization::get_by_id(&mut fixture.app.store.database(), &1)
            .await
            .unwrap()
            .completed
    );
    std::fs::write(&path, encrypted).unwrap();
    fixture.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "需要独立 PostgreSQL TEST_DATABASE_URL"]
async fn expired_draft_requires_explicit_discard_and_residual_data_is_fenced() {
    let fixture = Fixture::new("http://localhost").await;
    let ready = fixture.ready().await;
    assert_eq!(
        fixture
            .app
            .ensure_setup_code(true, true)
            .await
            .unwrap_err()
            .code,
        "SETUP_DRAFT_ACTIVE"
    );
    let mut state = Initialization::get_by_id(&mut fixture.app.store.database(), &1)
        .await
        .unwrap();
    state
        .update()
        .draft_expires_at(now() - 1)
        .exec(&mut fixture.app.store.database())
        .await
        .unwrap();
    assert_eq!(
        fixture
            .app
            .setup_draft(&ready.draft_id)
            .await
            .unwrap_err()
            .code,
        "SETUP_DRAFT_EXPIRED"
    );
    let path = fixture.app.ensure_setup_code(true, true).await.unwrap();
    let id = fixture
        .app
        .setup_verify(std::fs::read_to_string(path).unwrap())
        .await
        .unwrap();
    assert_ne!(id, ready.draft_id);
    assert!(fixture.app.setup_draft(&id).await.unwrap().key.is_none());
    assert!(
        fixture
            .config
            .setup_dir
            .join(format!("key-{}.enc", ready.key.unwrap().operation_id))
            .exists()
    );
    assert_eq!(fixture.app.cleanup_setup_keys().await.unwrap(), 1);
    // 丢失控制记录但留下业务数据时，启动必须拒绝将其认定为空中心。
    Initialization::all()
        .delete()
        .exec(&mut fixture.app.store.database())
        .await
        .unwrap();
    toasty::create!(AuditRecord {
        id: token(),
        at: now(),
        actor: "original",
        action: "EXISTING_CENTER",
        target: "original"
    })
    .exec(&mut fixture.app.store.database())
    .await
    .unwrap();
    assert!(
        Application::bootstrap(fixture.config.clone())
            .await
            .is_err()
    );
    fixture.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "需要 PostgreSQL、已构建管理前端、Node.js 和 Playwright 浏览器"]
async fn real_browser_completes_setup_and_recovers_lost_response() {
    let acceptor = TcpListener::new("127.0.0.1:0").bind().await;
    let base = format!("http://{}", acceptor.inner().local_addr().unwrap());
    let fixture = Fixture::new(&base).await;
    let server = Server::new(acceptor);
    let handle = server.handle();
    let task = tokio::spawn(server.serve(views::service(fixture.app.clone())));
    let result = tokio::process::Command::new("node")
        .arg("scripts/check-setup-ui.mjs")
        .current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
        )
        .env("SETUP_TEST_URL", &base)
        .env("SETUP_TEST_CODE", fixture.code())
        .output()
        .await
        .unwrap();
    handle.stop_graceful(None);
    task.await.unwrap();
    assert!(
        result.status.success(),
        "浏览器验收失败：{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(fixture.app.initialized().await.unwrap());
    fixture.cleanup().await;
}
