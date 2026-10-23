//! SV 配套单元测试：用内存事务模型验证调用顺序、原子提交和来源隔离。
//! 模型不属于 SDK 产品接口，也不替代未来真实数据库、HTTP 和钱包 UI 的集成验收。
mod common;
use common::*;
use crypto_sso_holder::{Error as HE, Holder};
use crypto_sso_issuer::{BlindCredential, EnrollmentRequest, Issuer};
use crypto_sso_verifier::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Barrier, Mutex},
};

#[derive(Default, Clone)]
struct AppState {
    pending: Option<LoginChallenge>,
    sessions: Vec<Subject>,
}
struct AppStore(Mutex<AppState>);
impl AppStore {
    fn new(challenge: LoginChallenge) -> Self {
        Self(Mutex::new(AppState {
            pending: Some(challenge),
            sessions: Vec::new(),
        }))
    }
    fn commit(
        &self,
        verified: &VerifiedProof,
        commit_now: u64,
        fail_after_consume: bool,
        fail_commit: bool,
    ) -> bool {
        let mut state = self.0.lock().unwrap();
        if state.pending.as_ref() != Some(verified.challenge())
            || verified.challenge().expires_at <= commit_now
        {
            return false;
        }
        // 在事务副本内执行消费和会话创建，只有整个提交成功才替换可见状态。
        let mut transaction = state.clone();
        transaction.pending = None;
        if fail_after_consume {
            return false;
        }
        transaction.sessions.push(verified.subject().clone());
        if fail_commit {
            return false;
        }
        *state = transaction;
        true
    }
}
#[test]
fn sv_02_atomic_consume_has_one_winner_and_cookie_only_after_commit() {
    let f = Fixture::standard();
    let store = Arc::new(AppStore::new(f.challenge.clone()));
    let barrier = Arc::new(Barrier::new(4));
    let winners = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let store = store.clone();
                let barrier = barrier.clone();
                let fixture = &f;
                scope.spawn(move || {
                    let verified = fixture.verified(&fixture.presentation).unwrap();
                    barrier.wait();
                    // 返回值代表提交后的 Set-Cookie；没有提前设置的会话 Cookie。
                    store.commit(&verified, NOW, false, false)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|won| *won)
            .count()
    });
    assert_eq!(winners, 1);
    let state = store.0.lock().unwrap();
    assert!(state.pending.is_none());
    assert_eq!(state.sessions.len(), 1);
    drop(state);
    let checked = f.verified(&f.presentation).unwrap();
    assert!(!store.commit(&checked, NOW, false, false));
}
#[test]
fn sv_03_04_expiry_rollback_missing_consumed_and_full_record_match() {
    let f = Fixture::standard();
    let checked = f.verified(&f.presentation).unwrap();
    let store = AppStore::new(f.challenge.clone());
    assert!(!store.commit(&checked, f.challenge.expires_at, false, false));
    for (fail_session, fail_commit) in [(true, false), (false, true)] {
        assert!(!store.commit(&checked, NOW, fail_session, fail_commit));
        let state = store.0.lock().unwrap();
        assert_eq!(state.pending.as_ref(), Some(&f.challenge));
        assert!(state.sessions.is_empty());
    }
    let mut changed = f.challenge.clone();
    changed.required_claims = ClaimKeys::new(["role"]).unwrap();
    let conflicting = AppStore::new(changed);
    assert!(!conflicting.commit(&checked, NOW, false, false));
    let absent = AppStore(Mutex::new(AppState::default()));
    assert!(!absent.commit(&checked, NOW, false, false));
    assert!(store.commit(&checked, NOW, false, false));
    assert!(!store.commit(&checked, NOW, false, false));
    assert_eq!(store.0.lock().unwrap().sessions.len(), 1);
}

#[derive(Clone)]
struct Issuance {
    member: String,
    request: EnrollmentRequest,
    approved: Claims,
    response: BlindCredential,
}
#[derive(Default, Clone)]
struct LedgerState {
    by_member: BTreeMap<String, Issuance>,
    by_request: BTreeMap<String, String>,
    signatures: usize,
}
struct Ledger(Mutex<LedgerState>);
impl Ledger {
    fn issue(
        &self,
        issuer: &Issuer,
        member: &str,
        request: &EnrollmentRequest,
        approved: &Claims,
        fail_commit: bool,
    ) -> std::result::Result<BlindCredential, ()> {
        let mut state = self.0.lock().unwrap();
        let request_id = request.request_id.as_str();
        if let Some(existing) = state.by_member.get(member) {
            if existing.member == member
                && existing.request == *request
                && existing.approved == *approved
            {
                return Ok(existing.response.clone());
            }
            return Err(());
        }
        if state.by_request.contains_key(request_id) {
            return Err(());
        }
        let mut transaction = state.clone();
        let response = issuer
            .issue_blind(request, approved, &mut rng(91))
            .map_err(|_| ())?;
        transaction
            .by_request
            .insert(request_id.into(), member.into());
        transaction.by_member.insert(
            member.into(),
            Issuance {
                member: member.into(),
                request: request.clone(),
                approved: approved.clone(),
                response: response.clone(),
            },
        );
        transaction.signatures += 1;
        if fail_commit {
            return Err(());
        }
        *state = transaction;
        Ok(response)
    }
}
#[test]
fn sv_05_issuance_ledger_idempotence_conflicts_concurrency_and_rollback() {
    let mut f = Fixture::standard();
    let ledger = Ledger(Mutex::new(LedgerState::default()));
    assert!(
        ledger
            .issue(&f.issuer, "alice", &f.request, &claims(), true)
            .is_err()
    );
    assert_eq!(ledger.0.lock().unwrap().signatures, 0);
    let barrier = Barrier::new(4);
    let responses = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    ledger
                        .issue(&f.issuer, "alice", &f.request, &claims(), false)
                        .unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(responses.iter().all(|r| r == &responses[0]));
    assert_eq!(ledger.0.lock().unwrap().signatures, 1);
    assert!(
        ledger
            .issue(&f.issuer, "bob", &f.request, &claims(), false)
            .is_err()
    );
    let (_, other_request) =
        Holder::begin_enrollment(f.issuer.public(), &claims(), &mut f.rng).unwrap();
    assert!(
        ledger
            .issue(&f.issuer, "alice", &other_request, &claims(), false)
            .is_err()
    );
    let mut changed = f.request.clone();
    changed.claims = Claims::new([("role", "admin")]).unwrap();
    assert_ne!(
        changed.request_hash().unwrap(),
        f.request.request_hash().unwrap()
    );
    assert!(
        ledger
            .issue(&f.issuer, "alice", &changed, &changed.claims, false)
            .is_err()
    );
    assert!(
        ledger
            .issue(&f.issuer, "bob", &changed, &changed.claims, false)
            .is_err()
    );
    assert!(
        ledger
            .issue(&f.issuer, "alice", &f.request, &Claims::empty(), false)
            .is_err()
    );
}

#[test]
fn sv_06_business_values_are_evaluated_after_cryptographic_verification() {
    for (values, required) in [
        (
            Claims::new([("expires_at", "1")]).unwrap(),
            ClaimKeys::new(["expires_at"]).unwrap(),
        ),
        (
            Claims::new([("role", "guest")]).unwrap(),
            ClaimKeys::new(["role"]).unwrap(),
        ),
    ] {
        let mut f = Fixture::new(values, required.clone());
        let (c, b) = f
            .verifier
            .create_challenge(&required, NOW, &mut f.rng)
            .unwrap();
        let p = f
            .holder
            .present(&c, &origin(), &required, NOW, &mut f.rng)
            .unwrap();
        let verified = f.verifier.verify_proof(&p, &c, &b, NOW).unwrap();
        let permitted = verified.disclosed_claims().get("role") == Some("member")
            && verified
                .disclosed_claims()
                .get("expires_at")
                .and_then(|s| s.parse::<u64>().ok())
                .is_none_or(|time| time > NOW);
        assert!(!permitted);
    }
}

#[test]
fn sv_07_consent_trusted_origin_cookie_source_and_server_clock() {
    let mut f = Fixture::standard();
    let consent = ClaimKeys::new(["role"]).unwrap();
    let (c, b) = f
        .verifier
        .create_challenge(
            &ClaimKeys::new(["role", "display_name"]).unwrap(),
            NOW,
            &mut f.rng,
        )
        .unwrap();
    assert!(matches!(
        f.holder.present(&c, &origin(), &consent, NOW, &mut f.rng),
        Err(HE::MissingRequiredClaims)
    ));
    let spoofed_origin = Origin::parse("https://attacker.example.org").unwrap();
    let mut forged = c;
    forged.audience = spoofed_origin;
    assert!(matches!(
        f.holder
            .present(&forged, &origin(), &claims().keys(), NOW, &mut f.rng),
        Err(HE::OriginMismatch)
    ));
    let p = f
        .holder
        .present(&f.challenge, &origin(), &consent, NOW, &mut f.rng)
        .unwrap();
    assert!(matches!(
        f.verifier.verify_proof(&p, &f.challenge, &b, NOW),
        Err(Error::ClientBindingMismatch)
    ));
    // 钱包时钟尚有效，服务器时钟已到期；最终由后者决定。
    assert!(matches!(
        f.verifier
            .verify_proof(&p, &f.challenge, &f.binding, NOW + 120),
        Err(Error::ChallengeExpired)
    ));
    let public = String::from_utf8(wire::to_json(&p).unwrap()).unwrap();
    assert!(!public.contains(f.binding.expose_cookie_value()));
}
