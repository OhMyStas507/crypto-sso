//! Persistence adapter for Salvo's SessionHandler. Cookie signing and session lifecycle
//! belong to Salvo; atomic persistence uses Toasty's native transaction executor.
use crate::{
    error::Error,
    models::{RevokedSession, WebSession},
    store::{IsolationLevel, Store},
};
use salvo::{
    async_trait,
    session::{Session, SessionStore},
};

#[derive(Clone)]
pub struct PgSessionStore {
    pub store: Store,
    pub namespace: String,
}

impl std::fmt::Debug for PgSessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PgSessionStore")
    }
}

#[async_trait]
impl SessionStore for PgSessionStore {
    #[tracing::instrument(skip_all)]
    async fn load_session(&self, cookie_value: String) -> saysion::Result<Option<Session>> {
        let id = Session::id_from_cookie_value(&cookie_value)?;
        Ok(
            WebSession::filter_by_id(format!("{}/{}", self.namespace, id))
                .first()
                .exec(&mut self.store.database())
                .await?
                .and_then(|row| Session::validate(row.data.0)),
        )
    }

    #[tracing::instrument(skip_all)]
    async fn store_session(&self, session: Session) -> saysion::Result<Option<String>> {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let id = format!("{}/{}", self.namespace, session.id());
        // Serializable isolation prevents a late request from resurrecting a
        // session revoked concurrently by logout or login rotation.
        if RevokedSession::filter_by_id(id.clone())
            .first()
            .exec(&mut tx)
            .await?
            .is_some()
        {
            tracing::debug!("refused to restore a revoked session");
            return Err(Error::conflict().into());
        }
        // Framework saves are naturally upserts; use Toasty's atomic builder.
        WebSession::upsert_by_id(id)
            .namespace(self.namespace.clone())
            .data(&session)
            .exec(&mut tx)
            .await?;
        tx.commit().await?;
        session.reset_data_changed();
        Ok(session.into_cookie_value())
    }

    #[tracing::instrument(skip_all)]
    async fn destroy_session(&self, session: Session) -> saysion::Result {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        let id = format!("{}/{}", self.namespace, session.id());
        WebSession::filter_by_id(id.clone())
            .delete()
            .exec(&mut tx)
            .await?;
        RevokedSession::upsert_by_id(id)
            .or_ignore()
            .exec(&mut tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn clear_store(&self) -> saysion::Result {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::Serializable)
            .begin()
            .await?;
        for row in WebSession::filter_by_namespace(self.namespace.clone())
            .exec(&mut tx)
            .await?
        {
            RevokedSession::upsert_by_id(row.id.clone())
                .or_ignore()
                .exec(&mut tx)
                .await?;
            row.delete().exec(&mut tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
