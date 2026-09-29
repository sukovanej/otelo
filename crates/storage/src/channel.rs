use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

use crate::Batch;

#[must_use]
pub fn batch_channel(capacity: usize) -> (Sender, Inbox) {
    let (tx, rx) = sync_channel(capacity);
    let dropped = Arc::new(AtomicU64::new(0));
    (
        Sender {
            tx,
            dropped: Arc::clone(&dropped),
        },
        Inbox { rx, dropped },
    )
}

#[derive(Clone)]
pub struct Sender {
    tx: SyncSender<Batch>,
    dropped: Arc<AtomicU64>,
}

impl Sender {
    #[must_use = "a dropped batch is lost, and its source can tell its sender"]
    pub fn send(&self, batch: Batch) -> bool {
        // A burst drops batches and never waits or grows memory.
        match self.tx.try_send(batch) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    #[must_use]
    pub fn dropped_batches(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

pub struct Inbox {
    rx: Receiver<Batch>,
    dropped: Arc<AtomicU64>,
}

impl Inbox {
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Batch, RecvTimeoutError> {
        self.rx.recv_timeout(timeout)
    }

    pub fn try_iter(&self) -> impl Iterator<Item = Batch> + '_ {
        self.rx.try_iter()
    }

    #[must_use]
    pub fn dropped_batches(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}
