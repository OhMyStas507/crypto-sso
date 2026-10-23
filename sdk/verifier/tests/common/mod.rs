//! 合成、确定性的离线夹具。测试不读取 references，也不依赖系统时间。
#![allow(dead_code)]
use crypto_sso_holder::{Holder, PendingEnrollment};
use crypto_sso_issuer::{BlindCredential, EnrollmentRequest, Issuer};
use crypto_sso_verifier::*;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

pub const NOW: u64 = 1_800_000_000;
pub fn rng(seed: u8) -> ChaCha20Rng {
    ChaCha20Rng::from_seed([seed; 32])
}
pub fn claims() -> Claims {
    Claims::new([
        ("department", "engineering"),
        ("display_name", "Alice"),
        ("role", "member"),
    ])
    .unwrap()
}
pub fn origin() -> Origin {
    Origin::parse("https://forum.example.org").unwrap()
}
pub struct Fixture {
    pub rng: ChaCha20Rng,
    pub issuer: Issuer,
    pub pending: PendingEnrollment,
    pub request: EnrollmentRequest,
    pub blind: BlindCredential,
    pub holder: Holder,
    pub verifier: Verifier,
    pub challenge: LoginChallenge,
    pub binding: ClientBindingSecret,
    pub presentation: Presentation,
}
impl Fixture {
    pub fn new(claims: Claims, selected: ClaimKeys) -> Self {
        let mut rng = rng(11);
        let issuer = Issuer::generate("example-org", &mut rng).unwrap();
        let (pending, request) =
            Holder::begin_enrollment(issuer.public(), &claims, &mut rng).unwrap();
        let blind = issuer.issue_blind(&request, &claims, &mut rng).unwrap();
        let holder = pending.finish(&blind).unwrap();
        let verifier = Verifier::new(issuer.public().clone(), origin()).unwrap();
        let (challenge, binding) = verifier
            .create_challenge(&ClaimKeys::empty(), NOW, &mut rng)
            .unwrap();
        let presentation = holder
            .present(&challenge, &origin(), &selected, NOW, &mut rng)
            .unwrap();
        // 每个攻击夹具先完成一次验真，防止负例因基础样本本就非法而虚假通过。
        let checked = verifier
            .verify_proof(&presentation, &challenge, &binding, NOW)
            .unwrap();
        assert_eq!(checked.disclosed_claims().keys(), selected);
        Self {
            rng,
            issuer,
            pending,
            request,
            blind,
            holder,
            verifier,
            challenge,
            binding,
            presentation,
        }
    }
    pub fn standard() -> Self {
        Self::new(claims(), ClaimKeys::new(["department", "role"]).unwrap())
    }
    pub fn verified(&self, p: &Presentation) -> Result<VerifiedProof> {
        self.verifier
            .verify_proof(p, &self.challenge, &self.binding, NOW)
    }
    pub fn subject(&self) -> Subject {
        self.verified(&self.presentation).unwrap().subject().clone()
    }
}
pub fn change_json(bytes: &[u8], edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let mut value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    edit(&mut value);
    serde_json::to_vec(&value).unwrap()
}
pub fn assert_roundtrip<T: wire::WireMessage + PartialEq>(value: &T) {
    let bytes = wire::to_json(value).unwrap();
    let decoded: T = wire::from_json(&bytes).unwrap();
    assert!(&decoded == value);
    assert_eq!(wire::to_json(&decoded).unwrap(), bytes);
}
