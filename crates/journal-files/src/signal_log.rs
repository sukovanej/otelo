use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use otelo_journal::{
    Frames, Hour, Position, SyncPublisher, SyncSubscription, SyncTicket, SyncedEnd,
    SyncedEndSender, open_sync_channel,
};
use otelo_query::Signal;

use crate::frame::encode_frame;
use crate::maintenance::MaintenanceTask;
use crate::reader::{FrameReader, ReadableEnd};
use crate::segment::SegmentDirectory;

const MIN_INTERVAL_BETWEEN_SYNCS: Duration = Duration::from_millis(200);

pub struct OpenSegment {
    pub hour: Hour,
    pub file: Arc<File>,
    pub length: u64,
}

pub struct SignalLog {
    signal: Signal,
    segments: SegmentDirectory,
    appending: Mutex<Appending>,
    frame_appended: Condvar,
    readable_end: Mutex<ReadableEnd>,
    sync_publisher: SyncPublisher,
    sync_subscription: SyncSubscription,
    maintenance_tasks: Sender<MaintenanceTask>,
}

struct Appending {
    open_segment: Option<OpenSegment>,
    newest_closed_hour: Option<Hour>,
    appended_frames: u64,
    newest_received_at: i64,
    sync_failure: Option<String>,
    stopping: bool,
}

impl Appending {
    fn hour_of_next_frame(&self, received_at: i64) -> Hour {
        let open_hour = self
            .open_segment
            .as_ref()
            .map(|open_segment| open_segment.hour);
        [open_hour, self.newest_closed_hour.map(Hour::next)]
            .into_iter()
            .flatten()
            .fold(Hour::containing(received_at), Hour::max)
    }
}

impl SignalLog {
    pub fn new(
        signal: Signal,
        segments: SegmentDirectory,
        open_segment: Option<OpenSegment>,
        newest_closed_hour: Option<Hour>,
        maintenance_tasks: Sender<MaintenanceTask>,
    ) -> Self {
        let readable_end = open_segment
            .as_ref()
            .map_or(ReadableEnd::EveryHour, |open| {
                ReadableEnd::OpenSegment(Position {
                    segment_hour: open.hour,
                    byte_offset: open.length,
                })
            });
        let (sync_publisher, sync_subscription) = open_sync_channel();
        Self {
            signal,
            segments,
            appending: Mutex::new(Appending {
                open_segment,
                newest_closed_hour,
                appended_frames: 0,
                newest_received_at: 0,
                sync_failure: None,
                stopping: false,
            }),
            frame_appended: Condvar::new(),
            readable_end: Mutex::new(readable_end),
            sync_publisher,
            sync_subscription,
            maintenance_tasks,
        }
    }

    pub const fn segments(&self) -> &SegmentDirectory {
        &self.segments
    }

    pub fn open_hour(&self) -> Option<Hour> {
        lock(&self.appending)
            .open_segment
            .as_ref()
            .map(|open_segment| open_segment.hour)
    }

    pub fn append_frame(&self, received_at: i64, request: &[u8]) -> anyhow::Result<SyncTicket> {
        let frame = encode_frame(received_at, request)?;
        let mut appending = lock(&self.appending);
        if let Some(sync_failure) = &appending.sync_failure {
            return Err(anyhow!(
                "the {} journal stopped after a failed sync: {sync_failure}",
                self.segments.path().display()
            ));
        }
        let hour = appending.hour_of_next_frame(received_at);
        if appending
            .open_segment
            .as_ref()
            .is_none_or(|open_segment| open_segment.hour < hour)
        {
            self.close_open_segment(&mut appending)?;
            appending.open_segment = Some(self.create_segment(hour)?);
            self.publish_readable_end(Position {
                segment_hour: hour,
                byte_offset: 0,
            });
        }
        let open_segment = appending
            .open_segment
            .as_mut()
            .expect("an open segment was made above");
        if let Err(write_error) = (&*open_segment.file).write_all(&frame) {
            let path = self.segments.uncompressed_segment_path(open_segment.hour);
            if let Err(truncate_error) = open_segment.file.set_len(open_segment.length) {
                let sync_failure = format!(
                    "cut a partly written frame off {}: {truncate_error}",
                    path.display()
                );
                self.sync_publisher.publish_failure(&sync_failure);
                appending.sync_failure = Some(sync_failure);
            }
            return Err(write_error).with_context(|| format!("append to {}", path.display()));
        }
        open_segment.length += frame.len() as u64;
        appending.appended_frames += 1;
        appending.newest_received_at = received_at;
        let frame_number = appending.appended_frames;
        drop(appending);
        self.frame_appended.notify_one();
        Ok(self.sync_subscription.ticket_for_frame(frame_number))
    }

    pub fn read_frames(&self, from: Option<Position>) -> anyhow::Result<Frames> {
        // Listed first, so a segment made after the listing cannot be read past what is synced.
        let segments = self.segments.list_segments()?;
        let readable_end = *lock(&self.readable_end);
        Ok(Box::new(FrameReader::new(
            &self.segments,
            &segments,
            from,
            readable_end,
        )))
    }

    pub fn sync_frames_until_stopped(&self, synced_end_sender: &SyncedEndSender) {
        let mut synced_frames = 0;
        loop {
            let started_at = Instant::now();
            let (file, appended_frames, synced_end) = {
                let mut appending = lock(&self.appending);
                while appending.appended_frames == synced_frames
                    && !appending.stopping
                    && appending.sync_failure.is_none()
                {
                    appending = self
                        .frame_appended
                        .wait(appending)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                if appending.appended_frames == synced_frames || appending.sync_failure.is_some() {
                    return;
                }
                let open_segment = appending
                    .open_segment
                    .as_ref()
                    .expect("a frame was appended to an open segment");
                (
                    Arc::clone(&open_segment.file),
                    appending.appended_frames,
                    SyncedEnd {
                        signal: self.signal,
                        end: Position {
                            segment_hour: open_segment.hour,
                            byte_offset: open_segment.length,
                        },
                        newest_received_at: appending.newest_received_at,
                    },
                )
            };
            match file.sync_data() {
                Ok(()) => {
                    synced_frames = appended_frames;
                    self.publish_readable_end(synced_end.end);
                    self.sync_publisher.publish_synced_frames(synced_frames);
                    synced_end_sender.send_synced_end(synced_end);
                }
                Err(error) => {
                    let sync_failure = format!(
                        "sync {}: {error}",
                        self.segments
                            .uncompressed_segment_path(synced_end.end.segment_hour)
                            .display()
                    );
                    tracing::error!(signal = self.signal.name(), "{sync_failure}");
                    self.sync_publisher.publish_failure(&sync_failure);
                    lock(&self.appending).sync_failure = Some(sync_failure);
                    return;
                }
            }
            thread::sleep(MIN_INTERVAL_BETWEEN_SYNCS.saturating_sub(started_at.elapsed()));
        }
    }

    pub fn stop_syncing(&self) {
        lock(&self.appending).stopping = true;
        self.frame_appended.notify_one();
    }

    fn close_open_segment(&self, appending: &mut Appending) -> anyhow::Result<()> {
        let Some(open_segment) = appending.open_segment.take() else {
            return Ok(());
        };
        let path = self.segments.uncompressed_segment_path(open_segment.hour);
        if let Err(error) = open_segment.file.sync_data() {
            let sync_failure = format!("sync {}: {error}", path.display());
            self.sync_publisher.publish_failure(&sync_failure);
            appending.sync_failure = Some(sync_failure.clone());
            appending.open_segment = Some(open_segment);
            return Err(anyhow!(sync_failure));
        }
        appending.newest_closed_hour = Some(open_segment.hour);
        // After the maintenance stopped, the next startup compresses the segment.
        let _ = self
            .maintenance_tasks
            .send(MaintenanceTask::CompressSegment {
                signal: self.signal,
                hour: open_segment.hour,
            });
        Ok(())
    }

    fn create_segment(&self, hour: Hour) -> anyhow::Result<OpenSegment> {
        let path = self.segments.uncompressed_segment_path(hour);
        let file = OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("create {}", path.display()))?;
        self.segments.sync()?;
        Ok(OpenSegment {
            hour,
            file: Arc::new(file),
            length: 0,
        })
    }

    fn publish_readable_end(&self, end: Position) {
        let mut readable_end = lock(&self.readable_end);
        if let ReadableEnd::OpenSegment(published) = *readable_end
            && published >= end
        {
            return;
        }
        *readable_end = ReadableEnd::OpenSegment(end);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
