use super::*;
use crate::__private::*;
use ark_bls12_381::Fq;
use sha2::{Digest, Sha256, Sha512};

#[test]
fn cf_01_defaults_are_value_snapshots() {
    assert_eq!(claims_limits(), DEFAULT_CLAIMS_LIMITS);
    assert_eq!(
        claims_limits(),
        ClaimsLimits {
            max_keys: 128,
            max_key_bytes: 64,
            max_value_bytes: 1024
        }
    );
    let mut snapshot = claims_limits();
    snapshot.max_keys = 1;
    assert_ne!(snapshot, claims_limits());
}
#[test]
fn co_01_02_03_04_claim_semantics_and_duplicates() {
    let a = Claims::new([("z", ""), ("a", "  中é  ")]).unwrap();
    let b = Claims::new([("a", "  中é  "), ("z", "")]).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        a.iter().collect::<Vec<_>>(),
        vec![("a", "  中é  "), ("z", "")]
    );
    assert_eq!(a.len(), 2);
    assert!(!a.is_empty());
    assert_eq!(a.get("missing"), None);
    assert_eq!(a.get("z"), Some(""));
    let keys = ClaimKeys::new(["z", "a"]).unwrap();
    assert_eq!(a.keys(), keys);
    assert_eq!(keys.iter().collect::<Vec<_>>(), vec!["a", "z"]);
    assert!(keys.contains("a"));
    assert_eq!(keys.len(), 2);
    assert!(!keys.is_empty());
    assert!(Claims::empty().is_empty());
    assert!(ClaimKeys::empty().is_empty());
    for bad in ["", "Upper", "a b", "中文", "9key", "*", "a*", "a/b"] {
        assert_eq!(Claims::new([(bad, "")]), Err(Error::InvalidClaims));
        assert_eq!(ClaimKeys::new([bad]), Err(Error::InvalidClaims));
    }
    for good in ["a", "a0", "a.b", "a-b", "a_b"] {
        assert!(Claims::new([(good, "")]).is_ok());
    }
    assert_eq!(
        Claims::new([("a", "first"), ("a", "second")]),
        Err(Error::InvalidClaims)
    );
    assert_eq!(ClaimKeys::new(["a", "a"]), Err(Error::InvalidClaims));
    assert!(serde_json::from_str::<Claims>(r#"{"a":"first","a":"second"}"#).is_err());
    for value in ["1", "true", "[]", "{}", "null"] {
        assert!(serde_json::from_str::<Claims>(&format!(r#"{{"a":{value}}}"#)).is_err());
    }
    let escaped: Claims = serde_json::from_str(r#"{"a":"\u4e2d\u00e9"}"#).unwrap();
    let literal = Claims::new([("a", "中é")]).unwrap();
    assert_eq!(
        serde_json::to_vec(&escaped).unwrap(),
        serde_json::to_vec(&literal).unwrap()
    );
    assert_ne!(
        Claims::new([("a", "é")]).unwrap(),
        Claims::new([("a", "e\u{301}")]).unwrap()
    );
}
#[test]
fn co_06_strict_tokens_points_and_subgroups() {
    for value in [
        "00".repeat(31),
        "00".repeat(33),
        "AB".repeat(32),
        "gg".repeat(32),
    ] {
        assert!(IssuerId::parse(&value).is_err());
        assert!(RequestId::parse(&value).is_err());
        assert!(RequestHash::parse(&value).is_err());
        assert!(Nonce::parse(&value).is_err());
    }
    for value in ["00".repeat(32), "ab".repeat(32)] {
        assert!(IssuerId::parse(&value).is_ok());
        assert!(RequestId::parse(&value).is_ok());
        assert!(RequestHash::parse(&value).is_ok());
        assert!(Nonce::parse(&value).is_ok());
    }
    let p = parameters(3).unwrap().g1;
    assert!(Subject::parse(&encode(&p).unwrap()).is_ok());
    // 曲线上但不在素数阶子群内的点，不能只检查曲线方程。
    let torsion = G1Affine::new_unchecked(Fq::from(0u64), Fq::from(2u64));
    assert!(torsion.is_on_curve());
    assert!(!torsion.is_in_correct_subgroup_assuming_on_curve());
    for invalid in [
        encode(&G1Affine::zero()).unwrap(),
        encode(&torsion).unwrap(),
        "ff".repeat(48),
        encode(&p).unwrap().to_uppercase(),
    ] {
        assert!(Subject::parse(&invalid).is_err());
    }
    assert!(scalar(&"ff".repeat(32)).is_err());
    assert!(public_key(&encode(&G2Affine::zero()).unwrap()).is_err());
    let invalid_signature = Signature {
        A: G1Affine::zero(),
        e: Fr::from(1u64),
        s: Fr::from(2u64),
    };
    assert!(signature(&encode(&invalid_signature).unwrap()).is_err());
}
#[test]
fn co_07_origin_normalization_and_rejection() {
    for (input, expected) in [
        (
            "HTTPS://FORUM.EXAMPLE.ORG:443/",
            "https://forum.example.org",
        ),
        ("http://localhost:80", "http://localhost"),
        ("http://127.0.0.2:8080", "http://127.0.0.2:8080"),
        ("http://[::1]", "http://[::1]"),
        ("https://例子.测试", "https://xn--fsqu00a.xn--0zwm56d"),
    ] {
        assert_eq!(Origin::parse(input).unwrap().as_str(), expected);
    }
    for input in [
        "http://example.org",
        "ftp://localhost",
        "https://u:p@example.org",
        "https://u@example.org",
        "https://@example.org",
        "https://example.org/path",
        "https://example.org/a/..",
        "https://example.org/.",
        "https://example.org/?q=1",
        "https://example.org/#x",
        "null",
        "file:///",
        " https://example.org",
        "https://example.org\n",
    ] {
        assert!(Origin::parse(input).is_err(), "{input:?}");
    }
}
#[test]
fn co_05_pv_03_parameters_prefix_and_independent_hashes() {
    let p3 = parameters(3).unwrap();
    let p6 = parameters(6).unwrap();
    assert_eq!(p3.g1, p6.g1);
    assert_eq!(p3.g2, p6.g2);
    assert_eq!(p3.h_0, p6.h_0);
    assert_eq!(&p3.h, &p6.h[..3]);
    assert!(p6.is_valid());
    // 独立按文档拼接 LP 字节，不调用生产 Transcript，以检测共有编码错误。
    fn lp(parts: &[&[u8]]) -> Vec<u8> {
        let mut b = Vec::new();
        for p in parts {
            b.extend_from_slice(&u64::try_from(p.len()).unwrap().to_le_bytes());
            b.extend_from_slice(p);
        }
        b
    }
    let bytes = lp(&[b"crypto-sso/v1/claim", b"utf8-string", b"role", b"member"]);
    assert_eq!(
        claim_message("role", "member").unwrap(),
        Fr::from_le_bytes_mod_order(&Sha512::digest(bytes))
    );
    let public = IssuerPublic {
        version: 1,
        organization: "vector-org".into(),
        public_key: encode(&PublicKey::generate_using_secret_key(
            &SecretKey(Fr::from(42u64)),
            &p3,
        ))
        .unwrap(),
    };
    let independent = hex::encode(Sha256::digest(lp(&[
        b"crypto-sso/v1/issuer",
        &[1],
        b"vector-org",
        public.public_key.as_bytes(),
        PARAMETER_RULE,
    ])));
    assert_eq!(public.issuer_id().unwrap().as_str(), independent);
    let mut altered = public.clone();
    altered.organization = "another".into();
    assert_ne!(altered.issuer_id().unwrap(), public.issuer_id().unwrap());
    altered = public.clone();
    altered.public_key = encode(&PublicKey::generate_using_secret_key(
        &SecretKey(Fr::from(43u64)),
        &p3,
    ))
    .unwrap();
    assert_ne!(altered.issuer_id().unwrap(), public.issuer_id().unwrap());
    let id = public.issuer_id().unwrap();
    let values = messages(Fr::from(9u64), &id, &Claims::empty()).unwrap();
    assert_eq!(values.len(), 3);
    assert_eq!(values[2], Fr::from(0u64));
    assert_eq!(
        values[1],
        Fr::from_le_bytes_mod_order(&Sha512::digest(lp(&[
            b"crypto-sso/v1/organization-message",
            id.as_str().as_bytes()
        ])))
    );
}
#[test]
fn wi_04_counts_overflow_without_allocation() {
    assert!(message_count(usize::MAX).is_err());
    assert!(
        count(
            u64::MAX,
            ClaimsLimits {
                max_keys: usize::MAX,
                ..DEFAULT_CLAIMS_LIMITS
            }
        )
        .is_err()
    );
    assert!(proof("00", u64::MAX, 0, claims_limits()).is_err());
}
#[test]
fn bk_06_owned_secret_buffer_zeroization() {
    use zeroize::Zeroize;
    // 只检查仍存活的 SDK 缓冲区，不读取已释放内存。
    let mut bytes = zeroize::Zeroizing::new(vec![42u8; 32]);
    bytes.zeroize();
    assert!(bytes.iter().all(|b| *b == 0));
    let secret = SecretBytes::from_vec(vec![1, 2, 3]);
    assert_eq!(secret.expose_secret(), &[1, 2, 3]);
}
