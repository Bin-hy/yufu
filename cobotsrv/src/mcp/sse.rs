use std::convert::Infallible;
use std::time::Duration;

use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream;

/// SSE endpoint for MCP server-to-client notifications.
///
/// Phase 1: keeps the connection alive with periodic comments (30s interval).
/// No business events are pushed yet.
///
/// Phase 2 will add `notifications/tools/list_changed` when devices come
/// online/offline via MQTT. Requires adding a broadcast channel to AppState.
pub async fn sse_handler() -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let s = stream::pending::<Result<Event, Infallible>>();
    Sse::new(s).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keep-alive"),
    )
}
