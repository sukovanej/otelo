use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::time::Duration;

use otelo_query::Signal;

use crate::Position;

// A message only wakes the indexer, which then reads every synced frame, so a full queue that
// drops one loses no frame. The capacity is a minute of syncs of every signal.
const SYNCED_END_QUEUE_CAPACITY: usize = 1_024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncedEnd {
    pub signal: Signal,
    pub end: Position,
    pub newest_received_at: i64,
}

#[must_use]
pub fn open_synced_end_queue() -> (SyncedEndSender, SyncedEndInbox) {
    let (queue_sender, queue_receiver) = sync_channel(SYNCED_END_QUEUE_CAPACITY);
    (
        SyncedEndSender {
            queue: queue_sender,
        },
        SyncedEndInbox {
            queue: queue_receiver,
        },
    )
}

#[derive(Clone)]
pub struct SyncedEndSender {
    queue: SyncSender<SyncedEnd>,
}

impl SyncedEndSender {
    pub fn send_synced_end(&self, synced_end: SyncedEnd) {
        let _ = self.queue.try_send(synced_end);
    }
}

pub struct SyncedEndInbox {
    queue: Receiver<SyncedEnd>,
}

impl SyncedEndInbox {
    pub fn wait_for_synced_end(&self, timeout: Duration) -> Result<SyncedEnd, RecvTimeoutError> {
        self.queue.recv_timeout(timeout)
    }

    pub fn take_queued_synced_ends(&self) -> impl Iterator<Item = SyncedEnd> + '_ {
        self.queue.try_iter()
    }
}
