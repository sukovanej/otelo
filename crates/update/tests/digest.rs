use otelo_update::verify_sha256_digest;

const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

#[test]
fn a_matching_digest_passes() {
    verify_sha256_digest(b"hello", &format!("{HELLO_SHA256} *otelo.tar.gz\n")).unwrap();
}

#[test]
fn the_digest_compares_without_case() {
    verify_sha256_digest(b"hello", &HELLO_SHA256.to_uppercase()).unwrap();
}

#[test]
fn a_changed_download_fails() {
    let error = verify_sha256_digest(b"hellp", HELLO_SHA256).unwrap_err();

    assert!(error.to_string().contains("digest mismatch"), "{error}");
}

#[test]
fn a_malformed_digest_file_fails() {
    assert!(verify_sha256_digest(b"hello", "").is_err());
    assert!(verify_sha256_digest(b"hello", "not-a-digest").is_err());
    assert!(verify_sha256_digest(b"hello", &HELLO_SHA256[..40]).is_err());
}
