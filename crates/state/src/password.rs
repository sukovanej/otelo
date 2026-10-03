use anyhow::{Context, anyhow};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use data_encoding::BASE32_NOPAD;

const PASSWORD_BYTES: usize = 16;

pub fn generate_password() -> anyhow::Result<String> {
    let mut bytes = [0; PASSWORD_BYTES];
    getrandom::fill(&mut bytes).context("generate a password")?;
    Ok(BASE32_NOPAD.encode(&bytes))
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|error| anyhow!("hash the password: {error}"))
}

pub fn is_password_of_hash(candidate: &str, hash: &str) -> anyhow::Result<bool> {
    match Argon2::default().verify_password(candidate.as_bytes(), hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::PasswordInvalid) => Ok(false),
        Err(error) => Err(anyhow!("check the password: {error}")),
    }
}
