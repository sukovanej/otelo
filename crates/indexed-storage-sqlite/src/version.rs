use std::fmt;

pub const STORAGE_VERSION: i64 = 2;

// The xxh3 of schema.sql at STORAGE_VERSION. A test fails when the schema changes and the hash
// does not, so a change of the schema bumps both.
pub const SCHEMA_HASH_AT_STORAGE_VERSION: u64 = 0x9397_c0b9_f13c_9cc1;

#[derive(Debug, PartialEq, Eq)]
pub struct OtherStorageVersion {
    pub found_version: i64,
}

impl fmt::Display for OtherStorageVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "telemetry.sqlite has storage version {} and this otelo writes {STORAGE_VERSION}. \
             Stop otelo and run otelo reindex.",
            self.found_version
        )
    }
}

impl std::error::Error for OtherStorageVersion {}
