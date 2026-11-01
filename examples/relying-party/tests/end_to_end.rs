//! Real PostgreSQL, Salvo HTTP, encrypted wallet and verifier acceptance tests.
//! Run only against a dedicated *_test database: cargo test -p crypto-sso-relying-party --test end_to_end -- --ignored
use crypto_sso_holder::{ClaimKeys, Claims, Holder, Origin, wire};
use crypto_sso_identity_api::{
    ClaimHint, EnrollmentStatus, EnrollmentView, Page, ReviewDecision, SubmitEnrollment,
};
use crypto_sso_relying_party::{App as RelyingParty, StoredChallenge};
use issuer_service::viewmodels::contracts::{
    AdminEnrollmentView, AuditEventView, ClaimHintsResponse, MemberDetailView, MemberView,
    OverviewView, ReviewAction,
};
use issuer_service::{
    config::Config,
    models::{Enrollment, Idempotency, Member, Settings},
    security::{now, token},
    store::{Store, connect_database},
    viewmodels::Application,
    worker,
};
use rand::rngs::OsRng;
use reqwest::Client;
use salvo::prelude::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use wallet_lib::engine::Engine;

fn submit(
    request: &crypto_sso_holder::EnrollmentRequest,
    revision: Option<u32>,
) -> SubmitEnrollment {
    SubmitEnrollment {
        enrollment_request: serde_json::value::RawValue::from_string(
            String::from_utf8(wire::to_json(request).unwrap()).unwrap(),
        )
        .unwrap(),
        claim_hints_revision: revision,
        application_note: Some("test application".into()),
    }
}
fn decision(e: &Enrollment, member: &str) -> ReviewDecision {
    ReviewDecision {
        expected_revision: e.view.revision,
        request_hash: e.view.request_hash.clone(),
        member_ref: Some(member.into()),
        evidence_source: Some("trusted roster".into()),
        verification_note: Some("all claims reviewed".into()),
        internal_note: None,
        approved_claims: Some(e.claims.0.clone()),
        public_reason: Some("Please verify your email".into()),
        claims_confirmed: true,
    }
}
async fn read(app: &Application, id: &str) -> Enrollment {
    Enrollment::get_by_request_id(&mut app.store.database(), &id.to_owned())
        .await
        .unwrap()
}

async fn http_get<T: serde::de::DeserializeOwned>(client: &Client, url: &str) -> T {
    client
        .get(url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires dedicated TEST_DATABASE_URL, TEST_REDIS_URL and Windows DPAPI"]
async fn registration_approval_wallet_recovery_and_offline_login() {
    let _ = dotenvy::dotenv();
    let root_database =
        url::Url::parse(&std::env::var("TEST_DATABASE_URL").expect("需要 TEST_DATABASE_URL"))
            .unwrap();
    assert!(root_database.path().ends_with("_test"));
    assert!(!root_database.query_pairs().any(|(key, _)| key == "dbname"));
    let mut admin_db = connect_database(root_database.as_str()).await.unwrap();
    let schema = format!("end_to_end_test_{}", &token()[..16]);
    toasty::sql::statement(format!("CREATE SCHEMA {schema}"))
        .exec(&mut admin_db)
        .await
        .unwrap();
    let mut isolated = root_database.clone();
    isolated
        .query_pairs_mut()
        .append_pair("options", &format!("-c search_path={schema}"));
    let database = isolated.to_string();
    let temporary = tempfile::tempdir().unwrap();
    let store = Store::connect(&database).await.unwrap();
    let acceptor = TcpListener::new("127.0.0.1:0").bind().await;
    let base = format!("http://{}", acceptor.inner().local_addr().unwrap());
    let password = token();
    let config = Config {
        bind: base.trim_start_matches("http://").into(),
        database_url: database.clone(),
        redis_url: std::env::var("TEST_REDIS_URL").expect("需要 TEST_REDIS_URL"),
        admin_allowed_origins: vec![base.clone()],
        setup_dir: temporary.path().join("setup"),
        setup_storage_key: [21; 32],
        session_ttl: 3600,
        secure_cookie: false,
        worker_interval_ms: 10,
        lease_seconds: 60,
        body_limit: 1048576,
    };
    // 注册验收同样从真实空库初始化开始，不预置管理员或导入签发私钥。
    let app = Application::bootstrap(config.clone()).await.unwrap();
    let code_path = app.ensure_setup_code(false, false).await.unwrap();
    let draft_id = app
        .setup_verify(std::fs::read_to_string(code_path).unwrap())
        .await
        .unwrap();
    let draft = app.setup_draft(&draft_id).await.unwrap();
    let draft = app
        .setup_admin(
            &draft_id,
            crypto_sso_identity_api::SetupAdminInput {
                expected_revision: draft.revision,
                username: "admin".into(),
                password: Some(password.clone()),
            },
            &token(),
        )
        .await
        .unwrap();
    let draft = app
        .setup_service(
            &draft_id,
            crypto_sso_identity_api::SetupServiceInput {
                expected_revision: draft.revision,
                service: crypto_sso_identity_api::SetupService {
                    display_name: "Integration identity center".into(),
                    public_base_url: base.clone(),
                    description: String::new(),
                    contact: String::new(),
                },
            },
            &token(),
        )
        .await
        .unwrap();
    let key = app
        .setup_generate(
            &draft_id,
            crypto_sso_identity_api::SetupKeyInput {
                expected_revision: draft.revision,
                organization: "Integration identity center".into(),
            },
            &token(),
        )
        .await
        .unwrap();
    let draft = app.setup_draft(&draft_id).await.unwrap();
    let (operation, _) = app
        .setup_begin_submission(&draft_id, &token())
        .await
        .unwrap();
    app.setup_complete(
        &draft_id,
        crypto_sso_identity_api::SetupCompleteInput {
            operation_id: operation.operation_id,
            draft_id: draft_id.clone(),
            expected_revision: draft.revision,
            key_id: key.operation_id,
            identity_confirmed: true,
        },
    )
    .await
    .unwrap();
    let issuer = app.issuer().await.unwrap();
    let server = Server::new(acceptor);
    let server_handle = server.handle();
    let server_task = tokio::spawn(server.serve(issuer_service::views::service(app.clone())));
    assert_eq!(
        Client::new()
            .get(format!("{base}/admin/login"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        Client::new()
            .get(format!("{base}/assets/index.js"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let browser = Client::builder().cookie_store(true).build().unwrap();
    let admin_url = format!("{base}/api/v1/admin");
    assert_eq!(
        browser
            .get(format!("{admin_url}/settings"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let csrf: Value = browser
        .get(format!("{admin_url}/csrf"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // Salvo's CSRF middleware rejects unsafe requests without a valid proof.
    assert_eq!(
        browser
            .post(format!("{admin_url}/sessions"))
            .header("origin", &base)
            .json(&json!({"username":"admin","password":password}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let response = browser
        .post(format!("{admin_url}/sessions"))
        .header("origin", &base)
        .header("x-csrf-token", csrf["csrf_token"].as_str().unwrap())
        .json(&json!({"username":"admin","password":password}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    let login: Value = response.json().await.unwrap();
    let csrf = login["csrf_token"].as_str().unwrap();
    assert_eq!(
        browser
            .put(format!("{admin_url}/settings"))
            .header("origin", "https://untrusted.example")
            .header("x-csrf-token", csrf)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );

    let mut s: crypto_sso_identity_api::ServiceSettings =
        Settings::get_by_id(&mut app.store.database(), &1)
            .await
            .unwrap()
            .into();
    s.registration_enabled = true;
    s.issuance_enabled = true;
    s.claim_hints = vec![ClaimHint {
        key: "role".into(),
        label: "角色".into(),
        description: "requires roster verification".into(),
        default_value: Some("member".into()),
        example_value: None,
        include_by_default: true,
    }];
    let key = token();
    let saved = browser
        .put(format!("{admin_url}/settings"))
        .header("origin", &base)
        .header("x-csrf-token", csrf)
        .header("idempotency-key", &key)
        .json(&s)
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), 200);
    assert_eq!(
        app.save_settings("admin", &key, s.clone())
            .await
            .unwrap()
            .revision,
        2
    );
    assert_eq!(
        app.save_settings("admin", &token(), s.clone())
            .await
            .unwrap_err()
            .status,
        409
    );
    let restarted = Application::bootstrap(config.clone()).await.unwrap();
    assert!(
        restarted
            .info()
            .await
            .unwrap()
            .settings
            .registration_enabled
    );

    let hints_url = format!("{base}/api/v1/claim-hints");
    let hints_response = browser.get(&hints_url).send().await.unwrap();
    assert_eq!(hints_response.status(), 200);
    let etag = hints_response.headers()["etag"].clone();
    let hints: ClaimHintsResponse = hints_response.json().await.unwrap();
    assert_eq!(hints.revision, 2);
    assert_eq!(hints.claims[0].key, "role");
    assert!(hints.allow_custom_claims);
    let cached = browser
        .get(&hints_url)
        .header("if-none-match", etag)
        .send()
        .await
        .unwrap();
    assert_eq!(cached.status(), 304);
    assert!(cached.bytes().await.unwrap().is_empty());

    let wallet_directory = temporary.path().join("wallet");
    let engine = Engine::new(wallet_directory.clone(), 5, 1048576).unwrap();
    engine
        .command("initialize_wallet", json!({}))
        .await
        .unwrap();
    let info = app.info().await.unwrap();
    assert!(
        engine
            .command(
                "confirm_center",
                json!({"url":base,"expected_issuer_id":"0".repeat(64)})
            )
            .await
            .is_err()
    );
    let center = engine
        .command(
            "confirm_center",
            json!({"url":base,"expected_issuer_id":info.issuer_id}),
        )
        .await
        .unwrap();
    let center_id = center["id"].as_str().unwrap();
    let mut draft = engine
        .command("create_enrollment_draft", json!({"center_id":center_id}))
        .await
        .unwrap();
    // Custom attributes and explicit empty strings remain unchanged; defaults are advisory.
    draft["rows"] = json!([["role", "member"], ["name", " Alice "], ["custom.note", ""]]);
    engine
        .command("save_enrollment_draft", draft.clone())
        .await
        .unwrap();
    // Check live registration settings even when the wallet cached an open center.
    let mut settings = app.info().await.unwrap().settings;
    settings.registration_enabled = false;
    app.save_settings("admin", &token(), settings)
        .await
        .unwrap();
    assert_eq!(
        engine
            .command("submit_enrollment", json!({"draft_id":draft["id"]}))
            .await
            .unwrap_err(),
        "REGISTRATION_CLOSED"
    );
    engine.command("lock_wallet", json!({})).await.unwrap();
    engine.command("unlock_wallet", json!({})).await.unwrap();
    assert_eq!(
        engine
            .command("get_enrollment_draft", json!({"id":draft["id"]}))
            .await
            .unwrap(),
        draft
    );
    assert_eq!(
        engine.command("list_enrollments", json!({})).await.unwrap(),
        json!([])
    );
    engine
        .command("refresh_claim_hints", json!({"id":center_id}))
        .await
        .unwrap();
    // Reopening must work even while the cached registration setting remains closed.
    let mut settings = app.info().await.unwrap().settings;
    settings.registration_enabled = true;
    app.save_settings("admin", &token(), settings)
        .await
        .unwrap();
    let submitted = engine
        .command("submit_enrollment", json!({"draft_id":draft["id"]}))
        .await
        .unwrap();
    let id = submitted["id"].as_str().unwrap();
    assert_eq!(submitted["view"]["status"], "PENDING_REVIEW");
    assert!(submitted.get("access").is_none() && submitted.get("pending").is_none());
    assert_eq!(
        browser
            .get(format!("{base}/api/v1/enrollments/{id}"))
            .bearer_auth(token())
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    engine.command("lock_wallet", json!({})).await.unwrap();
    assert!(engine.command("list_credentials", json!({})).await.is_err());
    drop(engine);
    let engine = Engine::new(wallet_directory.clone(), 5, 1048576).unwrap();
    engine.command("unlock_wallet", json!({})).await.unwrap();
    let e = read(&app, id).await;
    let approval = decision(&e, "member-1");
    let key = token();
    let approved = app
        .review(id, "admin", &key, ReviewAction::Approve, approval.clone())
        .await
        .unwrap();
    assert_eq!(approved.status, EnrollmentStatus::Approved);
    assert_eq!(
        serde_json::to_value(
            app.review(id, "admin", &key, ReviewAction::Approve, approval.clone())
                .await
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(&approved).unwrap()
    );
    assert_eq!(
        app.review(id, "admin", &token(), ReviewAction::Approve, approval)
            .await
            .unwrap_err()
            .status,
        409
    );
    let (left, right) = tokio::join!(worker::tick(&app), worker::tick(&restarted));
    assert_ne!(
        left.unwrap(),
        right.unwrap(),
        "exactly one worker owns the lease"
    );
    let issued = read(&app, id).await;
    assert_eq!(issued.view.status, EnrollmentStatus::Issued);
    // Exercise each typed admin read endpoint and preserve the original JSON projection.
    let overview: OverviewView = http_get(&browser, &format!("{admin_url}/overview")).await;
    assert_eq!(overview.issued, 1);
    assert_eq!(overview.recent[0].view.request_id, id);
    let detail: AdminEnrollmentView =
        http_get(&browser, &format!("{admin_url}/enrollments/{id}")).await;
    assert_eq!(
        serde_json::to_value(&detail).unwrap(),
        json!({
            "view": issued.view, "claims": issued.claims, "application_note": issued.application_note,
            "claim_hints_revision": issued.claim_hints_revision, "hints_snapshot": issued.hints_snapshot,
            "decision": issued.decision, "reviewer": issued.reviewer, "reviewed_at": issued.reviewed_at,
            "issued_at": issued.issued_at, "failure_code": issued.failure_code, "history": issued.history,
        })
    );
    let serialized = serde_json::to_string(&detail).unwrap();
    for secret in [
        "access_hash",
        "metadata_hash",
        "raw_request",
        "blind_credential",
        "lease_until",
    ] {
        assert!(!serialized.contains(secret));
    }
    for resource in ["enrollments", "issuances"] {
        let page: Page<AdminEnrollmentView> = http_get(
            &browser,
            &format!("{admin_url}/{resource}?status=ISSUED&q=Alice"),
        )
        .await;
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].view.request_id, id);
        assert!(page.next_cursor.is_none());
    }
    let empty: Page<AdminEnrollmentView> =
        http_get(&browser, &format!("{admin_url}/enrollments?status=UNKNOWN")).await;
    assert_eq!(empty.total, 0);
    let beyond: Page<AdminEnrollmentView> = http_get(
        &browser,
        &format!("{admin_url}/enrollments?cursor={}", usize::MAX),
    )
    .await;
    assert!(beyond.items.is_empty());
    assert!(beyond.next_cursor.is_none());
    let members: Page<MemberView> =
        http_get(&browser, &format!("{admin_url}/members?q=member-1")).await;
    assert_eq!(members.total, 1);
    assert!(members.items[0].issued);
    let member: MemberDetailView =
        http_get(&browser, &format!("{admin_url}/members/member-1")).await;
    assert_eq!(member.enrollment.view.request_id, id);
    assert_eq!(member.member.member_ref, "member-1");
    let audit: Page<AuditEventView> =
        http_get(&browser, &format!("{admin_url}/audit-events")).await;
    assert!(
        audit
            .items
            .iter()
            .any(|event| event.action == "CREDENTIAL_ISSUED" && event.target == id)
    );
    let raw = issued.blind_credential.clone().unwrap();
    worker::tick(&app).await.unwrap();
    assert_eq!(
        read(&app, id).await.blind_credential.as_deref(),
        Some(raw.as_str())
    );
    engine.sync(id).await.unwrap();
    let credentials = engine.command("list_credentials", json!({})).await.unwrap();
    assert_eq!(credentials.as_array().unwrap().len(), 1);
    assert_eq!(credentials[0]["claims"]["name"], " Alice ");
    assert!(credentials[0].get("holder").is_none());
    let ciphertext = std::fs::read_to_string(wallet_directory.join("vault.enc")).unwrap();
    assert!(!ciphertext.contains(" Alice ") && !ciphertext.contains("pending"));

    // SDK parsing must reject duplicate inner fields before an ordinary JSON object can swallow them.
    let (_, request) =
        Holder::begin_enrollment(issuer.public(), &Claims::empty(), &mut OsRng).unwrap();
    let request_json = String::from_utf8(wire::to_json(&request).unwrap()).unwrap();
    let malformed = request_json.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
    let body = format!(
        "{{\"enrollment_request\":{malformed},\"claim_hints_revision\":null,\"application_note\":null}}"
    );
    assert_eq!(
        browser
            .post(format!("{base}/api/v1/enrollments"))
            .bearer_auth(token())
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let access = token();
    let (created, empty) = app.submit(submit(&request, None), &access).await.unwrap();
    assert!(created);
    assert!(!app.submit(submit(&request, None), &access).await.unwrap().0);
    let mut changed = submit(&request, None);
    changed.application_note = Some("changed metadata".into());
    assert_eq!(app.submit(changed, &access).await.unwrap_err().status, 409);
    assert!(app.submit(submit(&request, None), &token()).await.is_err());

    // One trusted member cannot receive another credential, even from another process.
    let e = read(&app, &empty.request_id).await;
    assert_eq!(
        restarted
            .review(
                &empty.request_id,
                "admin",
                &token(),
                ReviewAction::Approve,
                decision(&e, "member-1")
            )
            .await
            .unwrap_err()
            .code,
        "MEMBER_ALREADY_RESERVED"
    );
    // Competing approval and cancellation allow exactly one state transition.
    let approval = decision(&e, "member-2");
    let approve_key = token();
    let (approve, cancel) = tokio::join!(
        app.review(
            &empty.request_id,
            "admin",
            &approve_key,
            ReviewAction::Approve,
            approval
        ),
        restarted.enrollment_cancel(&empty.request_id, &access, 1)
    );
    assert_ne!(approve.is_ok(), cancel.is_ok());

    let (_, request) =
        Holder::begin_enrollment(issuer.public(), &Claims::empty(), &mut OsRng).unwrap();
    let cancel_access = token();
    let (_, cancellable) = app
        .submit(submit(&request, None), &cancel_access)
        .await
        .unwrap();
    let cancel_url = format!(
        "{base}/api/v1/enrollments/{}/cancel",
        cancellable.request_id
    );
    let cancelled = browser
        .post(&cancel_url)
        .bearer_auth(&cancel_access)
        .json(&json!({"expected_revision": 1}))
        .send()
        .await
        .unwrap();
    assert_eq!(cancelled.status(), 200);
    let cancelled: EnrollmentView = cancelled.json().await.unwrap();
    assert_eq!(cancelled.status, EnrollmentStatus::Cancelled);
    assert_eq!(cancelled.revision, 2);
    assert_eq!(
        browser
            .post(&cancel_url)
            .bearer_auth(&cancel_access)
            .json(&json!({"expected_revision": 1}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );

    // Stale signing leases are fenced out and become explicitly retryable.
    let (_, request) =
        Holder::begin_enrollment(issuer.public(), &Claims::empty(), &mut OsRng).unwrap();
    let (_, view) = app
        .submit(submit(&request, Some(999)), &token())
        .await
        .unwrap();
    let entry = read(&app, &view.request_id).await;
    app.review(
        &view.request_id,
        "admin",
        &token(),
        ReviewAction::Approve,
        decision(&entry, "member-3"),
    )
    .await
    .unwrap();
    let mut db = app.store.database();
    let mut tx = db.transaction().await.unwrap();
    let mut e = Enrollment::get_by_request_id(&mut tx, &view.request_id)
        .await
        .unwrap();
    e.view.status = EnrollmentStatus::Signing;
    let signing_view = e.view.clone();
    e.update()
        .view(signing_view)
        .generation(1)
        .lease_until(now() - 1)
        .exec(&mut tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    worker::tick(&app).await.unwrap();
    assert_eq!(
        read(&app, &view.request_id).await.view.status,
        EnrollmentStatus::IssueFailed
    );

    // Zero and one attribute are valid complete sets, alongside the three-claim wallet path above.
    for (index, claims) in [Claims::empty(), Claims::new([("name", "")]).unwrap()]
        .into_iter()
        .enumerate()
    {
        let (pending, request) =
            Holder::begin_enrollment(issuer.public(), &claims, &mut OsRng).unwrap();
        let (_, view) = app.submit(submit(&request, None), &token()).await.unwrap();
        let entry = read(&app, &view.request_id).await;
        app.review(
            &view.request_id,
            "admin",
            &token(),
            ReviewAction::Approve,
            decision(&entry, &format!("small-set-{index}")),
        )
        .await
        .unwrap();
        for _ in 0..5 {
            worker::tick(&app).await.unwrap();
            if read(&app, &view.request_id).await.view.status == EnrollmentStatus::Issued {
                break;
            }
        }
        let response = read(&app, &view.request_id).await.blind_credential.unwrap();
        let credential = pending
            .finish(&wire::from_json(response.as_bytes()).unwrap())
            .unwrap();
        assert_eq!(credential.claims(), &claims);
    }

    // A structurally valid request with an invalid knowledge proof never gets a deliverable signature.
    let (_, mut invalid) =
        Holder::begin_enrollment(issuer.public(), &Claims::empty(), &mut OsRng).unwrap();
    invalid.response_secret = "00".repeat(32);
    let (_, invalid_view) = app.submit(submit(&invalid, None), &token()).await.unwrap();
    let invalid_entry = read(&app, &invalid_view.request_id).await;
    app.review(
        &invalid_view.request_id,
        "admin",
        &token(),
        ReviewAction::Approve,
        decision(&invalid_entry, "invalid-proof-member"),
    )
    .await
    .unwrap();
    for _ in 0..5 {
        worker::tick(&app).await.unwrap();
        if read(&app, &invalid_view.request_id).await.view.status
            == EnrollmentStatus::InvalidRequest
        {
            break;
        }
    }
    let failed = read(&app, &invalid_view.request_id).await;
    assert_eq!(failed.view.status, EnrollmentStatus::InvalidRequest);
    assert!(failed.blind_credential.is_none());

    for action in [ReviewAction::RequestChanges, ReviewAction::Reject] {
        let (_, request) =
            Holder::begin_enrollment(issuer.public(), &Claims::empty(), &mut OsRng).unwrap();
        let (_, view) = app.submit(submit(&request, None), &token()).await.unwrap();
        let entry = read(&app, &view.request_id).await;
        app.review(
            &view.request_id,
            "admin",
            &token(),
            action,
            decision(&entry, "unused"),
        )
        .await
        .unwrap();
        let terminal = read(&app, &view.request_id).await;
        assert_eq!(
            terminal.view.public_reason.as_deref(),
            Some("Please verify your email")
        );
        assert_eq!(terminal.history.len(), 1);
        assert!(
            app.review(
                &view.request_id,
                "admin",
                &token(),
                ReviewAction::Approve,
                decision(&terminal, "unused")
            )
            .await
            .is_err()
        );
    }

    // Two distinct applications compete for the same previously absent member
    // reservation. Losing the native transaction must leave no partial approval.
    let (_, request_a) =
        Holder::begin_enrollment(issuer.public(), &Claims::empty(), &mut OsRng).unwrap();
    let (_, request_b) =
        Holder::begin_enrollment(issuer.public(), &Claims::empty(), &mut OsRng).unwrap();
    let (_, view_a) = app
        .submit(submit(&request_a, None), &token())
        .await
        .unwrap();
    let (_, view_b) = app
        .submit(submit(&request_b, None), &token())
        .await
        .unwrap();
    let decision_a = decision(&read(&app, &view_a.request_id).await, "competing-member");
    let decision_b = decision(&read(&app, &view_b.request_id).await, "competing-member");
    let key_a = token();
    let key_b = token();
    let (result_a, result_b) = tokio::join!(
        app.review(
            &view_a.request_id,
            "admin",
            &key_a,
            ReviewAction::Approve,
            decision_a
        ),
        restarted.review(
            &view_b.request_id,
            "admin",
            &key_b,
            ReviewAction::Approve,
            decision_b
        ),
    );
    assert_ne!(result_a.is_ok(), result_b.is_ok());
    let (winner, loser, loser_key, error) = match (result_a, result_b) {
        (Ok(_), Err(error)) => (&view_a, &view_b, &key_b, error),
        (Err(error), Ok(_)) => (&view_b, &view_a, &key_a, error),
        _ => unreachable!(),
    };
    assert_eq!(error.status, 409);
    let member = Member::get_by_member_ref(&mut store.database(), &"competing-member".to_owned())
        .await
        .unwrap();
    assert_eq!(member.request_id, winner.request_id);
    let unchanged = read(&app, &loser.request_id).await;
    assert_eq!(unchanged.view.status, EnrollmentStatus::PendingReview);
    assert_eq!(unchanged.view.revision, 1);
    assert!(unchanged.decision.is_none() && unchanged.history.is_empty());
    assert!(
        Idempotency::filter_by_key(loser_key.clone())
            .first()
            .exec(&mut store.database())
            .await
            .unwrap()
            .is_none()
    );

    // Persisted Salvo sessions survive a store reconnect, then logout revokes them.
    assert_eq!(
        browser
            .get(format!("{admin_url}/session"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        browser
            .delete(format!("{admin_url}/session"))
            .header("origin", &base)
            .header("x-csrf-token", csrf)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        Client::new()
            .get(format!("{admin_url}/session"))
            .header("cookie", cookie.split(';').next().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    // Stop the center. Existing credentials must still prove and verify without any center call.
    server_handle.stop_graceful(None);
    server_task.await.unwrap();
    let origin = Origin::parse("http://127.0.0.1:39001").unwrap();
    let verifier = Arc::new(
        crypto_sso_verifier::Verifier::new(issuer.public().clone(), origin.clone()).unwrap(),
    );
    let app_config=engine.command("save_trusted_app",json!({"id":"","name":"Test application","origin":origin.as_str(),"issuer_id":info.issuer_id})).await.unwrap();
    let (challenge, binding) = verifier
        .create_challenge(&ClaimKeys::new(["role"]).unwrap(), now(), &mut OsRng)
        .unwrap();
    let task=engine.command("import_login_challenge",json!({"app_id":app_config["id"],"challenge_json":String::from_utf8(wire::to_json(&challenge).unwrap()).unwrap()})).await.unwrap();
    let plan = engine
        .command("get_disclosure_plan", json!({"id":task["id"]}))
        .await
        .unwrap();
    assert_eq!(plan["state"], "REVIEW_DISCLOSURE");
    let proof=engine.command("create_presentation",json!({"id":task["id"],"expected_revision":task["revision"],"credential_id":id,"disclose_keys":["role"]})).await.unwrap();
    let raw = proof["presentation_json"].as_str().unwrap().as_bytes();
    let relying = RelyingParty {
        verifier: verifier.clone(),
        store: store.clone(),
        required: ClaimKeys::new(["role"]).unwrap(),
        policy: BTreeMap::from([("role".into(), "member".into())]),
        session_ttl: 3600,
        body_limit: 1048576,
    };
    toasty::create!(StoredChallenge {
        id: challenge.nonce().as_str().to_owned(),
        raw: String::from_utf8(wire::to_json(&challenge).unwrap()).unwrap(),
        consumed: false,
    })
    .exec(&mut store.database())
    .await
    .unwrap();
    let (first, second) = tokio::join!(
        relying.verify_and_consume(raw, binding.expose_cookie_value()),
        relying.verify_and_consume(raw, binding.expose_cookie_value())
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let session = first.or(second).unwrap();
    assert_eq!(session["session"]["claims"], json!({"role":"member"}));
    assert!(session["session"]["claims"].get("name").is_none());
    // Exercise the actual relying-party Salvo handlers with an independent browser session.
    let rp_acceptor = TcpListener::new("127.0.0.1:0").bind().await;
    let rp_base = format!("http://{}", rp_acceptor.inner().local_addr().unwrap());
    let rp_server = Server::new(rp_acceptor);
    let rp_handle = rp_server.handle();
    let rp_secret = token() + &token();
    let rp_task = tokio::spawn(rp_server.serve(crypto_sso_relying_party::service(
        relying.clone(),
        rp_secret.as_bytes(),
        [9; 32],
        false,
    )));
    let rp_browser = Client::builder().cookie_store(true).build().unwrap();
    assert!(
        rp_browser
            .get(&rp_base)
            .send()
            .await
            .unwrap()
            .status()
            .is_client_error()
    );
    let context: Value = rp_browser
        .get(format!("{rp_base}/api/v1/login/context"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let csrf = context["csrf_token"].as_str().unwrap();
    let fresh: Value = rp_browser
        .post(format!("{rp_base}/api/v1/login/challenges"))
        .header("origin", origin.as_str())
        .header("x-csrf-token", csrf)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fresh_task=engine.command("import_login_challenge",json!({"app_id":app_config["id"],"task_id":task["id"],"challenge_json":fresh["challenge_json"]})).await.unwrap();
    let proof=engine.command("create_presentation",json!({"id":task["id"],"expected_revision":fresh_task["revision"],"credential_id":id,"disclose_keys":["role"]})).await.unwrap();
    let public_proof = proof["presentation_json"].as_str().unwrap();
    let login = rp_browser
        .post(format!("{rp_base}/api/v1/login/proofs"))
        .header("origin", origin.as_str())
        .header("x-csrf-token", csrf)
        .header("content-type", "application/json")
        .body(public_proof.to_owned())
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    assert_eq!(
        login.json::<Value>().await.unwrap()["claims"],
        json!({"role":"member"})
    );
    assert_eq!(
        rp_browser
            .get(format!("{rp_base}/api/v1/login/session"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        rp_browser
            .post(format!("{rp_base}/api/v1/login/proofs"))
            .header("origin", origin.as_str())
            .header("x-csrf-token", csrf)
            .body(public_proof.to_owned())
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    rp_handle.stop_graceful(None);
    rp_task.await.unwrap();
    engine.command("lock_wallet", json!({})).await.unwrap();
    drop(engine);
    let engine = Engine::new(wallet_directory, 5, 1048576).unwrap();
    engine.command("unlock_wallet", json!({})).await.unwrap();
    assert_eq!(
        engine
            .command("list_credentials", json!({}))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        engine
            .command("get_disclosure_plan", json!({"id":task["id"]}))
            .await
            .unwrap()["state"],
        "NEEDS_CHALLENGE"
    );
    engine
        .command("delete_credential", json!({"id":id}))
        .await
        .unwrap();
    assert_eq!(
        engine
            .command("get_enrollment", json!({"id":id}))
            .await
            .unwrap()["local_state"],
        "CREDENTIAL_DELETED"
    );
    assert_eq!(
        engine
            .command("get_disclosure_plan", json!({"id":task["id"]}))
            .await
            .unwrap()["state"],
        "CREDENTIAL_DELETED"
    );
    toasty::sql::statement(format!("DROP SCHEMA {schema} CASCADE"))
        .exec(&mut admin_db)
        .await
        .unwrap();
}
