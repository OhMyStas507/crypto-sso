mod common;
use common::*;
use crypto_sso_core::__private as math;
use crypto_sso_core::__private::PrimeField;
use crypto_sso_holder::{Holder, PendingEnrollment};
use crypto_sso_issuer::{BlindCredential, EnrollmentRequest, Issuer};
use crypto_sso_verifier::*;
use sha2::{Digest, Sha256, Sha512};

const VECTORS: &str = include_str!("fixtures/v1/vectors.json");
fn lp(parts: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for part in parts {
        out.extend_from_slice(&u64::try_from(part.len()).unwrap().to_le_bytes());
        out.extend_from_slice(part);
    }
    out
}

#[test]
fn pv_03_04_frozen_vectors_and_independent_message_encoding() {
    let v: serde_json::Value = serde_json::from_str(VECTORS).unwrap();
    let get = |key: &str| v[key].as_str().unwrap();
    let public: IssuerPublic = wire::from_json(get("public").as_bytes()).unwrap();
    let request: EnrollmentRequest = wire::from_json(get("request").as_bytes()).unwrap();
    let blind: BlindCredential = wire::from_json(get("blind").as_bytes()).unwrap();
    let c: LoginChallenge = wire::from_json(get("challenge").as_bytes()).unwrap();
    assert_eq!(wire::to_json(&public).unwrap(), get("public").as_bytes());
    assert_eq!(wire::to_json(&request).unwrap(), get("request").as_bytes());
    assert_eq!(wire::to_json(&blind).unwrap(), get("blind").as_bytes());
    assert_eq!(wire::to_json(&c).unwrap(), get("challenge").as_bytes());
    assert_eq!(
        request.request_hash().unwrap().as_str(),
        get("request_hash")
    );
    let independent_hash = hex::encode(Sha256::digest(lp(&[
        b"crypto-sso/v1/enrollment-request",
        get("request").as_bytes(),
    ])));
    assert_eq!(independent_hash, get("request_hash"));
    let independent_id = hex::encode(Sha256::digest(lp(&[
        b"crypto-sso/v1/issuer",
        &[1],
        public.organization.as_bytes(),
        public.public_key.as_bytes(),
        v["parameters"]["rule"].as_str().unwrap().as_bytes(),
    ])));
    assert_eq!(independent_id, get("issuer_id"));
    assert_eq!(public.issuer_id().unwrap().as_str(), get("issuer_id"));
    let org = math::Fr::from_le_bytes_mod_order(&Sha512::digest(lp(&[
        b"crypto-sso/v1/organization-message",
        get("issuer_id").as_bytes(),
    ])));
    assert_eq!(math::encode(&org).unwrap(), get("organization_message"));
    for (key, value) in claims().iter() {
        let encoded = lp(&[
            b"crypto-sso/v1/claim",
            b"utf8-string",
            key.as_bytes(),
            value.as_bytes(),
        ]);
        let m = math::Fr::from_le_bytes_mod_order(&Sha512::digest(encoded));
        assert_eq!(
            math::encode(&m).unwrap(),
            v["claim_messages"][key].as_str().unwrap()
        );
        assert_eq!(m, math::claim_message(key, value).unwrap());
    }
    assert_eq!(get("count_message"), format!("03{}", "00".repeat(31)));
    let params = math::parameters(6).unwrap();
    assert_eq!(math::encode(&params.g1).unwrap(), v["parameters"]["g1"]);
    assert_eq!(math::encode(&params.g2).unwrap(), v["parameters"]["g2"]);
    assert_eq!(math::encode(&params.h_0).unwrap(), v["parameters"]["h0"]);
    for (i, expected) in v["parameters"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let mut input = lp(&[b"crypto-sso/v1/bbsplus/fixed-parameters", b"message"]);
        input.extend_from_slice(&u64::try_from(i).unwrap().to_le_bytes());
        assert_eq!(
            math::encode(&math::hash_g1(&input).unwrap()).unwrap(),
            expected.as_str().unwrap()
        );
        assert_eq!(
            math::encode(&params.h[i]).unwrap(),
            expected.as_str().unwrap()
        );
    }
    let issuer = Issuer::import_secret(get("issuer_backup").as_bytes(), &public).unwrap();
    assert_eq!(issuer.public(), &public);
    let pending =
        PendingEnrollment::import_secret(get("pending_backup").as_bytes(), &public).unwrap();
    let holder = pending.finish(&blind).unwrap();
    assert_eq!(holder.claims(), &claims());
    assert_eq!(
        Holder::import_secret(get("holder_backup").as_bytes(), &public)
            .unwrap()
            .claims(),
        &claims()
    );
    let app = Verifier::new(public, origin()).unwrap();
    let binding = ClientBindingSecret::from_cookie_value(get("cookie")).unwrap();
    for raw in v["presentations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .chain([get("presentation")])
    {
        let p: Presentation = wire::from_json(raw.as_bytes()).unwrap();
        assert_eq!(wire::to_json(&p).unwrap(), raw.as_bytes());
        assert_eq!(
            app.verify_proof(&p, &c, &binding, NOW)
                .unwrap()
                .subject()
                .as_str(),
            get("subject")
        );
    }
    let bad: Presentation = wire::from_json(get("invalid_presentation").as_bytes()).unwrap();
    assert!(matches!(
        app.verify_proof(&bad, &c, &binding, NOW),
        Err(Error::InvalidPresentation)
    ));
    // 固定种子重算结果必须与已提交向量相同；本测试没有写文件的能力。
    let mut fresh = Fixture::standard();
    assert_eq!(
        wire::to_json(&fresh.request).unwrap(),
        get("request").as_bytes()
    );
    assert_eq!(
        wire::to_json(&fresh.blind).unwrap(),
        get("blind").as_bytes()
    );
    assert_eq!(
        wire::to_json(&fresh.presentation).unwrap(),
        get("presentation").as_bytes()
    );
    for (selection, expected) in [
        ClaimKeys::empty(),
        ClaimKeys::new(["role"]).unwrap(),
        claims().keys(),
    ]
    .into_iter()
    .zip(v["presentations"].as_array().unwrap())
    {
        let p = fresh
            .holder
            .present(&fresh.challenge, &origin(), &selection, NOW, &mut fresh.rng)
            .unwrap();
        assert_eq!(
            wire::to_json(&p).unwrap(),
            expected.as_str().unwrap().as_bytes()
        );
    }
}
