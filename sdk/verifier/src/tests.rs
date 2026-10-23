use super::*;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

#[test]
fn ch_01_05_tm_01_07_challenge_and_cookie_unit_boundaries() {
    let public = IssuerPublic {
        version: 1,
        organization: "verifier-unit".into(),
        public_key: encode(&PublicKey::generate_using_secret_key(
            &SecretKey(Fr::from(123u64)),
            &parameters(0).unwrap(),
        ))
        .unwrap(),
    };
    let app = Verifier::new(public, Origin::parse("https://example.org").unwrap()).unwrap();
    let mut rng = ChaCha20Rng::from_seed([74; 32]);
    let (first, cookie) = app
        .create_challenge(&ClaimKeys::new(["role"]).unwrap(), 0, &mut rng)
        .unwrap();
    let (second, other) = app
        .create_challenge(&ClaimKeys::empty(), 0, &mut rng)
        .unwrap();
    assert_eq!(first.expires_at, 120);
    assert_ne!(first.nonce, second.nonce);
    assert_ne!(cookie.expose_cookie_value(), other.expose_cookie_value());
    assert_eq!(
        first.session_binding,
        binding_hash(cookie.expose_cookie_value()).unwrap()
    );
    assert!(
        ClientBindingSecret::from_cookie_value(&cookie.expose_cookie_value().to_uppercase())
            .is_err()
    );
    assert!(
        app.create_challenge(&ClaimKeys::empty(), u64::MAX - 120, &mut rng)
            .is_ok()
    );
    assert!(matches!(
        app.create_challenge(&ClaimKeys::empty(), u64::MAX - 119, &mut rng),
        Err(Error::InvalidChallengeTime)
    ));
}
