use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::Context;
use otelo_journal::Hour;
use otelo_query::Signal;

use crate::journal_files::JournalFiles;
use crate::segment::{SegmentDirectory, SegmentFiles};

const COMPRESSION_LEVEL: i32 = 3;
const RETENTION_INTERVAL: Duration = Duration::from_hours(1);

pub enum MaintenanceTask {
    CompressSegment { signal: Signal, hour: Hour },
    Stop,
}

pub fn run_maintenance_until_stopped(journal: &JournalFiles, tasks: &Receiver<MaintenanceTask>) {
    let mut next_retention_at = Instant::now() + RETENTION_INTERVAL;
    loop {
        match tasks.recv_timeout(next_retention_at.saturating_duration_since(Instant::now())) {
            Ok(MaintenanceTask::CompressSegment { signal, hour }) => {
                let segments = journal.signal_log(signal).segments();
                if let Err(error) = compress_segment(segments, hour) {
                    tracing::warn!(
                        "compress the journal segment of {signal:?} for the hour {hour:?}: {error:#}"
                    );
                }
            }
            Ok(MaintenanceTask::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if Instant::now() >= next_retention_at {
            journal.delete_segments_past_retention();
            next_retention_at = Instant::now() + RETENTION_INTERVAL;
        }
    }
}

pub fn compress_segment(segments: &SegmentDirectory, hour: Hour) -> anyhow::Result<()> {
    let uncompressed_path = segments.uncompressed_segment_path(hour);
    let temporary_path = segments.temporary_compressed_segment_path(hour);
    let compressed_path = segments.compressed_segment_path(hour);
    let mut uncompressed = File::open(&uncompressed_path)
        .with_context(|| format!("open {}", uncompressed_path.display()))?;
    let temporary = File::create(&temporary_path)
        .with_context(|| format!("create {}", temporary_path.display()))?;
    let mut encoder = zstd::Encoder::new(temporary, COMPRESSION_LEVEL)
        .with_context(|| format!("compress into {}", temporary_path.display()))?;
    io::copy(&mut uncompressed, &mut encoder)
        .with_context(|| format!("compress {}", uncompressed_path.display()))?;
    encoder
        .finish()
        .and_then(|temporary| temporary.sync_all())
        .with_context(|| format!("write {}", temporary_path.display()))?;
    fs::rename(&temporary_path, &compressed_path)
        .with_context(|| format!("rename {}", temporary_path.display()))?;
    segments.sync()?;
    fs::remove_file(&uncompressed_path)
        .with_context(|| format!("delete {}", uncompressed_path.display()))
}

pub fn delete_segments_past_retention(
    segments: &SegmentDirectory,
    listed_segments: &BTreeMap<Hour, SegmentFiles>,
    open_hour: Option<Hour>,
    retained_since: i64,
) -> anyhow::Result<()> {
    let expired_hours = listed_segments
        .iter()
        .filter(|(hour, _)| hour.ended_at() <= retained_since && Some(**hour) != open_hour);
    for (hour, files) in expired_hours {
        let paths = [
            files
                .uncompressed
                .then(|| segments.uncompressed_segment_path(*hour)),
            files
                .compressed
                .then(|| segments.compressed_segment_path(*hour)),
        ];
        for path in paths.into_iter().flatten() {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| format!("delete {}", path.display()));
                }
            }
        }
    }
    Ok(())
}
