//! Real PostgreSQL CRUD, transaction visibility, rollback and write-skew checks.
use issuer_service::{
    error::{Error, Result},
    models::{Settings, StoredChallenge},
    security::token,
    store::{IsolationLevel, Store, connect_database},
};
use tokio::sync::Barrier;

async fn interrupted_use_case(store: &Store) -> Result<()> {
    let mut db = store.database();
    let mut tx = db.transaction().await?;
    toasty::create!(StoredChallenge {
        id: "rollback/first",
        raw: "first",
        consumed: false
    })
    .exec(&mut tx)
    .await?;
    toasty::create!(StoredChallenge {
        id: "rollback/second",
        raw: "second",
        consumed: false
    })
    .exec(&mut tx)
    .await?;
    StoredChallenge::filter_by_id("preserved")
        .delete()
        .exec(&mut tx)
        .await?;
    Err(Error::invalid())?;
    tx.commit().await?;
    Ok(())
}

async fn reserve_slot(store: &Store, own_slot: &str, ready: &Barrier) -> Result<()> {
    let mut db = store.database();
    let mut tx = db
        .transaction_builder()
        .isolation(IsolationLevel::Serializable)
        .begin()
        .await?;
    let slots = StoredChallenge::all()
        .filter(StoredChallenge::fields().id().starts_with("slot/"))
        .exec(&mut tx)
        .await?;
    assert!(slots.iter().all(|slot| !slot.consumed));
    assert_eq!(slots.len(), 2);
    ready.wait().await;
    StoredChallenge::filter_by_id(own_slot.to_owned())
        .update()
        .consumed(true)
        .exec(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires a dedicated PostgreSQL TEST_DATABASE_URL"]
async fn native_transactions_preserve_atomicity_and_reject_write_skew() {
    let _ = dotenvy::dotenv();
    let database = url::Url::parse(
        &std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required"),
    )
    .unwrap();
    assert!(database.path().ends_with("_test"));
    assert!(!database.query_pairs().any(|(key, _)| key == "dbname"));
    let mut admin = connect_database(database.as_str()).await.unwrap();
    let schema = format!("transaction_test_{}", &token()[..16]);
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

    let mut tx = db.transaction().await.unwrap();
    let created = toasty::create!(StoredChallenge {
        id: "preserved",
        raw: "original",
        consumed: false
    })
    .exec(&mut tx)
    .await
    .unwrap();
    let loaded = StoredChallenge::get_by_id(&mut tx, &created.id)
        .await
        .unwrap();
    assert_eq!(loaded.raw, "original");
    assert!(
        StoredChallenge::filter_by_id("preserved")
            .first()
            .exec(&mut store.database())
            .await
            .unwrap()
            .is_none()
    );
    tx.commit().await.unwrap();
    assert_eq!(
        StoredChallenge::get_by_id(&mut db, &created.id)
            .await
            .unwrap()
            .raw,
        "original"
    );

    assert_eq!(interrupted_use_case(&store).await.unwrap_err().status, 400);
    assert!(
        StoredChallenge::all()
            .filter(StoredChallenge::fields().id().starts_with("rollback/"))
            .exec(&mut db)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        StoredChallenge::filter_by_id("preserved")
            .first()
            .exec(&mut db)
            .await
            .unwrap()
            .is_some()
    );

    let mut tx = db.transaction().await.unwrap();
    toasty::create!(StoredChallenge {
        id: "explicit-rollback",
        raw: "discarded",
        consumed: false
    })
    .exec(&mut tx)
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    assert!(
        StoredChallenge::filter_by_id("explicit-rollback")
            .first()
            .exec(&mut db)
            .await
            .unwrap()
            .is_none()
    );

    let mut tx = db.transaction().await.unwrap();
    toasty::create!(StoredChallenge {
        id: "outer",
        raw: "retained",
        consumed: false
    })
    .exec(&mut tx)
    .await
    .unwrap();
    let mut nested = tx.transaction().await.unwrap();
    toasty::create!(StoredChallenge {
        id: "nested",
        raw: "discarded",
        consumed: false
    })
    .exec(&mut nested)
    .await
    .unwrap();
    nested.rollback().await.unwrap();
    tx.commit().await.unwrap();
    assert!(
        StoredChallenge::filter_by_id("outer")
            .first()
            .exec(&mut db)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        StoredChallenge::filter_by_id("nested")
            .first()
            .exec(&mut db)
            .await
            .unwrap()
            .is_none()
    );

    // Use the same typed Settings update as result submission: a stale enabled
    // snapshot cannot overwrite a disabling change that already committed.
    toasty::create!(Settings {
        id: 1,
        revision: 1,
        claim_hints_revision: 1,
        display_name: "Test",
        description: "",
        contact: "",
        public_base_url: "http://localhost",
        registration_enabled: true,
        issuance_enabled: true,
        review_ttl_seconds: 60,
        poll_interval_seconds: 5,
        updated_at: 0,
        claim_hints: Vec::<crypto_sso_identity_api::ClaimHint>::new(),
    })
    .exec(&mut db)
    .await
    .unwrap();
    let mut tx = db
        .transaction_builder()
        .isolation(IsolationLevel::Serializable)
        .begin()
        .await
        .unwrap();
    let mut settings = Settings::get_by_id(&mut tx, &1).await.unwrap();
    Settings::filter_by_id(1)
        .update()
        .issuance_enabled(false)
        .exec(&mut store.database())
        .await
        .unwrap();
    toasty::create!(StoredChallenge {
        id: "signing/response",
        raw: "discarded",
        consumed: false
    })
    .exec(&mut tx)
    .await
    .unwrap();
    let enabled = settings.issuance_enabled;
    let error = settings
        .update()
        .issuance_enabled(enabled)
        .exec(&mut tx)
        .await
        .unwrap_err();
    assert!(error.is_serialization_failure());
    assert_eq!(Error::from(error).status, 409);
    drop(tx);
    assert!(
        StoredChallenge::filter_by_id("signing/response")
            .first()
            .exec(&mut db)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !Settings::get_by_id(&mut db, &1)
            .await
            .unwrap()
            .issuance_enabled
    );

    toasty::create!(StoredChallenge {
        id: "slot/a",
        raw: "a",
        consumed: false
    })
    .exec(&mut db)
    .await
    .unwrap();
    toasty::create!(StoredChallenge {
        id: "slot/b",
        raw: "b",
        consumed: false
    })
    .exec(&mut db)
    .await
    .unwrap();
    let ready = Barrier::new(2);
    let (left, right) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        tokio::join!(
            reserve_slot(&store, "slot/a", &ready),
            reserve_slot(&store, "slot/b", &ready)
        )
    })
    .await
    .expect("concurrent transactions must complete");
    assert_ne!(left.is_ok(), right.is_ok());
    let conflict = left.err().or_else(|| right.err()).unwrap();
    assert_eq!(conflict.status, 409);
    let slots = StoredChallenge::all()
        .filter(StoredChallenge::fields().id().starts_with("slot/"))
        .exec(&mut db)
        .await
        .unwrap();
    assert_eq!(slots.into_iter().filter(|slot| slot.consumed).count(), 1);

    drop(db);
    drop(store);
    toasty::sql::statement(format!("DROP SCHEMA {schema} CASCADE"))
        .exec(&mut admin)
        .await
        .unwrap();
}
