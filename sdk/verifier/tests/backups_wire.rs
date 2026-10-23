mod common;
use common::*;
use crypto_sso_core::__private as math;
use crypto_sso_holder::{Error as HE, Holder, PendingEnrollment};
use crypto_sso_issuer::{BlindCredential, EnrollmentRequest, Issuer};
use crypto_sso_verifier::*;

#[test]
fn bk_01_02_03_04_05_restore_and_tampering() {
    let mut f = Fixture::standard();
    let public = f.issuer.public();
    let issuer_bytes = f.issuer.export_secret().unwrap();
    let pending_bytes = f.pending.export_secret().unwrap();
    let holder_bytes = f.holder.export_secret().unwrap();
    let issuer = Issuer::import_secret(issuer_bytes.expose_secret(), public).unwrap();
    assert_eq!(issuer.public(), public);
    let response = issuer
        .issue_blind(&f.request, &claims(), &mut f.rng)
        .unwrap();
    let pending = PendingEnrollment::import_secret(pending_bytes.expose_secret(), public).unwrap();
    assert!(pending.finish(&response).is_ok());
    let holder = Holder::import_secret(holder_bytes.expose_secret(), public).unwrap();
    let p = holder
        .present(
            &f.challenge,
            &origin(),
            &ClaimKeys::new(["display_name"]).unwrap(),
            NOW,
            &mut f.rng,
        )
        .unwrap();
    assert_eq!(f.verified(&p).unwrap().subject(), &f.subject());
    assert_eq!(
        f.verified(&p)
            .unwrap()
            .disclosed_claims()
            .get("display_name"),
        Some("Alice")
    );
    assert_eq!(holder.claims(), &claims());
    for field in ["secret", "blinding", "request_hash", "request"] {
        let changed = change_json(pending_bytes.expose_secret(), |v| match field {
            "request" => v["request"]["claims"]["role"] = "admin".into(),
            "request_hash" => v[field] = "aa".repeat(32).into(),
            _ => v[field] = math::encode(&math::Fr::from(1u64)).unwrap().into(),
        });
        assert!(
            PendingEnrollment::import_secret(&changed, public).is_err(),
            "{field}"
        );
    }
    // 保持请求哈希一致仍不足以通过：还需开口证明和秘密承诺一致。
    let changed = change_json(pending_bytes.expose_secret(), |v| {
        v["request"]["claims"]["role"] = "admin".into();
        let request: EnrollmentRequest =
            wire::from_json(&serde_json::to_vec(&v["request"]).unwrap()).unwrap();
        v["request_hash"] = request.request_hash().unwrap().as_str().into();
    });
    assert!(matches!(
        PendingEnrollment::import_secret(&changed, public),
        Err(HE::InvalidCredential)
    ));
    for field in ["secret", "credential", "claims", "zero"] {
        let changed = change_json(holder_bytes.expose_secret(), |v| match field {
            "claims" => v["claims"]["role"] = "admin".into(),
            "secret" => v[field] = math::encode(&math::Fr::from(1u64)).unwrap().into(),
            "zero" => v["secret"] = math::encode(&math::Fr::from(0u64)).unwrap().into(),
            _ => {
                let mut sig = math::signature(v[field].as_str().unwrap()).unwrap();
                sig.s += math::Fr::from(1u64);
                v[field] = math::encode(&sig).unwrap().into();
            }
        });
        assert!(
            matches!(
                Holder::import_secret(&changed, public),
                Err(HE::InvalidCredential)
            ),
            "{field}"
        );
    }
    let other = Issuer::generate("other", &mut f.rng).unwrap();
    assert!(Issuer::import_secret(issuer_bytes.expose_secret(), other.public()).is_err());
    assert!(
        PendingEnrollment::import_secret(pending_bytes.expose_secret(), other.public()).is_err()
    );
    assert!(Holder::import_secret(holder_bytes.expose_secret(), other.public()).is_err());
    for kind in 0..3 {
        let bytes = match kind {
            0 => issuer_bytes.expose_secret(),
            1 => pending_bytes.expose_secret(),
            _ => holder_bytes.expose_secret(),
        };
        let changed = change_json(bytes, |v| v["version"] = 2.into());
        match kind {
            0 => assert!(matches!(
                Issuer::import_secret(&changed, public),
                Err(crypto_sso_issuer::Error::Core(
                    CoreError::UnsupportedVersion { found: 2 }
                ))
            )),
            1 => assert!(matches!(
                PendingEnrollment::import_secret(&changed, public),
                Err(HE::Core(CoreError::UnsupportedVersion { found: 2 }))
            )),
            _ => assert!(matches!(
                Holder::import_secret(&changed, public),
                Err(HE::Core(CoreError::UnsupportedVersion { found: 2 }))
            )),
        }
    }
    for invalid in ["Upper", "", "bad key"] {
        let changed = change_json(holder_bytes.expose_secret(), |v| {
            v["claims"] = serde_json::json!({invalid:"value"})
        });
        assert!(Holder::import_secret(&changed, public).is_err());
    }
}

#[test]
fn wi_01_02_09_canonical_messages_hashes_and_mutable_dtos() {
    let mut f = Fixture::standard();
    assert_roundtrip(f.issuer.public());
    assert_roundtrip(&f.request);
    assert_roundtrip(&f.blind);
    assert_roundtrip(&f.challenge);
    assert_roundtrip(&f.presentation);
    let bytes = wire::to_json(&f.request).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let pretty = serde_json::to_string_pretty(&value)
        .unwrap()
        .replace("member", "\\u006dember");
    let request: EnrollmentRequest = wire::from_json(pretty.as_bytes()).unwrap();
    assert_eq!(
        request.request_hash().unwrap(),
        f.request.request_hash().unwrap()
    );
    assert_eq!(wire::to_json(&request).unwrap(), bytes);
    let fields = [
        "version",
        "issuer_id",
        "request_id",
        "commitment",
        "proof_commitment",
        "response_secret",
        "response_blinding",
        "claims",
    ];
    let text = String::from_utf8(bytes).unwrap();
    let positions: Vec<_> = fields
        .iter()
        .map(|field| text.find(&format!("\"{field}\":")).unwrap())
        .collect();
    assert!(positions.windows(2).all(|p| p[0] < p[1]));
    let mut public = f.issuer.public().clone();
    let id = public.issuer_id().unwrap();
    public.organization = "new-name".into();
    assert_ne!(public.issuer_id().unwrap(), id);
    assert_eq!(f.issuer.issuer_id(), &id);
    assert_eq!(f.holder.issuer_id(), &id);
    assert_eq!(f.verifier.issuer_id(), &id);
    public.version = 2;
    assert!(matches!(
        public.issuer_id(),
        Err(CoreError::UnsupportedVersion { found: 2 })
    ));
    let mut request = f.request.clone();
    let hash = request.request_hash().unwrap();
    request.claims = Claims::new([("new_key", "new-value")]).unwrap();
    assert_ne!(request.request_hash().unwrap(), hash);
    assert!(
        f.issuer
            .issue_blind(&request, &request.claims, &mut f.rng)
            .is_err()
    );
    request.commitment = "bad".into();
    assert!(request.request_hash().is_err());
}

fn rejects<T: wire::WireMessage>(json: &[u8]) {
    assert!(wire::from_json::<T>(json).is_err(), "输入应拒绝");
}
#[test]
fn wi_03_04_05_06_07_strict_json_and_bounded_proof_containers() {
    let f = Fixture::standard();
    let request = wire::to_json(&f.request).unwrap();
    let presentation = wire::to_json(&f.presentation).unwrap();
    let challenge = wire::to_json(&f.challenge).unwrap();
    for variant in 0..5 {
        let changed = change_json(&request, |v| match variant {
            0 => {
                v.as_object_mut().unwrap().remove("claims");
            }
            1 => v["claims"] = serde_json::Value::Null,
            2 => v["unknown"] = true.into(),
            3 => v["claims"] = serde_json::json!({"role":1}),
            _ => v["claims"] = serde_json::json!({"role":null}),
        });
        rejects::<EnrollmentRequest>(&changed);
    }
    let text = String::from_utf8(request.clone()).unwrap();
    rejects::<EnrollmentRequest>(
        text.replacen("\"version\":1", "\"version\":1,\"version\":1", 1)
            .as_bytes(),
    );
    rejects::<EnrollmentRequest>(
        text.replace(
            "\"role\":\"member\"",
            "\"role\":\"member\",\"role\":\"admin\"",
        )
        .as_bytes(),
    );
    for value in [
        serde_json::json!(["role", "department"]),
        serde_json::json!(["role", "role"]),
        serde_json::Value::Null,
    ] {
        rejects::<LoginChallenge>(&change_json(&challenge, |v| v["required_claims"] = value));
    }
    rejects::<LoginChallenge>(&change_json(&challenge, |v| {
        v.as_object_mut().unwrap().remove("required_claims");
    }));
    rejects::<BlindCredential>(&change_json(&wire::to_json(&f.blind).unwrap(), |v| {
        v.as_object_mut().unwrap().remove("claims");
    }));
    for field in ["claim_count", "disclosed_claims"] {
        rejects::<Presentation>(&change_json(&presentation, |v| {
            v.as_object_mut().unwrap().remove(field);
        }));
    }
    rejects::<Presentation>(&change_json(&presentation, |v| {
        v["disclosed_claims"] = serde_json::Value::Null
    }));
    for variant in 0..7 {
        let changed = change_json(&presentation, |v| match variant {
            0 => v["disclosed_claims"][0]["extra"] = true.into(),
            1 => {
                v["disclosed_claims"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("key");
            }
            2 => v["disclosed_claims"][0]["index"] = (-1).into(),
            3 => v["disclosed_claims"][0]["index"] = 1.5.into(),
            4 => v["disclosed_claims"][0]["index"] = 3.into(),
            5 => v["claim_count"] = u64::MAX.into(),
            _ => v["disclosed_claims"][1]["key"] = "department".into(),
        });
        rejects::<Presentation>(&changed);
    }
    let text = String::from_utf8(presentation.clone()).unwrap();
    rejects::<Presentation>(
        text.replacen("\"index\":0", "\"index\":18446744073709551616", 1)
            .as_bytes(),
    );
    rejects::<Presentation>(
        text.replacen("\"index\":0", "\"index\":0,\"index\":0", 1)
            .as_bytes(),
    );
    for bytes in [
        vec![0xff],
        b"{".to_vec(),
        [request.as_slice(), b"garbage"].concat(),
    ] {
        rejects::<EnrollmentRequest>(&bytes);
    }
    assert!(matches!(
        wire::from_json::<EnrollmentRequest>(&change_json(&request, |v| v["version"] = 2.into())),
        Err(CoreError::UnsupportedVersion { found: 2 })
    ));
    assert!(matches!(
        wire::from_json::<Presentation>(&change_json(
            &presentation,
            |v| v["challenge"]["version"] = 2.into()
        )),
        Err(CoreError::UnsupportedVersion { found: 2 })
    ));
    for origin in [
        "HTTPS://FORUM.EXAMPLE.ORG",
        "https://forum.example.org:443",
        "https://forum.example.org/",
    ] {
        rejects::<LoginChallenge>(&change_json(&challenge, |v| v["audience"] = origin.into()));
    }
    rejects::<EnrollmentRequest>(&change_json(&request, |v| {
        v["commitment"] = f.request.commitment.to_uppercase().into()
    }));
    for variant in 0..5 {
        let mut p = f.presentation.clone();
        let mut bytes = hex::decode(&p.credential_proof).unwrap();
        match variant {
            0 => bytes[305..313].copy_from_slice(&u64::MAX.to_le_bytes()),
            1 => bytes[305..313].copy_from_slice(&3u64.to_le_bytes()),
            2 => bytes[304] = 0,
            3 => {
                bytes.pop();
            }
            _ => {
                let last = bytes.len() - 1;
                bytes[last] = 1;
            }
        }
        p.credential_proof = hex::encode(bytes);
        assert!(wire::to_json(&p).is_err());
        rejects::<Presentation>(&change_json(&presentation, |v| {
            v["credential_proof"] = p.credential_proof.into()
        }));
    }
}

#[test]
fn wi_08_role_entry_points_revalidate_dtos() {
    let mut f = Fixture::standard();
    let mut request = f.request.clone();
    request.version = 2;
    assert!(matches!(
        f.issuer.issue_blind(&request, &claims(), &mut f.rng),
        Err(crypto_sso_issuer::Error::Core(
            CoreError::UnsupportedVersion { found: 2 }
        ))
    ));
    request = f.request.clone();
    request.commitment = "00".into();
    assert!(
        f.issuer
            .issue_blind(&request, &claims(), &mut f.rng)
            .is_err()
    );
    let mut response = f.blind.clone();
    response.version = 2;
    assert!(matches!(
        f.pending.finish(&response),
        Err(HE::Core(CoreError::UnsupportedVersion { found: 2 }))
    ));
    let mut public = f.issuer.public().clone();
    public.public_key = "00".into();
    assert!(Verifier::new(public.clone(), origin()).is_err());
    assert!(Holder::begin_enrollment(&public, &claims(), &mut f.rng).is_err());
    let mut c = f.challenge.clone();
    c.version = 2;
    assert!(matches!(
        f.holder
            .present(&c, &origin(), &ClaimKeys::empty(), NOW, &mut f.rng),
        Err(HE::Core(CoreError::UnsupportedVersion { found: 2 }))
    ));
    c = f.challenge.clone();
    c.session_binding = "GG".repeat(32);
    assert!(
        f.holder
            .present(&c, &origin(), &ClaimKeys::empty(), NOW, &mut f.rng)
            .is_err()
    );
    let mut p = f.presentation.clone();
    p.version = 2;
    assert!(matches!(
        f.verified(&p),
        Err(Error::Core(CoreError::UnsupportedVersion { found: 2 }))
    ));
    for (key, value) in [("Invalid", "ok"), ("role", &"x".repeat(1025))] {
        p = f.presentation.clone();
        p.disclosed_claims[0].key = key.into();
        p.disclosed_claims[0].value = value.into();
        assert!(f.verified(&p).is_err());
    }
}

#[test]
fn bk_06_logs_and_error_chains_are_redacted() {
    use std::error::Error as _;
    let f = Fixture::standard();
    let logs = format!(
        "{:?}{:?}{:?}{:?}{:?}{:?}",
        f.issuer.public(),
        f.request,
        f.blind,
        f.challenge,
        f.presentation,
        f.presentation.disclosed_claims
    );
    for hidden in [
        "engineering",
        "Alice",
        "member",
        &f.request.commitment,
        &f.blind.signature,
        &f.presentation.credential_proof,
    ] {
        assert!(!logs.contains(hidden));
    }
    let bad = change_json(f.holder.export_secret().unwrap().expose_secret(), |v| {
        v["secret"] = "PRIVATE_MARKER".into()
    });
    let error = Holder::import_secret(&bad, f.issuer.public())
        .err()
        .unwrap();
    assert!(!error.to_string().contains("PRIVATE_MARKER"));
    assert!(error.source().is_some());
    let mut source = error.source();
    while let Some(e) = source {
        assert!(!e.to_string().contains("PRIVATE_MARKER"));
        source = e.source();
    }
}
