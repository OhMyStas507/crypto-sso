use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
pub fn equal(a: &str, b: &str) -> bool {
    bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn valid_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
