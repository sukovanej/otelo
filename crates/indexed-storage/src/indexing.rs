use otelo_query::Signal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// How far the index has read the journal of each signal. A query answers
/// from the index, so it misses what the index has not read yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Indexing {
    pub logs: SignalIndexing,
    pub spans: SignalIndexing,
    pub metrics: SignalIndexing,
}

impl Indexing {
    pub const STARTING: Self = Self {
        logs: SignalIndexing::Starting,
        spans: SignalIndexing::Starting,
        metrics: SignalIndexing::Starting,
    };

    pub fn replace_signal(&mut self, signal: Signal, indexing: SignalIndexing) -> bool {
        let replaced = match signal {
            Signal::Logs => &mut self.logs,
            Signal::Spans => &mut self.spans,
            Signal::Metrics => &mut self.metrics,
        };
        std::mem::replace(replaced, indexing) != indexing
    }
}

/// How far the index has read the journal of one signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SignalIndexing {
    /// The indexer has not read the journal yet since the daemon started.
    Starting,
    /// The index is behind the journal, after a start, a rebuild, or a burst.
    CatchingUp {
        /// How much of what the journal received since the catch-up began
        /// the index holds, from 0 to 99.
        indexed_percent: u8,
    },
    /// The index holds what the journal received up to the last second or so.
    CaughtUp,
}
