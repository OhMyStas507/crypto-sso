//! Generate a deployment password hash without accepting passwords in shell arguments.
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use rand::rngs::OsRng;

fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let password = zeroize::Zeroizing::new(std::env::var("ADMIN_BOOTSTRAP_PASSWORD")?);
    anyhow::ensure!(
        password.len() >= 12,
        "ADMIN_BOOTSTRAP_PASSWORD must contain at least 12 bytes"
    );
    let salt = SaltString::generate(&mut OsRng);
    println!(
        "{}",
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| anyhow::anyhow!("password hashing failed"))?
    );
    Ok(())
}
