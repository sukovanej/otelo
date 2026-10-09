use std::path::PathBuf;
use std::sync::Arc;

use otelo_indexed_storage_sqlite::{
    FrameMapper, ReindexProgress, TelemetryLock, reindex_from_journal,
};
use otelo_journal_files::JournalFiles;
use otelo_state::StateFile;

use crate::retention::Retention;
use crate::serve::{DEFAULT_DATA_DIR, JOURNAL_DIRECTORY_NAME};

#[derive(clap::Args)]
pub struct ReindexArgs {
    /// Directory of the state, the journal, and the telemetry of the stopped daemon
    #[arg(long = "data", value_name = "DATA", default_value = DEFAULT_DATA_DIR)]
    data_dir: PathBuf,
}

pub fn reindex_telemetry(args: &ReindexArgs) -> anyhow::Result<()> {
    let retention = Retention::from_defaults()?;
    // Opening the journal recovers its segments, which only one otelo may do at a time.
    let telemetry_lock = TelemetryLock::acquire(&args.data_dir)?;
    let indexed_attributes = StateFile::open(&args.data_dir)?.indexed_attributes()?;
    let opened = JournalFiles::open(otelo_journal_files::Config::new(
        args.data_dir.join(JOURNAL_DIRECTORY_NAME),
        retention.journal_days(),
    ))?;
    let map_frame: FrameMapper = Arc::new(otelo_otlp::map::map_journal_frame);
    let reindexed = reindex_from_journal(
        &telemetry_lock,
        retention.index_days(),
        indexed_attributes,
        Arc::clone(&opened.journal) as _,
        map_frame,
        print_progress,
    );
    opened.threads.stop_and_join()?;
    reindexed?;
    println!("reindexed {}", args.data_dir.display());
    Ok(())
}

fn print_progress(progress: ReindexProgress) {
    match progress {
        ReindexProgress::DeletedOtherStorageVersion { found_version } => {
            println!("deleted the telemetry of storage version {found_version}");
        }
        ReindexProgress::IndexedHour { signal, hour } => {
            let started_at = jiff::Timestamp::from_second(hour.hours_since_epoch() * 3_600)
                .expect("a journal hour is a valid instant");
            println!("{signal} {}", started_at.strftime("%Y-%m-%dT%H:00Z"));
        }
    }
}
