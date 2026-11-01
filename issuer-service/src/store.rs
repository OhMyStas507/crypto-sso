//! Toasty connection and startup migration setup. Business code uses the
//! generated model builders directly; this module does not implement CRUD.
use crate::{error::Result, models::*};
pub use toasty_core::driver::operation::IsolationLevel;

/// Shared model registration for the service and the official migration CLI.
pub async fn connect_database(url: &str) -> toasty::Result<toasty::Db> {
    toasty::Db::builder()
        .models(toasty::models!(
            IssuerMetadata,
            Settings,
            ClaimHintRevision,
            Enrollment,
            Member,
            Idempotency,
            AuditRecord,
            StoredChallenge,
            ApplicationSession,
            Initialization,
            Administrator,
            InitializationKey,
            InitializationSubmission
        ))
        .connect(url)
        .await
}

/// Toasty embeds the generated history and SQL, then tracks applied versions in PostgreSQL.
pub static MIGRATIONS: toasty::migration::MigrationSet = toasty::embed_migrations!();

#[derive(Clone)]
pub struct Store {
    db: toasty::Db,
}

impl Store {
    #[tracing::instrument(skip_all)]
    pub async fn connect(url: &str) -> Result<Self> {
        tracing::debug!("connecting database");
        let db = connect_database(url).await?;
        let report = MIGRATIONS.apply(&db).await?;
        tracing::info!(
            applied = report.applied(),
            skipped = report.skipped(),
            "database migrations complete"
        );
        Ok(Self { db })
    }

    /// Clones share Toasty's pool. Related operations must use the same
    /// transaction executor instead of obtaining another Db handle.
    pub fn database(&self) -> toasty::Db {
        self.db.clone()
    }
}
