use std::path::{Path, PathBuf};

use otelo_update::{Environment, Owner, find_owner_of_executable};

fn build_environment() -> Environment {
    Environment {
        cargo_home: Some(PathBuf::from("/home/me/.cargo")),
        homebrew_prefix: None,
    }
}

fn find_owner(executable: &str) -> Option<Owner> {
    find_owner_of_executable(Path::new(executable), &build_environment())
}

#[test]
fn an_executable_the_installer_wrote_has_no_other_owner() {
    assert_eq!(find_owner("/home/me/.local/bin/otelo"), None);
    assert_eq!(find_owner("/usr/local/bin/otelo"), None);
}

#[test]
fn cargo_install_owns_the_bin_of_cargo_home() {
    assert_eq!(find_owner("/home/me/.cargo/bin/otelo"), Some(Owner::Cargo));
}

#[test]
fn homebrew_owns_its_prefixes() {
    assert_eq!(
        find_owner("/opt/homebrew/Cellar/otelo/0.1/bin/otelo"),
        Some(Owner::Homebrew)
    );
    assert_eq!(
        find_owner("/usr/local/Cellar/otelo/0.1/bin/otelo"),
        Some(Owner::Homebrew)
    );
    let custom_prefix = Environment {
        homebrew_prefix: Some(PathBuf::from("/brew")),
        ..build_environment()
    };
    assert_eq!(
        find_owner_of_executable(Path::new("/brew/bin/otelo"), &custom_prefix),
        Some(Owner::Homebrew)
    );
}

#[test]
fn nix_and_system_paths_are_refused() {
    assert_eq!(
        find_owner("/nix/store/abc-otelo/bin/otelo"),
        Some(Owner::Nix)
    );
    assert_eq!(find_owner("/usr/bin/otelo"), Some(Owner::SystemPackage));
}

#[test]
fn the_refusal_names_the_owner_and_the_path() {
    let refusal = Owner::Cargo.describe_refusal(Path::new("/home/me/.cargo/bin/otelo"));

    assert!(refusal.contains("cargo install"), "{refusal}");
    assert!(refusal.contains("/home/me/.cargo/bin/otelo"), "{refusal}");
}

#[test]
fn a_build_in_a_cache_directory_is_refused() {
    let target = tempfile::tempdir().unwrap();
    std::fs::write(
        target.path().join("CACHEDIR.TAG"),
        "Signature: 8a477f597d28d172789f06886806bc55\n",
    )
    .unwrap();
    let debug = target.path().join("debug");
    std::fs::create_dir_all(&debug).unwrap();

    assert_eq!(
        find_owner_of_executable(&debug.join("otelo"), &build_environment()),
        Some(Owner::BuildCache)
    );
}
