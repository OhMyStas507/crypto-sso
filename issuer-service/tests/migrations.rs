//! Native startup migrations preserve old JSON records and load typed models.
use issuer_service::{
    models::*,
    security::token,
    store::{MIGRATIONS, Store, connect_database},
};
use serde_json::{Value, json};
use toasty::migration::{History, MigrationSet, Snapshot};

async fn scalar(db: &mut toasty::Db, sql: &str) -> toasty::stmt::Value {
    toasty::sql::query(sql).exec(db).await.unwrap()[0].as_record_unwrap()[0].clone()
}

async fn legacy_fixtures(db: &mut toasty::Db) -> Value {
    let hints = json!([{"key":"role","label":"Role","description":"Original hint",
        "default_value":"member","example_value":null,"include_by_default":true}]);
    let enrollment = json!({
        "view":{"request_id":"durable-request","request_hash":"fixed-hash","status":"ISSUED","revision":7,
            "submitted_at":10,"review_deadline":100,"public_reason":null,"poll_interval_seconds":5},
        "raw_request":"original SDK request", "claims":{"name":" Alice ","role":"member"},
        "metadata_hash":"metadata", "access_hash":"access", "claim_hints_revision":1,
        "hints_snapshot":hints, "application_note":"original application", "decision":null,
        "reviewer":"reviewer", "reviewed_at":20,"blind_credential":"original SDK response",
        "issued_at":30,"generation":3,"lease_until":0,"failure_code":null,"history":[]
    });
    let mut session = salvo::session::Session::new();
    session.insert("fixture", "preserved").unwrap();
    let rows = vec![
        ("issuer/id".to_owned(), json!("fixed-issuer")),
        (
            "settings/current".to_owned(),
            json!({"revision":3,"claim_hints_revision":1,
            "display_name":"Original issuer","description":"Description","contact":"Contact",
            "public_base_url":"http://localhost","registration_enabled":true,"issuance_enabled":false,
            "review_ttl_seconds":60,"poll_interval_seconds":5,"updated_at":42,"claim_hints":hints}),
        ),
        ("hints/1".to_owned(), hints.clone()),
        ("enrollment/durable-request".to_owned(), enrollment.clone()),
        (
            "member/member-1".to_owned(),
            json!({"member_ref":"member-1","request_id":"durable-request",
            "evidence_source":"roster","verification_note":"confirmed","verified_at":20,"issued":true}),
        ),
        (
            "audit/event-1".to_owned(),
            json!({"id":"event-1","at":20,"actor":"admin","action":"approve","target":"durable-request"}),
        ),
        (
            "idempotency/key-1".to_owned(),
            json!({"digest":"original-digest","result":{"status":"APPROVED"}}),
        ),
        (
            "session/browser".to_owned(),
            serde_json::to_value(&session).unwrap(),
        ),
        (
            "rp/session/browser".to_owned(),
            serde_json::to_value(&session).unwrap(),
        ),
        ("revoked/session/revoked-browser".to_owned(), json!(true)),
        (
            "rp/challenge/nonce-1".to_owned(),
            json!({"raw":"original SDK challenge","consumed":true}),
        ),
        (
            "rp/auth/ticket-hash".to_owned(),
            json!({"subject":"subject-1","issuer_id":"fixed-issuer",
            "origin":"http://localhost","claims":{"role":"member"},"expires_at":99}),
        ),
        (
            "extension/preserved".to_owned(),
            json!({"custom":"preserve unknown extension keys"}),
        ),
    ];
    for (id, payload) in rows {
        // SQL here reproduces the retired schema; runtime CRUD uses native models.
        toasty::sql::statement("INSERT INTO identity_records (id, payload) VALUES ($1, $2)")
            .bind(id)
            .bind(payload.to_string())
            .exec(db)
            .await
            .unwrap();
    }
    enrollment
}

#[tokio::test]
#[ignore = "requires a dedicated PostgreSQL TEST_DATABASE_URL"]
async fn startup_migrates_fresh_and_legacy_schemas_without_losing_data() {
    let _ = dotenvy::dotenv();
    let database = url::Url::parse(
        &std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required"),
    )
    .unwrap();
    assert!(database.path().ends_with("_test"));
    assert!(!database.query_pairs().any(|(key, _)| key == "dbname"));
    let mut admin = connect_database(database.as_str()).await.unwrap();

    // Cover empty databases, the pre-migration prototype and the prior release
    // whose migration history already includes 0000 and 0001.
    for version in 0..3 {
        let schema = format!("migration_test_{}", &token()[..16]);
        toasty::sql::statement(format!("CREATE SCHEMA {schema}"))
            .exec(&mut admin)
            .await
            .unwrap();
        let mut isolated = database.clone();
        isolated
            .query_pairs_mut()
            .append_pair("options", &format!("-c search_path={schema}"));
        let mut db = connect_database(isolated.as_str()).await.unwrap();
        let mut expected_enrollment = None;
        if version == 1 {
            toasty::sql::statement(
                "CREATE TABLE identity_records (id TEXT PRIMARY KEY, payload TEXT NOT NULL)",
            )
            .exec(&mut db)
            .await
            .unwrap();
            toasty::sql::statement("CREATE TABLE identity_transaction_guard (id INTEGER PRIMARY KEY, revision BIGINT NOT NULL)").exec(&mut db).await.unwrap();
            toasty::sql::statement("INSERT INTO identity_transaction_guard VALUES (1, 7)")
                .exec(&mut db)
                .await
                .unwrap();
        } else if version == 2 {
            MigrationSet::new(&MIGRATIONS.migrations()[..2])
                .apply(&db)
                .await
                .unwrap();
        }
        if version > 0 {
            expected_enrollment = Some(legacy_fixtures(&mut db).await);
        }

        let store = Store::connect(isolated.as_str()).await.unwrap();
        assert_eq!(
            scalar(&mut db, "SELECT COUNT(*) FROM __toasty_migrations").await,
            MIGRATIONS.migrations().len() as i64
        );
        assert_eq!(scalar(&mut db, "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = current_schema() AND table_name IN ('identity_records', 'identity_transaction_guard')").await, 0_i64);
        if let Some(expected) = expected_enrollment {
            assert_eq!(
                IssuerMetadata::get_by_id(&mut db, &1)
                    .await
                    .unwrap()
                    .issuer_id,
                "fixed-issuer"
            );
            let settings = Settings::get_by_id(&mut db, &1).await.unwrap();
            assert_eq!(settings.revision, 3);
            assert!(settings.registration_enabled && !settings.issuance_enabled);
            assert_eq!(
                settings.claim_hints[0].default_value.as_deref(),
                Some("member")
            );
            assert_eq!(
                ClaimHintRevision::get_by_revision(&mut db, &1)
                    .await
                    .unwrap()
                    .hints[0]
                    .key,
                "role"
            );
            let member = Member::get_by_member_ref(&mut db, &"member-1".to_owned())
                .await
                .unwrap();
            assert!(member.issued);
            // Traverse Toasty's generated association instead of manually joining IDs.
            let enrollment = member.enrollment().exec(&mut db).await.unwrap();
            let mut restored = serde_json::to_value(enrollment).unwrap();
            restored.as_object_mut().unwrap().remove("request_id");
            assert_eq!(restored, expected);
            assert_eq!(
                AuditRecord::get_by_id(&mut db, &"event-1".to_owned())
                    .await
                    .unwrap()
                    .action,
                "approve"
            );
            assert_eq!(
                Idempotency::get_by_key(&mut db, &"key-1".to_owned())
                    .await
                    .unwrap()
                    .result,
                json!({"status":"APPROVED"})
            );
            assert_eq!(
                WebSession::get_by_id(&mut db, &"session/browser".to_owned())
                    .await
                    .unwrap()
                    .data
                    .get::<String>("fixture")
                    .as_deref(),
                Some("preserved")
            );
            assert_eq!(
                WebSession::filter_by_namespace("rp/session")
                    .exec(&mut db)
                    .await
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(RevokedSession::all().exec(&mut db).await.unwrap().len(), 1);
            assert!(
                StoredChallenge::get_by_id(&mut db, &"nonce-1".to_owned())
                    .await
                    .unwrap()
                    .consumed
            );
            let session = ApplicationSession::get_by_id(&mut db, &"ticket-hash".to_owned())
                .await
                .unwrap();
            assert_eq!(
                session.claims.get("role").map(String::as_str),
                Some("member")
            );
            assert_eq!(
                scalar(&mut db, "SELECT COUNT(*) FROM identity_records_legacy").await,
                13_i64
            );
        } else {
            toasty::create!(IssuerMetadata {
                id: 1,
                issuer_id: "fresh-issuer"
            })
            .exec(&mut db)
            .await
            .unwrap();
        }
        let restarted = Store::connect(isolated.as_str()).await.unwrap();
        assert!(
            IssuerMetadata::filter_by_id(1)
                .first()
                .exec(&mut restarted.database())
                .await
                .unwrap()
                .is_some()
        );
        let report = MIGRATIONS.apply(&db).await.unwrap();
        assert_eq!(report.applied(), 0);
        assert_eq!(report.skipped(), MIGRATIONS.migrations().len());
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("toasty");
        let history = History::load_or_default(directory.join("history.toml")).unwrap();
        let latest = history.entries().last().unwrap();
        let snapshot =
            Snapshot::load(directory.join("snapshots").join(&latest.snapshot_name)).unwrap();
        assert!(
            toasty::migration::generate(
                db.driver(),
                &snapshot.schema,
                &db.schema().db,
                &Default::default()
            )
            .is_none()
        );
        drop(restarted);
        drop(store);
        drop(db);
        toasty::sql::statement(format!("DROP SCHEMA {schema} CASCADE"))
            .exec(&mut admin)
            .await
            .unwrap();
    }
}
