use super::*;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

#[test]
fn en_09_bk_01_05_organization_and_nonzero_keys() {
    let mut rng = ChaCha20Rng::from_seed([81; 32]);
    for bad in [
        "",
        " leading",
        "trailing ",
        "a\nb",
        &"x".repeat(129),
        &"中".repeat(43),
    ] {
        assert!(matches!(
            Issuer::generate(bad, &mut rng),
            Err(Error::Core(CoreError::InvalidIssuer))
        ));
    }
    for name in ["a", &"x".repeat(128), &"中".repeat(42)] {
        let issuer = Issuer::generate(name, &mut rng).unwrap();
        assert!(!issuer.key.0.is_zero());
        let bytes = issuer.export_secret().unwrap();
        let restored = Issuer::import_secret(bytes.expose_secret(), issuer.public()).unwrap();
        assert_eq!(issuer.public(), restored.public());
        assert_eq!(issuer.issuer_id(), restored.issuer_id());
        let other = Issuer::generate("other", &mut rng).unwrap();
        assert!(matches!(
            Issuer::import_secret(bytes.expose_secret(), other.public()),
            Err(Error::IssuerMismatch)
        ));
        for field in ["version", "secret_key"] {
            let mut value: serde_json::Value =
                serde_json::from_slice(bytes.expose_secret()).unwrap();
            value[field] = if field == "version" {
                2.into()
            } else {
                encode(&Fr::from(1u64)).unwrap().into()
            };
            assert!(
                Issuer::import_secret(&serde_json::to_vec(&value).unwrap(), issuer.public())
                    .is_err()
            );
        }
    }
}
