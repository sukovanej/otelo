mod digest;
mod github;
mod install;
mod owner;

pub use digest::verify_sha256_digest;
pub use github::{Asset, Channel, GithubReleases, Release, UpdateStep};
pub use install::replace_executable_from_archive;
pub use owner::{Environment, Owner, find_owner_of_executable};

#[must_use]
pub fn detect_release_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        _ => None,
    }
}

#[must_use]
pub fn build_archive_name(target: &str) -> String {
    format!("otelo-{target}.tar.gz")
}
