use anyhow::Context;
use data_encoding::BASE32_NOPAD;
use sha2::{Digest, Sha256};

const PASSWORD_BYTES: usize = 16;

pub fn generate_password() -> anyhow::Result<String> {
    let mut bytes = [0; PASSWORD_BYTES];
    getrandom::fill(&mut bytes).context("generate a password")?;
    Ok(BASE32_NOPAD.encode(&bytes))
}

// The password is 128 random bits, so a fast hash is as safe as a slow one.
pub fn hash_password(password: &str) -> [u8; 32] {
    Sha256::digest(password.as_bytes()).into()
}
