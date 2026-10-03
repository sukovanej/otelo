use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use flate2::read::GzDecoder;
use tar::Archive;

const EXECUTABLE_NAME: &str = "otelo";

pub fn replace_executable_from_archive(archive_gz: &[u8], executable: &Path) -> Result<()> {
    let staged = name_staged_sibling(executable)?;
    let mut archive = Archive::new(GzDecoder::new(Cursor::new(archive_gz)));
    let mut found = false;
    for entry in archive.entries().context("reading the release archive")? {
        let mut entry = entry.context("reading the release archive")?;
        let path = entry.path().context("reading an archive entry path")?;
        if path.file_name() != Some(EXECUTABLE_NAME.as_ref())
            || !entry.header().entry_type().is_file()
        {
            continue;
        }
        entry
            .unpack(&staged)
            .with_context(|| format!("writing {}", staged.display()))?;
        found = true;
        break;
    }
    if !found {
        bail!("the release archive holds no file named {EXECUTABLE_NAME}");
    }
    mark_executable(&staged)?;
    fs::rename(&staged, executable)
        .inspect_err(|_| {
            let _ = fs::remove_file(&staged);
        })
        .with_context(|| format!("replacing {}", executable.display()))
}

// A temporary name next to the executable keeps the rename on one file system, which is what makes
// it atomic. A daemon that runs the old executable keeps running it until it restarts.
fn name_staged_sibling(executable: &Path) -> Result<PathBuf> {
    let directory = executable
        .parent()
        .with_context(|| format!("{} has no parent directory", executable.display()))?;
    let name = executable
        .file_name()
        .with_context(|| format!("{} has no file name", executable.display()))?
        .to_string_lossy();
    Ok(directory.join(format!(".{name}.update.{}", std::process::id())))
}

fn mark_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("marking {} executable", path.display()))
}
