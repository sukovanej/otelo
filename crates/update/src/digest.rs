use anyhow::{Result, bail};
use data_encoding::HEXLOWER;
use sha2::{Digest as _, Sha256};

pub fn verify_sha256_digest(bytes: &[u8], published_digest_file: &str) -> Result<()> {
    let Some(expected) = published_digest_file.split_whitespace().next() else {
        bail!("the published digest file is empty");
    };
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("the published digest {expected:?} is not a SHA-256 hex string");
    }
    let actual = HEXLOWER.encode(&Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("digest mismatch: the release publishes {expected}, the download hashes to {actual}");
    }
    Ok(())
}
