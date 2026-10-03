mod hour;
mod sync;

use otelo_query::Signal;

pub use hour::Hour;
pub use sync::{SyncPublisher, SyncSubscription, SyncTicket, open_sync_channel};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub hour: Hour,
    pub offset: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub received_at: i64,
    pub request: Vec<u8>,
    pub position_after: Position,
}

pub type Frames = Box<dyn Iterator<Item = anyhow::Result<Frame>> + Send>;

pub trait Journal: Send + Sync {
    fn append_frame(
        &self,
        signal: Signal,
        received_at: i64,
        request: &[u8],
    ) -> anyhow::Result<SyncTicket>;

    fn read_frames(&self, signal: Signal, from: Option<Position>) -> anyhow::Result<Frames>;

    fn size_in_bytes(&self) -> anyhow::Result<u64>;
}
