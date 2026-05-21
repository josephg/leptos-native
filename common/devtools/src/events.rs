//! Outgoing-event fan-out: lets the *backend* push unsolicited CDP events
//! (not just command responses) to connected frontends.
//!
//! Each live session registers an unbounded sender; its run loop selects
//! on the matching receiver and writes anything it gets as a WS frame.
//! Used by inspect-from-app — GTK pointer events translate to
//! `Overlay.nodeHighlightRequested` / `Overlay.inspectNodeRequested`
//! events that must reach the frontend out of band.

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use std::cell::RefCell;

thread_local! {
    static SINKS: RefCell<Vec<UnboundedSender<String>>> = const { RefCell::new(Vec::new()) };
}

/// Register a session's outgoing channel; the returned receiver yields
/// messages to write to that session's WebSocket.
pub fn register() -> UnboundedReceiver<String> {
    let (tx, rx) = unbounded();
    SINKS.with(|s| s.borrow_mut().push(tx));
    rx
}

/// Send a pre-serialized JSON message to every connected session,
/// dropping any whose receiver has closed.
pub fn broadcast(msg: String) {
    SINKS.with(|s| {
        s.borrow_mut()
            .retain(|tx| tx.unbounded_send(msg.clone()).is_ok())
    });
}
