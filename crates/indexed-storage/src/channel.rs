use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

use crate::Batch;

#[must_use]
pub fn open_batch_channel(capacity: usize) -> (BatchSender, BatchInbox) {
    let (queue_sender, queue_receiver) = sync_channel(capacity);
    let dropped_batches = Arc::new(AtomicU64::new(0));
    (
        BatchSender {
            queue: queue_sender,
            dropped_batches: Arc::clone(&dropped_batches),
        },
        BatchInbox {
            queue: queue_receiver,
            dropped_batches,
        },
    )
}

#[derive(Clone)]
pub struct BatchSender {
    queue: SyncSender<Batch>,
    dropped_batches: Arc<AtomicU64>,
}

impl BatchSender {
    #[must_use = "a dropped batch is lost, and its source can tell its sender"]
    pub fn send_batch(&self, batch: Batch) -> bool {
        // A burst drops batches and never waits or grows memory.
        match self.queue.try_send(batch) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped_batches.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    #[must_use]
    pub fn dropped_batches(&self) -> u64 {
        self.dropped_batches.load(Ordering::Relaxed)
    }
}

pub struct BatchInbox {
    queue: Receiver<Batch>,
    dropped_batches: Arc<AtomicU64>,
}

impl BatchInbox {
    pub fn wait_for_batch(&self, timeout: Duration) -> Result<Batch, RecvTimeoutError> {
        self.queue.recv_timeout(timeout)
    }

    pub fn take_queued_batches(&self) -> impl Iterator<Item = Batch> + '_ {
        self.queue.try_iter()
    }

    #[must_use]
    pub fn dropped_batches(&self) -> u64 {
        self.dropped_batches.load(Ordering::Relaxed)
    }
}
