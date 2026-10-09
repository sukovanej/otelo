#![allow(dead_code, reason = "each test file uses a part")]

use std::num::NonZeroU16;
use std::path::Path;
use std::sync::Arc;

use otelo_indexed_storage::{Batch, PipelineMeters, PipelineReading, now_unix_nanos};
use otelo_indexed_storage_sqlite::{
    Config, FrameMapper, RetentionDays, index_journal_until_caught_up,
};
use otelo_journal::Journal;
use otelo_journal_files::{JournalFiles, OpenedJournal};
use otelo_query::Signal;

pub const INDEX_RETENTION_DAYS: RetentionDays = RetentionDays {
    logs: SEVEN_DAYS,
    traces: SEVEN_DAYS,
    metrics: SEVEN_DAYS,
};
pub const JOURNAL_RETENTION_DAYS: NonZeroU16 = NonZeroU16::new(30).expect("thirty is not zero");
const SEVEN_DAYS: NonZeroU16 = NonZeroU16::new(7).expect("seven is not zero");

// A frame holds the number of its batch, and the mapper hands the batch back.
pub fn map_numbered_frames(batches: Vec<Batch>) -> FrameMapper {
    Arc::new(move |_, request| {
        let number = u32::from_le_bytes(request.try_into()?);
        Ok(batches[number as usize].clone())
    })
}

pub fn append_numbered_frames(journal: &dyn Journal, signal: Signal, numbers: &[(u32, i64)]) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let tickets: Vec<_> = numbers
        .iter()
        .map(|&(number, received_at)| {
            journal
                .append_frame(signal, received_at, &number.to_le_bytes())
                .unwrap()
        })
        .collect();
    for ticket in tickets {
        runtime.block_on(ticket.wait_until_synced()).unwrap();
    }
}

pub fn open_journal(directory: &Path) -> OpenedJournal {
    JournalFiles::open(otelo_journal_files::Config::new(
        directory.join("journal"),
        JOURNAL_RETENTION_DAYS,
    ))
    .unwrap()
}

pub fn index_batches(config: Config, batches: Vec<Batch>) {
    index_batches_of_signal(config, Signal::Logs, batches);
}

pub fn index_batches_of_signal(
    config: Config,
    signal: Signal,
    batches: Vec<Batch>,
) -> PipelineReading {
    let meters = Arc::new(PipelineMeters::default());
    let journal_directory = tempfile::tempdir().unwrap();
    let opened = open_journal(journal_directory.path());
    let numbers: Vec<(u32, i64)> = (0..batches.len())
        .map(|number| (u32::try_from(number).unwrap(), now_unix_nanos()))
        .collect();
    append_numbered_frames(opened.journal.as_ref(), signal, &numbers);
    index_journal_until_caught_up(
        config,
        opened.journal,
        map_numbered_frames(batches),
        Arc::clone(&meters),
        |_, _| {},
    )
    .unwrap();
    opened.threads.stop_and_join().unwrap();
    meters.read_pipeline()
}
