use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use flate2::Compression;
use flate2::write::GzEncoder;
use otelo_update::replace_executable_from_archive;

fn build_tar_gz(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for (path, bytes, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(*mode);
        header.set_cksum();
        builder.append_data(&mut header, path, *bytes).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

fn build_release_archive(executable: &[u8]) -> Vec<u8> {
    build_tar_gz(&[
        (
            "otelo-x86_64-unknown-linux-gnu/CHANGELOG.md",
            b"changes",
            0o644,
        ),
        ("otelo-x86_64-unknown-linux-gnu/otelo", executable, 0o755),
    ])
}

fn read_mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn the_executable_is_replaced_in_place_and_stays_executable() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("otelo");
    std::fs::write(&executable, b"old").unwrap();

    replace_executable_from_archive(&build_release_archive(b"new"), &executable).unwrap();

    assert_eq!(std::fs::read(&executable).unwrap(), b"new");
    assert_eq!(read_mode(&executable), 0o755);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn a_renamed_executable_takes_the_otelo_of_the_archive() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("otelo-prod");
    std::fs::write(&executable, b"old").unwrap();

    replace_executable_from_archive(&build_release_archive(b"new"), &executable).unwrap();

    assert_eq!(std::fs::read(&executable).unwrap(), b"new");
}

#[test]
fn an_archive_without_the_executable_leaves_the_old_one() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("otelo");
    std::fs::write(&executable, b"old").unwrap();
    let archive = build_tar_gz(&[("otelo-x/CHANGELOG.md", b"changes", 0o644)]);

    let error = replace_executable_from_archive(&archive, &executable).unwrap_err();

    assert!(error.to_string().contains("no file named otelo"), "{error}");
    assert_eq!(std::fs::read(&executable).unwrap(), b"old");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}
