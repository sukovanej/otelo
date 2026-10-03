use std::num::NonZeroU16;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use anyhow::{Context, anyhow};
use otelo_journal::{Frames, Journal, Position, SyncTicket, SyncedEndInbox, open_synced_end_queue};
use otelo_query::Signal;

use crate::maintenance::{
    MaintenanceTask, delete_segments_past_retention, run_maintenance_until_stopped,
};
use crate::recovery::recover_segments;
use crate::segment::SegmentDirectory;
use crate::signal_log::SignalLog;

const DEFAULT_RETENTION_DAYS: NonZeroU16 = NonZeroU16::new(30).expect("thirty is not zero");
const NANOS_PER_DAY: i64 = 24 * 3_600 * 1_000_000_000;

#[derive(Clone)]
pub struct Config {
    pub directory: PathBuf,
    pub retention_days: NonZeroU16,
}

impl Config {
    #[must_use]
    pub const fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            retention_days: DEFAULT_RETENTION_DAYS,
        }
    }

    const fn retained_since(&self, now: i64) -> i64 {
        now - self.retention_days.get() as i64 * NANOS_PER_DAY
    }
}

pub struct JournalFiles {
    config: Config,
    logs: SignalLog,
    spans: SignalLog,
    metrics: SignalLog,
}

pub struct OpenedJournal {
    pub journal: Arc<JournalFiles>,
    pub threads: JournalThreads,
    pub synced_ends: SyncedEndInbox,
}

impl JournalFiles {
    pub fn open(config: Config) -> anyhow::Result<OpenedJournal> {
        let now = now_unix_nanos();
        let (maintenance_tasks, maintenance_inbox) = mpsc::channel();
        let open_signal_log = |signal: Signal| -> anyhow::Result<SignalLog> {
            let segments = SegmentDirectory::new(&config.directory, signal);
            let recovered = recover_segments(&segments, config.retained_since(now), now)?;
            Ok(SignalLog::new(
                signal,
                segments,
                recovered.open_segment,
                recovered.newest_closed_hour,
                maintenance_tasks.clone(),
            ))
        };
        let journal = Arc::new(Self {
            logs: open_signal_log(Signal::Logs)?,
            spans: open_signal_log(Signal::Spans)?,
            metrics: open_signal_log(Signal::Metrics)?,
            config,
        });
        let (synced_end_sender, synced_ends) = open_synced_end_queue();
        let mut sync_threads = Vec::new();
        for signal in Signal::ALL {
            let journal = Arc::clone(&journal);
            let synced_end_sender = synced_end_sender.clone();
            let thread = thread::Builder::new()
                .name(format!("journal-sync-{}", signal.name()))
                .spawn(move || {
                    journal
                        .signal_log(signal)
                        .sync_frames_until_stopped(&synced_end_sender);
                })
                .context("start the sync of the journal")?;
            sync_threads.push(thread);
        }
        let maintenance_thread = thread::Builder::new()
            .name("journal-maintenance".into())
            .spawn({
                let journal = Arc::clone(&journal);
                move || run_maintenance_until_stopped(&journal, &maintenance_inbox)
            })
            .context("start the maintenance of the journal")?;
        let threads = JournalThreads {
            journal: Arc::clone(&journal),
            sync_threads,
            maintenance_thread,
            maintenance_tasks,
        };
        Ok(OpenedJournal {
            journal,
            threads,
            synced_ends,
        })
    }

    pub(crate) const fn signal_log(&self, signal: Signal) -> &SignalLog {
        match signal {
            Signal::Logs => &self.logs,
            Signal::Spans => &self.spans,
            Signal::Metrics => &self.metrics,
        }
    }

    pub(crate) fn delete_segments_past_retention(&self) {
        let retained_since = self.config.retained_since(now_unix_nanos());
        for signal in Signal::ALL {
            let signal_log = self.signal_log(signal);
            let deleted = signal_log
                .segments()
                .list_segments()
                .and_then(|listed_segments| {
                    delete_segments_past_retention(
                        signal_log.segments(),
                        &listed_segments,
                        signal_log.open_hour(),
                        retained_since,
                    )
                });
            if let Err(error) = deleted {
                tracing::warn!("delete the journal of {signal:?} past its retention: {error:#}");
            }
        }
    }
}

impl Journal for JournalFiles {
    fn append_frame(
        &self,
        signal: Signal,
        received_at: i64,
        request: &[u8],
    ) -> anyhow::Result<SyncTicket> {
        self.signal_log(signal).append_frame(received_at, request)
    }

    fn read_frames(&self, signal: Signal, from: Option<Position>) -> anyhow::Result<Frames> {
        self.signal_log(signal).read_frames(from)
    }

    fn size_in_bytes(&self) -> anyhow::Result<u64> {
        Signal::ALL
            .into_iter()
            .map(|signal| self.signal_log(signal).segments().size_in_bytes())
            .sum()
    }
}

pub struct JournalThreads {
    journal: Arc<JournalFiles>,
    sync_threads: Vec<JoinHandle<()>>,
    maintenance_thread: JoinHandle<()>,
    maintenance_tasks: Sender<MaintenanceTask>,
}

impl JournalThreads {
    pub fn stop_and_join(self) -> anyhow::Result<()> {
        for signal in Signal::ALL {
            self.journal.signal_log(signal).stop_syncing();
        }
        for thread in self.sync_threads {
            thread
                .join()
                .map_err(|_| anyhow!("a sync thread of the journal panicked"))?;
        }
        let _ = self.maintenance_tasks.send(MaintenanceTask::Stop);
        self.maintenance_thread
            .join()
            .map_err(|_| anyhow!("the maintenance thread of the journal panicked"))
    }
}

fn now_unix_nanos() -> i64 {
    i64::try_from(jiff::Timestamp::now().as_nanosecond()).expect("now fits an i64 until 2262")
}
