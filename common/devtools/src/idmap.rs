//! Global (per-thread) bijection between CDP integer node ids and taffy
//! [`NodeId`]s, plus the outgoing-event broadcast registry.
//!
//! DevTools assumes small monotonic integer node ids. The renderer's
//! `NodeId` is a generational slotmap key (a `u64` newtype) that we must
//! not leak directly — once a node is freed and its slot reused, the
//! generation differs, so a stale CDP id maps to `None` rather than
//! aliasing a new node.
//!
//! The map is **global to the thread**, not per-connection, so the
//! inspect-from-app path (GTK pointer → [`crate::notify_node_picked`])
//! can translate a taffy id into the same CDP id the frontend already
//! holds from `DOM.getDocument`. The devtools server is single-threaded
//! and effectively single-connection, so this is sound and simpler than
//! threading a per-session map everywhere.

use renderer::NodeId;
use std::cell::RefCell;
use std::collections::HashMap;

/// The synthetic `#document` node (nodeType 9).
pub const DOCUMENT_ID: i64 = 1;
/// The synthetic root container element that wraps every real root.
pub const ROOT_ID: i64 = 2;

/// First id handed out to a real taffy node. Leaves room below for the
/// reserved synthetic ids.
const FIRST_REAL_ID: i64 = 1000;

struct IdMap {
    next: i64,
    to_taffy: HashMap<i64, NodeId>,
    to_cdp: HashMap<NodeId, i64>,
}

thread_local! {
    static MAP: RefCell<IdMap> = RefCell::new(IdMap {
        next: FIRST_REAL_ID,
        to_taffy: HashMap::new(),
        to_cdp: HashMap::new(),
    });
}

/// CDP id for a taffy node, allocating a fresh one on first sight.
pub fn cdp_id(node: NodeId) -> i64 {
    MAP.with(|m| {
        let m = &mut *m.borrow_mut();
        if let Some(id) = m.to_cdp.get(&node) {
            return *id;
        }
        let id = m.next;
        m.next += 1;
        m.to_cdp.insert(node, id);
        m.to_taffy.insert(id, node);
        id
    })
}

/// Taffy node for a previously-issued CDP id (`None` for synthetic or
/// unknown ids).
pub fn taffy(cdp: i64) -> Option<NodeId> {
    MAP.with(|m| m.borrow().to_taffy.get(&cdp).copied())
}
