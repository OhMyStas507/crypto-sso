use super::{Application, contracts::*};
use crate::{
    error::Result,
    models::{AuditRecord, Enrollment, Member, Settings},
    store::IsolationLevel,
};
use crypto_sso_identity_api::{EnrollmentStatus, Page, ServiceSettings};

impl Application {
    pub async fn overview_get(&self) -> Result<OverviewView> {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::RepeatableRead)
            .read_only(true)
            .begin()
            .await?;
        let enrollments = list_enrollments(&mut tx).await?;
        let result = OverviewView {
            settings: Settings::get_by_id(&mut tx, &1).await?.into(),
            issuer_id: self.issuer.issuer_id().as_str().into(),
            key_available: true,
            pending: enrollments
                .iter()
                .filter(|e| e.view.status == EnrollmentStatus::PendingReview)
                .count(),
            failed: enrollments
                .iter()
                .filter(|e| e.view.status == EnrollmentStatus::IssueFailed)
                .count(),
            issued: enrollments
                .iter()
                .filter(|e| e.view.status == EnrollmentStatus::Issued)
                .count(),
            recent: enrollments
                .iter()
                .take(6)
                .map(AdminEnrollmentView::from)
                .collect(),
        };
        tx.commit().await?;
        Ok(result)
    }

    pub async fn settings_get(&self) -> Result<ServiceSettings> {
        Ok(Settings::get_by_id(&mut self.store.database(), &1)
            .await?
            .into())
    }

    pub async fn enrollments_list(&self, query: ListQuery) -> Result<Page<AdminEnrollmentView>> {
        self.enrollment_page(query, false).await
    }

    pub async fn issuances_list(&self, query: ListQuery) -> Result<Page<AdminEnrollmentView>> {
        self.enrollment_page(query, true).await
    }

    async fn enrollment_page(
        &self,
        query: ListQuery,
        reviewed_only: bool,
    ) -> Result<Page<AdminEnrollmentView>> {
        let enrollments = list_enrollments(&mut self.store.database()).await?;
        let items = enrollments
            .iter()
            .filter(|e| {
                // 状态按现有协议名称比较；未知状态仍返回空列表。
                let state =
                    serde_json::to_value(&e.view.status).expect("enrollment status serialization");
                (!reviewed_only || e.decision.is_some())
                    && query
                        .status
                        .as_deref()
                        .is_none_or(|s| state.as_str() == Some(s))
                    && query.search.as_deref().is_none_or(|q| {
                        e.view.request_id.contains(q) || e.claims.values().any(|v| v.contains(q))
                    })
            })
            .map(AdminEnrollmentView::from)
            .collect();
        Ok(page(items, query.cursor))
    }

    pub async fn enrollment_get(&self, id: &str) -> Result<AdminEnrollmentView> {
        let enrollment =
            Enrollment::get_by_request_id(&mut self.store.database(), &id.to_owned()).await?;
        Ok(AdminEnrollmentView::from(&enrollment))
    }

    pub async fn members_list(&self, query: ListQuery) -> Result<Page<MemberView>> {
        let members = Member::all().exec(&mut self.store.database()).await?;
        let items = members
            .into_iter()
            .filter(|m| {
                query
                    .search
                    .as_deref()
                    .is_none_or(|q| m.member_ref.contains(q))
            })
            .map(MemberView::from)
            .collect();
        Ok(page(items, query.cursor))
    }

    pub async fn member_get(&self, id: &str) -> Result<MemberDetailView> {
        let mut db = self.store.database();
        let mut tx = db
            .transaction_builder()
            .isolation(IsolationLevel::RepeatableRead)
            .read_only(true)
            .begin()
            .await?;
        let member = Member::get_by_member_ref(&mut tx, &id.to_owned()).await?;
        let enrollment = member.enrollment().exec(&mut tx).await?;
        let result = MemberDetailView {
            member: member.into(),
            enrollment: (&enrollment).into(),
        };
        tx.commit().await?;
        Ok(result)
    }

    pub async fn audit_events_list(&self, query: ListQuery) -> Result<Page<AuditEventView>> {
        let mut events = AuditRecord::all().exec(&mut self.store.database()).await?;
        events.sort_by_key(|e| std::cmp::Reverse((e.at, e.id.clone())));
        Ok(page(
            events.into_iter().map(AuditEventView::from).collect(),
            query.cursor,
        ))
    }
}

async fn list_enrollments(tx: &mut impl toasty::Executor) -> Result<Vec<Enrollment>> {
    let mut entries = Enrollment::all().exec(tx).await?;
    entries.sort_by_key(|e| std::cmp::Reverse((e.view.submitted_at, e.view.request_id.clone())));
    Ok(entries)
}

fn page<T>(items: Vec<T>, cursor: usize) -> Page<T> {
    let total = items.len();
    Page {
        items: items.into_iter().skip(cursor).take(25).collect(),
        total,
        next_cursor: if cursor.saturating_add(25) < total {
            Some((cursor + 25).to_string())
        } else {
            None
        },
    }
}
