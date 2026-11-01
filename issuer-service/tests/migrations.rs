//! 验证空库自动建表、重复启动保留数据，以及生成快照与当前模型一致。
use issuer_service::{
    models::*,
    security::token,
    store::{MIGRATIONS, Store, connect_database},
};
use toasty::migration::{History, Snapshot};

#[tokio::test]
#[ignore = "需要独立 PostgreSQL TEST_DATABASE_URL"]
async fn startup_creates_fresh_schema_and_preserves_data_on_restart() {
    let _ = dotenvy::dotenv();
    let database =
        url::Url::parse(&std::env::var("TEST_DATABASE_URL").expect("需要 TEST_DATABASE_URL"))
            .unwrap();
    assert!(database.path().ends_with("_test"));
    assert!(!database.query_pairs().any(|(key, _)| key == "dbname"));
    let mut admin = connect_database(database.as_str()).await.unwrap();
    let schema = format!("migration_test_{}", &token()[..16]);
    toasty::sql::statement(format!("CREATE SCHEMA {schema}"))
        .exec(&mut admin)
        .await
        .unwrap();
    let mut isolated = database.clone();
    isolated
        .query_pairs_mut()
        .append_pair("options", &format!("-c search_path={schema}"));
    let store = Store::connect(isolated.as_str()).await.unwrap();
    let mut db = store.database();
    toasty::create!(IssuerMetadata {
        id: 1,
        issuer_id: "fixed-issuer"
    })
    .exec(&mut db)
    .await
    .unwrap();
    toasty::create!(StoredChallenge {
        id: "durable-challenge",
        raw: "original challenge",
        consumed: true
    })
    .exec(&mut db)
    .await
    .unwrap();
    let restarted = Store::connect(isolated.as_str()).await.unwrap();
    assert_eq!(
        IssuerMetadata::get_by_id(&mut restarted.database(), &1)
            .await
            .unwrap()
            .issuer_id,
        "fixed-issuer"
    );
    assert!(
        StoredChallenge::get_by_id(&mut restarted.database(), &"durable-challenge".to_owned())
            .await
            .unwrap()
            .consumed
    );
    let report = MIGRATIONS.apply(&db).await.unwrap();
    assert_eq!(report.applied(), 0);
    assert_eq!(MIGRATIONS.migrations().len(), 1);
    assert_eq!(report.skipped(), 1);
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("toasty");
    let history = History::load_or_default(directory.join("history.toml")).unwrap();
    assert_eq!(history.entries().len(), 1);
    let latest = history.entries().last().unwrap();
    let snapshot = Snapshot::load(directory.join("snapshots").join(&latest.snapshot_name)).unwrap();
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
    drop(db);
    drop(store);
    toasty::sql::statement(format!("DROP SCHEMA {schema} CASCADE"))
        .exec(&mut admin)
        .await
        .unwrap();
}
