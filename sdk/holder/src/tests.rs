use super::*;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

/// 直接用公共数学原语模拟签发端，保持 holder 测试不依赖另一个角色 crate。
fn credential() -> (Holder, PendingEnrollment, BlindCredential) {
    let mut rng = ChaCha20Rng::from_seed([73; 32]);
    let key = SecretKey(nonzero(&mut rng));
    let public = IssuerPublic {
        version: 1,
        organization: "holder-unit".into(),
        public_key: encode(&PublicKey::generate_using_secret_key(
            &key,
            &parameters(0).unwrap(),
        ))
        .unwrap(),
    };
    let values = Claims::new([("a", "first"), ("b", "second"), ("c", "third")]).unwrap();
    let (pending, request) = Holder::begin_enrollment(&public, &values, &mut rng).unwrap();
    let messages = messages(Fr::zero(), pending.issuer_id(), &values).unwrap();
    let params = parameters(messages.len()).unwrap();
    assert!(verify_opening(&request, &params).unwrap());
    let signature = Signature::new_with_committed_messages(
        &mut rng,
        &point(&request.commitment).unwrap(),
        messages.iter().enumerate().skip(1).collect(),
        &key,
        &params,
    )
    .unwrap();
    let response = BlindCredential {
        version: 1,
        issuer_id: pending.issuer_id().clone(),
        request_id: request.request_id.clone(),
        request_hash: request.request_hash().unwrap(),
        signature: encode(&signature).unwrap(),
        claims: values,
    };
    (pending.finish(&response).unwrap(), pending, response)
}
#[test]
fn en_07_08_bk_02_pending_retry_and_opening_validation() {
    let (holder, pending, response) = credential();
    let mut bad = response.clone();
    let mut sig = signature(&bad.signature).unwrap();
    sig.e += Fr::from(1u64);
    bad.signature = encode(&sig).unwrap();
    assert!(matches!(
        pending.finish(&bad),
        Err(Error::InvalidCredential)
    ));
    assert!(pending.finish(&response).is_ok());
    let backup = pending.export_secret().unwrap();
    let restored =
        PendingEnrollment::import_secret(backup.expose_secret(), holder.issuer()).unwrap();
    assert!(restored.finish(&response).is_ok());
    let mut raw: serde_json::Value = serde_json::from_slice(backup.expose_secret()).unwrap();
    raw["blinding"] = encode(&Fr::from(1u64)).unwrap().into();
    assert!(matches!(
        PendingEnrollment::import_secret(&serde_json::to_vec(&raw).unwrap(), holder.issuer()),
        Err(Error::InvalidCredential)
    ));
}
#[test]
fn bk_03_04_holder_preserves_all_attributes_and_revalidates_signature() {
    let (holder, _, _) = credential();
    let backup = holder.export_secret().unwrap();
    let restored = Holder::import_secret(backup.expose_secret(), holder.issuer()).unwrap();
    assert_eq!(holder.claims(), restored.claims());
    for field in ["claims", "secret", "credential"] {
        let mut raw: serde_json::Value = serde_json::from_slice(backup.expose_secret()).unwrap();
        match field {
            "claims" => raw[field]["b"] = "tampered".into(),
            "secret" => raw[field] = encode(&Fr::from(1u64)).unwrap().into(),
            _ => {
                let mut sig = signature(raw[field].as_str().unwrap()).unwrap();
                sig.s += Fr::from(1u64);
                raw[field] = encode(&sig).unwrap().into();
            }
        }
        assert!(matches!(
            Holder::import_secret(&serde_json::to_vec(&raw).unwrap(), holder.issuer()),
            Err(Error::InvalidCredential)
        ));
    }
}
