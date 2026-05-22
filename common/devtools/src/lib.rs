//! Chrome DevTools Protocol (CDP) server for inspecting a leptos-native
//! layout tree.
//!
//! The renderer keeps every node in a single thread-local Taffy slotmap
//! (`renderer::scene::LayoutState<B>`) holding each node's style, computed
//! layout, parent and children. This crate maps that tree onto the CDP
//! DOM/CSS domains and serves it over a WebSocket so the Chrome DevTools
//! frontend can inspect the view tree, view and live-edit Taffy styles,
//! and read the computed box model (location/size/padding/border/margin).
//!
//! It is **transport-agnostic and port-agnostic**: it serves CDP over any
//! `futures` [`AsyncRead`](futures::AsyncRead)/[`AsyncWrite`] stream and
//! is generic over the port's [`LayoutBackend`]. The port owns the
//! listener and the native-loop socket integration, and drives
//! [`serve_connection`] on its main-loop executor — so the CDP dispatcher
//! reads and mutates the thread-local tree directly, with no locking or
//! cross-thread bridge.
//!
//! Wiring (per port): for each accepted connection, spawn
//! `serve_connection::<S, B>(stream, host, schedule)` on the main-loop
//! executor, where `schedule` triggers the port's relayout for a node
//! (so live style edits reflow).

mod events;
mod idmap;
mod mapping;
mod server;
mod session;

use leptos_native::renderer::NodeId;
use serde_json::json;
use std::rc::Rc;

pub use server::serve_connection;

// Re-exported for ports that want to test the mapping directly.
pub use mapping::{apply_css_text, box_model_json, css_decls};

/// Port-supplied callbacks the CDP server needs. All run on the main
/// thread (the server future is driven by the port's main-loop executor),
/// so they touch the thread-local layout tree and platform views directly.
#[derive(Clone)]
pub struct Hooks {
    /// Reflow a node after a live style edit (the port's relayout trigger).
    pub schedule_relayout: Rc<dyn Fn(NodeId)>,
    /// Highlight a node in the running UI (`None` clears). Drives the
    /// Elements-panel hover overlay.
    pub set_highlight: Rc<dyn Fn(Option<NodeId>)>,
    /// Displayable attributes for a node (e.g. a button's `title`), shown
    /// in the Elements tree. Reads the platform view, so it's port-specific.
    pub node_attributes: Rc<dyn Fn(NodeId) -> Vec<(String, String)>>,
    /// Enter/leave "inspect from app" mode: while on, the port watches its
    /// pointer and calls [`notify_node_hovered`] / [`notify_node_picked`].
    pub set_inspect_mode: Rc<dyn Fn(bool)>,
}

/// Inspect-mode: the user is hovering `node` in the running UI. Tells the
/// frontend to reveal/outline it in the Elements tree.
pub fn notify_node_hovered(node: NodeId) {
    let id = idmap::cdp_id(node);
    events::broadcast(
        json!({ "method": "Overlay.nodeHighlightRequested", "params": { "nodeId": id } })
            .to_string(),
    );
}

/// Inspect-mode: the user clicked `node` in the running UI. Tells the
/// frontend to select it (and the frontend then leaves inspect mode).
pub fn notify_node_picked(node: NodeId) {
    let id = idmap::cdp_id(node);
    events::broadcast(
        json!({ "method": "Overlay.inspectNodeRequested", "params": { "backendNodeId": id } })
            .to_string(),
    );
}
