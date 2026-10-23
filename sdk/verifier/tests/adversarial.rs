mod common;
use common::*;
use crypto_sso_core::__private::*;
use crypto_sso_holder::Holder;
use crypto_sso_verifier::*;
use dock_crypto_utils::signature::MessageOrBlinding;

#[test]
fn at_01_02_03_04_05_07_08_tampering_and_splicing() {
    let mut f = Fixture::standard();
    let first_only = f
        .holder
        .present(
            &f.challenge,
            &origin(),
            &ClaimKeys::new(["department"]).unwrap(),
            NOW,
            &mut f.rng,
        )
        .unwrap();
    assert!(f.verified(&first_only).is_ok());
    // 只改数量，槽位 0 在 2 和 4 两种变体中都合法。
    for n in [2, 4] {
        let mut p = first_only.clone();
        p.claim_count = n;
        assert!(matches!(f.verified(&p), Err(Error::InvalidPresentation)));
    }
    for variant in 0..15 {
        let mut p = f.presentation.clone();
        match variant {
            0 => p.disclosed_claims[0].key = "division".into(),
            1 => p.disclosed_claims[0].value = "sales".into(),
            2 => p.disclosed_claims[0].index = 1,
            3 => p.disclosed_claims[1].index = 0,
            4 => p.disclosed_claims[1].key = "department".into(),
            5 => p.disclosed_claims[1].index = 3,
            6 => p.disclosed_claims.swap(0, 1),
            7 => {
                p.disclosed_claims[0].key = "z".into();
                p.disclosed_claims[1].key = "a".into();
            }
            8 => {
                p.claim_count = 2;
                p.disclosed_claims[1].index = 1;
            }
            9 => p.claim_count = 4,
            10 => {
                p.disclosed_claims.remove(0);
            }
            11 => p.disclosed_claims.insert(
                1,
                DisclosedClaim {
                    index: 1,
                    key: "injected".into(),
                    value: "admin".into(),
                },
            ),
            12 => p.pseudonym = encode(&parameters(0).unwrap().g1).unwrap(),
            13 => {
                let mut proof = proof(
                    &p.credential_proof,
                    p.claim_count,
                    p.disclosed_claims.len(),
                    claims_limits(),
                )
                .unwrap();
                proof.sc_resp_2.as_mut().unwrap().0[0] += Fr::from(1u64);
                p.credential_proof = encode(&proof).unwrap();
            }
            _ => {
                let mut proof = proof(
                    &p.credential_proof,
                    p.claim_count,
                    p.disclosed_claims.len(),
                    claims_limits(),
                )
                .unwrap();
                proof.sc_resp_2.as_mut().unwrap().0.swap(2, 3);
                p.credential_proof = encode(&proof).unwrap();
            }
        }
        assert!(
            matches!(f.verified(&p), Err(Error::InvalidPresentation)),
            "variant {variant}"
        );
    }
    for add in [false, true] {
        let mut p = f.presentation.clone();
        let mut proof = proof(
            &p.credential_proof,
            p.claim_count,
            p.disclosed_claims.len(),
            claims_limits(),
        )
        .unwrap();
        if add {
            proof.sc_resp_2.as_mut().unwrap().0.push(Fr::from(1u64));
        } else {
            proof.sc_resp_2.as_mut().unwrap().0.pop();
        }
        p.credential_proof = encode(&proof).unwrap();
        assert!(matches!(f.verified(&p), Err(Error::InvalidPresentation)));
        assert!(wire::to_json(&p).is_err());
    }
    let (pending, request) =
        Holder::begin_enrollment(f.issuer.public(), &claims(), &mut f.rng).unwrap();
    let holder = pending
        .finish(
            &f.issuer
                .issue_blind(&request, &claims(), &mut f.rng)
                .unwrap(),
        )
        .unwrap();
    let other = holder
        .present(
            &f.challenge,
            &origin(),
            &ClaimKeys::new(["department", "role"]).unwrap(),
            NOW,
            &mut f.rng,
        )
        .unwrap();
    assert!(f.verified(&other).is_ok());
    assert_ne!(other.pseudonym, f.presentation.pseudonym);
    for mode in 0..3 {
        let mut p = f.presentation.clone();
        match mode {
            0 => {
                p.pseudonym = other.pseudonym.clone();
                p.pseudonym_commitment = other.pseudonym_commitment.clone();
            }
            1 => p.credential_proof = other.credential_proof.clone(),
            _ => p.disclosed_claims[0].value = "different-credential-value".into(),
        }
        assert!(matches!(f.verified(&p), Err(Error::InvalidPresentation)));
    }
    for invalid in [
        encode(&G1Affine::zero()).unwrap(),
        "ff".repeat(48),
        "00".repeat(48),
    ] {
        let mut p = f.presentation.clone();
        p.pseudonym = invalid;
        assert!(f.verified(&p).is_err());
        assert!(wire::to_json(&p).is_err());
    }
    let mut p = f.presentation.clone();
    let mut zero = proof(&p.credential_proof, 3, 2, claims_limits()).unwrap();
    zero.A_prime = G1Affine::zero();
    p.credential_proof = encode(&zero).unwrap();
    assert!(wire::to_json(&p).is_err());
    assert!(f.verified(&p).is_err());

    let altered = Claims::new([
        ("department", "sales"),
        ("display_name", "Bob"),
        ("role", "admin"),
    ])
    .unwrap();
    let (pending, request) =
        Holder::begin_enrollment(f.issuer.public(), &altered, &mut f.rng).unwrap();
    let other = pending
        .finish(
            &f.issuer
                .issue_blind(&request, &altered, &mut f.rng)
                .unwrap(),
        )
        .unwrap();
    let other = other
        .present(
            &f.challenge,
            &origin(),
            &ClaimKeys::new(["department", "role"]).unwrap(),
            NOW,
            &mut f.rng,
        )
        .unwrap();
    assert!(f.verified(&other).is_ok());
    let mut spliced = f.presentation.clone();
    spliced.disclosed_claims = other.disclosed_claims;
    assert!(matches!(
        f.verified(&spliced),
        Err(Error::InvalidPresentation)
    ));
}

#[test]
fn at_06_valid_bbs_proof_with_another_pseudonym_secret_is_rejected() {
    for selection in [claims().keys(), ClaimKeys::empty()] {
        let mut f = Fixture::new(claims(), selection.clone());
        // 只在测试中读合成备份，不增加指定或读取主秘密的生产接口。
        let backup: serde_json::Value =
            serde_json::from_slice(f.holder.export_secret().unwrap().expose_secret()).unwrap();
        let x = scalar(backup["secret"].as_str().unwrap()).unwrap();
        let y = x + Fr::from(1u64);
        let signature = signature(backup["credential"].as_str().unwrap()).unwrap();
        let params = parameters(6).unwrap();
        let values = messages(x, f.issuer.issuer_id(), &claims()).unwrap();
        let revealed = revealed(f.issuer.issuer_id(), 3, &f.presentation.disclosed_claims).unwrap();
        let base = pseudonym_base(f.issuer.issuer_id(), &origin()).unwrap();
        let b = nonzero(&mut f.rng);
        let pok = PoKOfSignatureG1Protocol::init(
            &mut f.rng,
            &signature,
            &params,
            values.iter().enumerate().map(|(i, m)| {
                if i == 0 {
                    MessageOrBlinding::blind_message_with(m, b)
                } else if revealed.contains_key(&i) {
                    MessageOrBlinding::RevealMessage(m)
                } else {
                    MessageOrBlinding::BlindMessageRandomly(m)
                }
            }),
        )
        .unwrap();
        let mut contribution = Vec::new();
        pok.challenge_contribution(&revealed, &params, &mut contribution)
            .unwrap();
        let mut p = f.presentation.clone();
        p.pseudonym = encode(&base.mul_bigint(y.into_bigint()).into_affine()).unwrap();
        p.pseudonym_commitment = encode(&base.mul_bigint(b.into_bigint()).into_affine()).unwrap();
        let key = public_key(&f.issuer.public().public_key).unwrap();
        let c = login_challenge(&p, &key, &base, &contribution).unwrap();
        let forged = pok.gen_proof(&c).unwrap();
        // 先证明 BBS+ 子证明本身有效，确保拒绝原因确实是共享秘密关系。
        forged.verify(&revealed, &c, key, params).unwrap();
        p.credential_proof = encode(&forged).unwrap();
        assert!(wire::to_json(&p).is_ok());
        assert!(matches!(f.verified(&p), Err(Error::InvalidPresentation)));
    }
}

#[test]
fn id_04_07_fixed_secret_across_claims_and_issuers() {
    let mut f = Fixture::standard();
    let subject = f.subject();
    let backup = f.holder.export_secret().unwrap();
    let original: serde_json::Value = serde_json::from_slice(backup.expose_secret()).unwrap();
    let x = scalar(original["secret"].as_str().unwrap()).unwrap();
    for (same_issuer, values) in [
        (true, Claims::empty()),
        (true, Claims::new([("role", "guest")]).unwrap()),
        (true, claims()),
        (false, claims()),
    ] {
        let other = crypto_sso_issuer::Issuer::generate("other", &mut f.rng).unwrap();
        let issuer = if same_issuer { &f.issuer } else { &other };
        let key_backup: serde_json::Value =
            serde_json::from_slice(issuer.export_secret().unwrap().expose_secret()).unwrap();
        let key = SecretKey(scalar(key_backup["secret_key"].as_str().unwrap()).unwrap());
        let messages = messages(x, issuer.issuer_id(), &values).unwrap();
        let sig = Signature::new(
            &mut f.rng,
            &messages,
            &key,
            &parameters(messages.len()).unwrap(),
        )
        .unwrap();
        let mut modified = original.clone();
        modified["issuer"] =
            serde_json::from_slice(&wire::to_json(issuer.public()).unwrap()).unwrap();
        modified["claims"] = serde_json::to_value(&values).unwrap();
        modified["credential"] = encode(&sig).unwrap().into();
        let holder =
            Holder::import_secret(&serde_json::to_vec(&modified).unwrap(), issuer.public())
                .unwrap();
        let app = Verifier::new(issuer.public().clone(), origin()).unwrap();
        let (c, b) = app
            .create_challenge(&ClaimKeys::empty(), NOW, &mut f.rng)
            .unwrap();
        let p = holder
            .present(&c, &origin(), &values.keys(), NOW, &mut f.rng)
            .unwrap();
        let checked = app.verify_proof(&p, &c, &b, NOW).unwrap();
        assert_eq!(checked.subject() == &subject, same_issuer);
        assert_eq!(holder.issuer_id(), issuer.issuer_id());
        if !same_issuer {
            assert!(matches!(
                f.verifier.verify_proof(&p, &c, &b, NOW),
                Err(Error::IssuerMismatch)
            ));
        }
    }
}

#[test]
fn pv_05_cross_purpose_and_context_reuse_fails() {
    let mut f = Fixture::standard();
    let mut p = f.presentation.clone();
    p.credential_proof = f.request.response_secret.clone();
    assert!(f.verified(&p).is_err());
    let mut request = f.request.clone();
    request.response_secret = f.presentation.credential_proof.clone();
    assert!(
        f.issuer
            .issue_blind(&request, &claims(), &mut f.rng)
            .is_err()
    );
    request = f.request.clone();
    request.commitment = f.presentation.pseudonym.clone();
    request.proof_commitment = f.presentation.pseudonym_commitment.clone();
    let proof = proof(&f.presentation.credential_proof, 3, 2, claims_limits()).unwrap();
    request.response_secret = encode(
        proof
            .get_resp_for_message(0, &[1, 2, 3, 5].into_iter().collect())
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        f.issuer.issue_blind(&request, &claims(), &mut f.rng),
        Err(crypto_sso_issuer::Error::InvalidEnrollmentProof)
    ));
    let labels = [
        b"crypto-sso/v1/issuer".as_slice(),
        b"crypto-sso/v1/enrollment-request",
        b"crypto-sso/v1/organization-message",
        b"crypto-sso/v1/claim",
        b"crypto-sso/v1/enrollment-proof",
        b"crypto-sso/v1/login-proof",
        b"crypto-sso/v1/pseudonym-scope",
        b"crypto-sso/v1/client-binding",
        PARAMETER_DOMAIN,
    ];
    let hashes: std::collections::BTreeSet<_> = labels
        .iter()
        .map(|label| hash_hex(label, &[b"same-input"]).unwrap())
        .collect();
    assert_eq!(hashes.len(), labels.len());
}
