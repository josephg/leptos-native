//! Bijection between CDP integer node ids and taffy [`NodeId`]s.
//!
//! DevTools assumes small monotonic integer node ids. The renderer's
//! `NodeId` is a generational slotmap key (a `u64` newtype) that we must
//! not leak directly — once a node is freed and its slot reused, the
//! generation differs, so a stale CDP id maps to `None` rather than
//! aliasing a new node.
//!
//! Two ids are reserved for synthetic nodes the frontend expects but the
//! tree doesn't have: the `#document` node and a single root container
//! element that parents every real subtree root.

use renderer::NodeId;
use std::collections::HashMap;

/// The synthetic `#document` node (nodeType 9).
pub const DOCUMENT_ID: i64 = 1;
/// The synthetic root container element that wraps every real root.
pub const ROOT_ID: i64 = 2;

/// First id handed out to a real taffy node. Leaves room below for the
/// reserved synthetic ids.
const FIRST_REAL_ID: i64 = 1000;

#[derive(Default)]
pub struct IdMap {
    next: i64,
    to_taffy: HashMap<i64, NodeId>,
    to_cdp: HashMap<NodeId, i64>,
}

impl IdMap {
    pub fn new() -> Self {
        IdMap {
            next: FIRST_REAL_ID,
            to_taffy: HashMap::new(),
            to_cdp: HashMap::new(),
        }
    }

    /// CDP id for a taffy node, allocating a fresh one on first sight.
    pub fn cdp_id(&mut self, node: NodeId) -> i64 {
        if let Some(id) = self.to_cdp.get(&node) {
            return *id;
        }
        let id = self.next;
        self.next += 1;
        self.to_cdp.insert(node, id);
        self.to_taffy.insert(id, node);
        id
    }

    /// Taffy node for a previously-issued CDP id (`None` for synthetic or
    /// unknown ids).
    pub fn taffy(&self, cdp: i64) -> Option<NodeId> {
        self.to_taffy.get(&cdp).copied()
    }
}
