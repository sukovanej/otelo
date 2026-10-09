use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::Stream;
use futures_util::stream;
use otelo_indexed_storage::Indexing;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::Api;

// The indexer reports after each transaction, many times a second while it catches up.
const MIN_INDEXING_EVENT_INTERVAL: Duration = Duration::from_secs(1);

/// The events of the daemon, as server-sent events. The first `indexing`
/// event comes at once and holds how far the index has read the journal, and
/// another comes after each change, a second apart at least.
#[utoipa::path(
    get,
    path = "/api/events",
    responses(
        (status = 200, content_type = "text/event-stream", body = Indexing,
            description = "An `indexing` event of the JSON of `Indexing`"),
    ),
)]
pub async fn stream_events(
    State(api): State<Api>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut indexing = api.storage.watch_indexing();
    indexing.mark_changed();
    let indexing_events = IndexingEvents {
        indexing,
        shutdown: api.shutdown,
        next_event_at: Instant::now(),
    };
    Sse::new(stream::unfold(indexing_events, send_next_indexing_event))
        .keep_alive(KeepAlive::default())
}

struct IndexingEvents {
    indexing: watch::Receiver<Indexing>,
    shutdown: CancellationToken,
    next_event_at: Instant,
}

// The server waits for its connections to end before it stops, so the stream ends at the shutdown.
async fn send_next_indexing_event(
    mut events: IndexingEvents,
) -> Option<(Result<Event, Infallible>, IndexingEvents)> {
    tokio::select! {
        () = events.shutdown.cancelled() => return None,
        changed = events.indexing.changed() => changed.ok()?,
    }
    tokio::select! {
        () = events.shutdown.cancelled() => return None,
        () = tokio::time::sleep_until(events.next_event_at) => {}
    }
    events.next_event_at = Instant::now() + MIN_INDEXING_EVENT_INTERVAL;
    let indexing = *events.indexing.borrow_and_update();
    let event = Event::default()
        .event("indexing")
        .json_data(indexing)
        .expect("an indexing serializes to JSON");
    Some((Ok(event), events))
}
