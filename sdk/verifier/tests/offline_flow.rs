mod common;
use common::*;
use crypto_sso_core::__private as math;
use crypto_sso_holder::{Error as HE, Holder, PendingEnrollment};
use crypto_sso_issuer::{Error as IE, Issuer};
use crypto_sso_verifier::*;

#[test]
fn en_01_offline_zero_one_three_claims() {
    for values in [
        Claims::empty(),
        Claims::new([("role", "member")]).unwrap(),
        claims(),
    ] {
        let f = Fixture::new(values.clone(), values.keys());
        assert_eq!(f.holder.claims(), &values);
        assert_eq!(
            f.verified(&f.presentation).unwrap().disclosed_claims(),
            &values
        );
        // 领证后仅保留 holder 与公开 verifier，签发私钥不再参与。
        let Fixture {
            holder,
            verifier,
            challenge,
            binding,
            mut rng,
            issuer,
            ..
        } = f;
        drop(issuer);
        let p = holder
            .present(&challenge, &origin(), &values.keys(), NOW, &mut rng)
            .unwrap();
        assert!(verifier.verify_proof(&p, &challenge, &binding, NOW).is_ok());
    }
}

#[test]
fn id_01_02_03_05_06_08_09_stability_and_isolation() {
    let mut f = Fixture::standard();
    let subject = f.subject();
    for selection in [
        ClaimKeys::empty(),
        ClaimKeys::new(["role"]).unwrap(),
        claims().keys(),
    ] {
        let (c, b) = f
            .verifier
            .create_challenge(&selection, NOW, &mut f.rng)
            .unwrap();
        assert_ne!(c.nonce, f.challenge.nonce);
        let p = f
            .holder
            .present(&c, &origin(), &selection, NOW, &mut f.rng)
            .unwrap();
        assert_eq!(
            f.verifier.verify_proof(&p, &c, &b, NOW).unwrap().subject(),
            &subject
        );
    }
    let another = f
        .issuer
        .issue_blind(&f.request, &claims(), &mut f.rng)
        .unwrap();
    assert_ne!(another.signature, f.blind.signature);
    let again = f.pending.finish(&another).unwrap();
    let restored = Holder::import_secret(
        f.holder.export_secret().unwrap().expose_secret(),
        f.issuer.public(),
    )
    .unwrap();
    for holder in [&again, &restored] {
        let p = holder
            .present(
                &f.challenge,
                &origin(),
                &ClaimKeys::empty(),
                NOW,
                &mut f.rng,
            )
            .unwrap();
        assert_eq!(f.verified(&p).unwrap().subject(), &subject);
        assert_eq!(holder.claims(), &claims());
    }
    let (new_pending, new_request) =
        Holder::begin_enrollment(f.issuer.public(), &claims(), &mut f.rng).unwrap();
    assert_ne!(new_request.request_id, f.request.request_id);
    assert_ne!(new_request.commitment, f.request.commitment);
    let new_holder = new_pending
        .finish(
            &f.issuer
                .issue_blind(&new_request, &claims(), &mut f.rng)
                .unwrap(),
        )
        .unwrap();
    let p = new_holder
        .present(
            &f.challenge,
            &origin(),
            &ClaimKeys::empty(),
            NOW,
            &mut f.rng,
        )
        .unwrap();
    assert_ne!(f.verified(&p).unwrap().subject(), &subject);
    for (url, same) in [
        ("HTTPS://FORUM.EXAMPLE.ORG:443/", true),
        ("https://forum.example.org:444", false),
        ("https://other.example.org", false),
    ] {
        let app = Verifier::new(f.issuer.public().clone(), Origin::parse(url).unwrap()).unwrap();
        let (c, b) = app
            .create_challenge(&ClaimKeys::empty(), NOW, &mut f.rng)
            .unwrap();
        let p = f
            .holder
            .present(&c, app.origin(), &ClaimKeys::empty(), NOW, &mut f.rng)
            .unwrap();
        assert_eq!(
            app.verify_proof(&p, &c, &b, NOW).unwrap().subject() == &subject,
            same
        );
        if !same {
            assert!(matches!(
                f.verifier.verify_proof(&p, &c, &b, NOW),
                Err(Error::OriginMismatch)
            ));
        }
    }
}

#[test]
fn en_02_03_04_05_06_07_08_enrollment_adversaries() {
    let mut f = Fixture::standard();
    for approved in [
        Claims::new([
            ("department", "engineering"),
            ("display_name", "Alice"),
            ("role", "admin"),
        ])
        .unwrap(),
        Claims::new([("role", "member")]).unwrap(),
        Claims::new(claims().iter().chain([("extra", "yes")])).unwrap(),
    ] {
        assert!(matches!(
            f.issuer.issue_blind(&f.request, &approved, &mut f.rng),
            Err(IE::ClaimsMismatch)
        ));
    }
    let other = Issuer::generate("other-org", &mut f.rng).unwrap();
    assert!(matches!(
        other.issue_blind(&f.request, &claims(), &mut f.rng),
        Err(IE::IssuerMismatch)
    ));
    let valid_point = math::encode(&math::parameters(3).unwrap().g1).unwrap();
    for field in [
        "commitment",
        "proof_commitment",
        "response_secret",
        "response_blinding",
        "claims",
        "claim_key",
        "claim_value",
        "request_id",
        "issuer_id",
    ] {
        let mut r = f.request.clone();
        match field {
            "commitment" => r.commitment = valid_point.clone(),
            "proof_commitment" => r.proof_commitment = valid_point.clone(),
            "response_secret" => r.response_secret = math::encode(&math::Fr::from(7u64)).unwrap(),
            "response_blinding" => {
                r.response_blinding = math::encode(&math::Fr::from(8u64)).unwrap()
            }
            "claims" => r.claims = Claims::new([("role", "admin")]).unwrap(),
            "claim_key" => {
                r.claims = Claims::new(
                    claims()
                        .iter()
                        .map(|(k, v)| (if k == "department" { "division" } else { k }, v)),
                )
                .unwrap()
            }
            "claim_value" => {
                r.claims = Claims::new(
                    claims()
                        .iter()
                        .map(|(k, v)| (k, if k == "role" { "admin" } else { v })),
                )
                .unwrap()
            }
            "request_id" => {
                r.request_id = crypto_sso_core::RequestId::parse(&"aa".repeat(32)).unwrap()
            }
            _ => r.issuer_id = other.issuer_id().clone(),
        }
        assert_ne!(r.request_hash().unwrap(), f.request.request_hash().unwrap());
        let result = f.issuer.issue_blind(&r, &r.claims, &mut f.rng);
        if field == "issuer_id" {
            assert!(matches!(result, Err(IE::IssuerMismatch)));
        } else {
            assert!(matches!(result, Err(IE::InvalidEnrollmentProof)), "{field}");
        }
    }
    for field in [
        "issuer_id",
        "request_id",
        "request_hash",
        "claims",
        "signature",
    ] {
        let mut response = f.blind.clone();
        match field {
            "issuer_id" => response.issuer_id = other.issuer_id().clone(),
            "request_id" => {
                response.request_id = crypto_sso_core::RequestId::parse(&"ab".repeat(32)).unwrap()
            }
            "request_hash" => {
                response.request_hash =
                    crypto_sso_core::RequestHash::parse(&"ac".repeat(32)).unwrap()
            }
            "claims" => response.claims = Claims::empty(),
            _ => {
                let mut sig = math::signature(&response.signature).unwrap();
                sig.e += math::Fr::from(1u64);
                response.signature = math::encode(&sig).unwrap();
            }
        }
        let result = f.pending.finish(&response);
        match field {
            "issuer_id" => assert!(matches!(result, Err(HE::IssuerMismatch))),
            "signature" => assert!(matches!(result, Err(HE::InvalidCredential))),
            _ => assert!(matches!(result, Err(HE::EnrollmentMismatch))),
        }
        assert!(f.pending.finish(&f.blind).is_ok());
    }
    // 元数据仍属于原请求，签名由另一私钥生成，确保实际到达最终验签。
    let other_key = math::SecretKey(math::nonzero(&mut f.rng));
    let params = math::parameters(6).unwrap();
    let values = math::messages(math::Fr::from(0u64), f.issuer.issuer_id(), &claims()).unwrap();
    let sig = math::Signature::new_with_committed_messages(
        &mut f.rng,
        &math::point(&f.request.commitment).unwrap(),
        values.iter().enumerate().skip(1).collect(),
        &other_key,
        &params,
    )
    .unwrap();
    let mut response = f.blind.clone();
    response.signature = math::encode(&sig).unwrap();
    assert!(matches!(
        f.pending.finish(&response),
        Err(HE::InvalidCredential)
    ));
    let restored = PendingEnrollment::import_secret(
        f.pending.export_secret().unwrap().expose_secret(),
        f.issuer.public(),
    )
    .unwrap();
    assert!(matches!(
        restored.finish(&response),
        Err(HE::InvalidCredential)
    ));
    assert!(restored.finish(&f.blind).is_ok());
}

#[test]
fn ds_01_02_03_04_05_06_07_selection_contract() {
    let mut f = Fixture::standard();
    // 穷举三个属性的八个子集，包含首尾、不连续槽位及多个隐藏消息。
    let all = ["department", "display_name", "role"];
    for mask in 0..8 {
        let selection = ClaimKeys::new(
            all.iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, k)| *k),
        )
        .unwrap();
        let p = f
            .holder
            .present(&f.challenge, &origin(), &selection, NOW, &mut f.rng)
            .unwrap();
        assert_eq!(p.claim_count, 3);
        assert_eq!(f.verified(&p).unwrap().disclosed_claims().keys(), selection);
        assert_eq!(f.holder.claims(), &claims());
    }
    assert!(matches!(
        f.holder.present(
            &f.challenge,
            &origin(),
            &ClaimKeys::new(["unknown"]).unwrap(),
            NOW,
            &mut f.rng
        ),
        Err(HE::InvalidDisclosure)
    ));
    for required in [
        ClaimKeys::new(["role"]).unwrap(),
        ClaimKeys::new(["role", "department"]).unwrap(),
        ClaimKeys::new(["absent"]).unwrap(),
    ] {
        let (c, b) = f
            .verifier
            .create_challenge(&required, NOW, &mut f.rng)
            .unwrap();
        assert!(matches!(
            f.holder
                .present(&c, &origin(), &ClaimKeys::empty(), NOW, &mut f.rng),
            Err(HE::MissingRequiredClaims)
        ));
        let mut p = f.presentation.clone();
        p.challenge = c.clone();
        p.disclosed_claims.clear();
        assert!(matches!(
            f.verifier.verify_proof(&p, &c, &b, NOW),
            Err(Error::MissingRequiredClaims)
        ));
    }
    let values = Claims::new([
        ("department", ""),
        ("display_name", "private"),
        ("role", "guest"),
    ])
    .unwrap();
    let mut guest = Fixture::new(values, ClaimKeys::new(["department", "role"]).unwrap());
    let (c, b) = guest
        .verifier
        .create_challenge(&ClaimKeys::new(["role"]).unwrap(), NOW, &mut guest.rng)
        .unwrap();
    let p = guest
        .holder
        .present(
            &c,
            &origin(),
            &ClaimKeys::new(["department", "role"]).unwrap(),
            NOW,
            &mut guest.rng,
        )
        .unwrap();
    let checked = guest.verifier.verify_proof(&p, &c, &b, NOW).unwrap();
    assert_eq!(checked.disclosed_claims().get("role"), Some("guest"));
    assert_eq!(checked.disclosed_claims().get("department"), Some(""));
    assert_eq!(checked.disclosed_claims().get("display_name"), None);
}

#[test]
fn pv_01_02_and_sv_01_randomization_privacy_and_statelessness() {
    let hidden = Claims::new([
        ("department", "PUBLIC"),
        ("private_marker", "HIDDEN_VALUE_MARKER"),
        ("role", "member"),
    ])
    .unwrap();
    let mut f = Fixture::new(hidden, ClaimKeys::new(["role"]).unwrap());
    let second = f
        .holder
        .present(
            &f.challenge,
            &origin(),
            &ClaimKeys::new(["role"]).unwrap(),
            NOW,
            &mut f.rng,
        )
        .unwrap();
    assert_ne!(second.credential_proof, f.presentation.credential_proof);
    assert_ne!(
        second.pseudonym_commitment,
        f.presentation.pseudonym_commitment
    );
    assert_eq!(f.verified(&second).unwrap().subject(), &f.subject());
    for _ in 0..2 {
        assert_eq!(f.verified(&f.presentation).unwrap().subject(), &f.subject());
    }
    let output = String::from_utf8(wire::to_json(&second).unwrap()).unwrap();
    for secret in [
        "private_marker",
        "HIDDEN_VALUE_MARKER",
        f.request.request_id.as_str(),
        f.blind.request_hash.as_str(),
        &f.blind.signature,
    ] {
        assert!(!output.contains(secret));
    }
    let backup: serde_json::Value =
        serde_json::from_slice(f.holder.export_secret().unwrap().expose_secret()).unwrap();
    assert!(!output.contains(backup["credential"].as_str().unwrap()));
    let keys: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(keys.as_object().unwrap().len(), 7);
    assert_eq!(keys["disclosed_claims"].as_array().unwrap().len(), 1);
    assert!(
        !output.contains(
            &math::encode(&math::claim_message("private_marker", "HIDDEN_VALUE_MARKER").unwrap())
                .unwrap()
        )
    );
}
