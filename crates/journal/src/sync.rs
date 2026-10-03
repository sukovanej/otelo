use std::sync::Arc;

use anyhow::anyhow;
use tokio::sync::watch;

#[derive(Clone, Debug)]
enum SyncState {
    Syncing {
        synced_frames: u64,
    },
    Failed {
        synced_frames: u64,
        error_message: Arc<str>,
    },
}

#[must_use]
pub fn open_sync_channel() -> (SyncPublisher, SyncSubscription) {
    let (sender, receiver) = watch::channel(SyncState::Syncing { synced_frames: 0 });
    (
        SyncPublisher { state: sender },
        SyncSubscription { state: receiver },
    )
}

pub struct SyncPublisher {
    state: watch::Sender<SyncState>,
}

impl SyncPublisher {
    pub fn publish_synced_frames(&self, synced_frames: u64) {
        self.state.send_modify(|state| {
            if let SyncState::Syncing {
                synced_frames: published,
            } = state
            {
                *published = (*published).max(synced_frames);
            }
        });
    }

    pub fn publish_failure(&self, error_message: &str) {
        self.state.send_modify(|state| {
            if let SyncState::Syncing { synced_frames } = *state {
                *state = SyncState::Failed {
                    synced_frames,
                    error_message: error_message.into(),
                };
            }
        });
    }
}

#[derive(Clone)]
pub struct SyncSubscription {
    state: watch::Receiver<SyncState>,
}

impl SyncSubscription {
    pub fn ticket_for_frame(&self, frame_number: u64) -> SyncTicket {
        SyncTicket {
            state: self.state.clone(),
            frame_number,
        }
    }
}

#[must_use = "the frame is durable only once the ticket says so"]
pub struct SyncTicket {
    state: watch::Receiver<SyncState>,
    frame_number: u64,
}

impl SyncTicket {
    pub async fn wait_until_synced(mut self) -> anyhow::Result<()> {
        let frame_number = self.frame_number;
        let state = self
            .state
            .wait_for(|state| match state {
                SyncState::Syncing { synced_frames } => *synced_frames >= frame_number,
                SyncState::Failed { .. } => true,
            })
            .await
            .map_err(|_| anyhow!("the journal stopped before it synced the frame"))?;
        match &*state {
            SyncState::Syncing { .. } => Ok(()),
            SyncState::Failed { synced_frames, .. } if *synced_frames >= frame_number => Ok(()),
            SyncState::Failed { error_message, .. } => Err(anyhow!(
                "the journal failed to sync the frame: {error_message}"
            )),
        }
    }
}
