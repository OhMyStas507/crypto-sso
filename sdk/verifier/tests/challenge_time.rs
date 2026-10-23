mod common;
use common::*;
use crypto_sso_holder::Error as HE;
use crypto_sso_issuer::Issuer;
use crypto_sso_verifier::*;

#[test]
fn ch_01_02_03_04_05_06_07_context_and_cookie_binding() {
    let mut f = Fixture::standard();
    let other_issuer = Issuer::generate("other", &mut f.rng).unwrap();
    let other_origin = Origin::parse("https://other.example.org").unwrap();
    let (another, binding) = f
        .verifier
        .create_challenge(&ClaimKeys::empty(), NOW, &mut f.rng)
        .unwrap();
    assert_ne!(another.nonce, f.challenge.nonce);
    assert_ne!(
        binding.expose_cookie_value(),
        f.binding.expose_cookie_value()
    );
    assert_ne!(another.nonce.as_str(), binding.expose_cookie_value());
    assert_ne!(another.session_binding, f.challenge.session_binding);
    assert_eq!(another.nonce.as_str().len(), 64);
    assert_eq!(binding.expose_cookie_value().len(), 64);
    assert!(
        !String::from_utf8(wire::to_json(&another).unwrap())
            .unwrap()
            .contains(binding.expose_cookie_value())
    );
    assert!(matches!(
        f.verifier
            .verify_proof(&f.presentation, &f.challenge, &binding, NOW),
        Err(Error::ClientBindingMismatch)
    ));
    for cookie in [
        "",
        &"ab".repeat(31),
        &"ab".repeat(33),
        &"AB".repeat(32),
        &"zz".repeat(32),
    ] {
        assert!(ClientBindingSecret::from_cookie_value(cookie).is_err());
    }
    let restored = ClientBindingSecret::from_cookie_value(f.binding.expose_cookie_value()).unwrap();
    assert!(
        f.verifier
            .verify_proof(&f.presentation, &f.challenge, &restored, NOW)
            .is_ok()
    );
    for field in [
        "version",
        "issuer",
        "origin",
        "nonce",
        "issued_at",
        "expires_at",
        "binding",
        "required",
    ] {
        let mut p = f.presentation.clone();
        match field {
            "version" => p.challenge.version = 2,
            "issuer" => p.challenge.issuer_id = other_issuer.issuer_id().clone(),
            "origin" => p.challenge.audience = other_origin.clone(),
            "nonce" => p.challenge.nonce = another.nonce.clone(),
            "issued_at" => p.challenge.issued_at -= 1,
            "expires_at" => p.challenge.expires_at -= 1,
            "binding" => p.challenge.session_binding = another.session_binding.clone(),
            _ => p.challenge.required_claims = ClaimKeys::new(["role"]).unwrap(),
        }
        assert!(
            matches!(f.verified(&p), Err(Error::ChallengeMismatch)),
            "{field}"
        );
        // 同步更改存储，才能抵达上下文或密码学检查，避免被首项比较掩盖。
        let result = f.verifier.verify_proof(
            &p,
            &p.challenge,
            if field == "binding" {
                &binding
            } else {
                &f.binding
            },
            NOW,
        );
        match field {
            "version" => assert!(matches!(
                result,
                Err(Error::Core(CoreError::UnsupportedVersion { found: 2 }))
            )),
            "issuer" => assert!(matches!(result, Err(Error::IssuerMismatch))),
            "origin" => assert!(matches!(result, Err(Error::OriginMismatch))),
            "issued_at" => assert!(matches!(result, Err(Error::InvalidChallengeTime))),
            _ => assert!(matches!(result, Err(Error::InvalidPresentation)), "{field}"),
        }
    }
    let mut c = f.challenge.clone();
    c.issuer_id = other_issuer.issuer_id().clone();
    assert!(matches!(
        f.holder
            .present(&c, &origin(), &ClaimKeys::empty(), NOW, &mut f.rng),
        Err(HE::IssuerMismatch)
    ));
    assert!(matches!(
        f.holder.present(
            &f.challenge,
            &other_origin,
            &ClaimKeys::empty(),
            NOW,
            &mut f.rng
        ),
        Err(HE::OriginMismatch)
    ));
    let mut p = f.presentation.clone();
    p.challenge.issued_at += 1;
    p.challenge.expires_at += 1;
    assert!(matches!(
        f.verifier
            .verify_proof(&p, &p.challenge, &f.binding, NOW + 1),
        Err(Error::InvalidPresentation)
    ));
    assert!(Subject::parse(&p.pseudonym).is_ok());
    p.credential_proof.clear();
    assert!(
        f.verifier
            .verify_proof(&p, &p.challenge, &f.binding, NOW + 1)
            .is_err()
    );
}

#[test]
fn tm_01_02_03_04_05_06_07_08_both_roles_and_u64_boundaries() {
    let mut f = Fixture::standard();
    assert_eq!(f.challenge.issued_at, NOW);
    assert_eq!(f.challenge.expires_at, NOW + 120);
    // 成功边界均针对所改挑战重新生成有效证明。
    for (issued, ttl, now) in [
        (NOW, 120, NOW + 119),
        (NOW + 30, 120, NOW),
        (NOW, 1, NOW),
        (NOW, 119, NOW),
        (NOW, 120, NOW),
        (0, 1, 0),
        (u64::MAX - 120, 120, u64::MAX - 1),
    ] {
        let mut c = f.challenge.clone();
        c.issued_at = issued;
        c.expires_at = issued + ttl;
        let p = f
            .holder
            .present(&c, &origin(), &ClaimKeys::empty(), now, &mut f.rng)
            .unwrap();
        assert!(f.verifier.verify_proof(&p, &c, &f.binding, now).is_ok());
    }
    for (issued, expires, now, expired) in [
        (NOW, NOW + 120, NOW + 120, true),
        (NOW, NOW + 120, NOW + 121, true),
        (NOW + 31, NOW + 151, NOW, false),
        (NOW, NOW, NOW, false),
        (NOW, NOW - 1, NOW, false),
        (NOW, NOW + 121, NOW, false),
        (0, 0, 0, false),
        (u64::MAX, u64::MAX, u64::MAX, false),
        (u64::MAX - 120, u64::MAX, u64::MAX, true),
    ] {
        let mut c = f.challenge.clone();
        c.issued_at = issued;
        c.expires_at = expires;
        let h = f
            .holder
            .present(&c, &origin(), &ClaimKeys::empty(), now, &mut f.rng);
        let mut p = f.presentation.clone();
        p.challenge = c.clone();
        let v = f.verifier.verify_proof(&p, &c, &f.binding, now);
        if expired {
            assert!(matches!(h, Err(HE::ChallengeExpired)));
            assert!(matches!(v, Err(Error::ChallengeExpired)));
        } else {
            assert!(matches!(h, Err(HE::InvalidChallengeTime)));
            assert!(matches!(v, Err(Error::InvalidChallengeTime)));
        }
    }
    let (max, _) = f
        .verifier
        .create_challenge(&ClaimKeys::empty(), u64::MAX - 120, &mut f.rng)
        .unwrap();
    assert_eq!(max.expires_at, u64::MAX);
    assert!(matches!(
        f.verifier
            .create_challenge(&ClaimKeys::empty(), u64::MAX - 119, &mut f.rng),
        Err(Error::InvalidChallengeTime)
    ));
    let last = f
        .holder
        .present(
            &f.challenge,
            &origin(),
            &ClaimKeys::empty(),
            NOW + 119,
            &mut f.rng,
        )
        .unwrap();
    assert!(matches!(
        f.verifier
            .verify_proof(&last, &f.challenge, &f.binding, NOW + 120),
        Err(Error::ChallengeExpired)
    ));
}
