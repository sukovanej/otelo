use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::Path;

use anyhow::{Context, anyhow};

const LOCK_FILE_NAME: &str = "telemetry.lock";

// The OS drops the lock with the file, also when the process dies.
pub struct TelemetryLock {
    _locked_file: File,
}

impl TelemetryLock {
    pub fn acquire(telemetry_directory: &Path) -> anyhow::Result<Self> {
        fs::create_dir_all(telemetry_directory).with_context(|| {
            format!(
                "make the telemetry directory {}",
                telemetry_directory.display()
            )
        })?;
        let path = telemetry_directory.join(LOCK_FILE_NAME);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("open {}", path.display()))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _locked_file: file }),
            Err(TryLockError::WouldBlock) => Err(anyhow!(
                "another otelo holds {}. Stop it and try again.",
                path.display()
            )),
            Err(TryLockError::Error(error)) => {
                Err(error).with_context(|| format!("lock {}", path.display()))
            }
        }
    }
}
