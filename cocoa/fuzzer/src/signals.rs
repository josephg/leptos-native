//! Per-type signal store, keyed by [`crate::spec::SignalId`]. The
//! generator records reactive attributes into the store; the
//! renderer reads them out to wire `move || sig.get()` closures into
//! cocoa builders; the chaos loop mutates them.
//!
//! Wrapped in `Rc<RefCell<…>>` so build closures inside reactive
//! `Show`-style branches can ensure-and-fetch signals on demand
//! without an exclusive borrow.

use crate::spec::SignalId;
use reactive_graph::owner::Owner;
use reactive_graph::signal::RwSignal;
use reactive_graph::traits::GetUntracked;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Inner {
    strings: HashMap<SignalId, RwSignal<String>>,
    bools: HashMap<SignalId, RwSignal<bool>>,
    /// `f32`-typed signals — used for layout attrs (padding/gap/
    /// flex_grow/etc.) which take `f32` throughout.
    floats: HashMap<SignalId, RwSignal<f32>>,
    /// `f64`-typed signals — used by slider/stepper/progress
    /// values, which take `f64`.
    floats64: HashMap<SignalId, RwSignal<f64>>,
    /// `usize` signals — popup / segmented selection index.
    indices: HashMap<SignalId, RwSignal<usize>>,
}

#[derive(Clone)]
pub struct SignalStore {
    inner: Arc<Mutex<Inner>>,
    /// The Owner that newly-created signals are scoped to. Captured
    /// at `SignalStore::new()` time so that signals lazily created
    /// from inside `Show`-style reactive closures don't end up
    /// scoped to the Show branch's child Owner — which would
    /// dispose them as soon as the branch flipped off, leaving
    /// the snapshot `get_untracked()` panicking on a dead signal.
    owner: Owner,
}

impl SignalStore {
    /// Build a `SignalStore` scoped to the current Owner. Call
    /// from inside `owner.with(...)` of the top-level test owner —
    /// every signal `ensure_*` creates from then on lives at this
    /// level, not in any nested reactive owner.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
            owner: Owner::current()
                .expect("SignalStore::new must run inside an Owner"),
        }
    }

    /// Lazily create a string signal with the given initial value.
    /// Subsequent lookups by the same id return the existing signal
    /// (initial is ignored on repeat).
    pub fn ensure_string(&self, id: SignalId, initial: &str) -> RwSignal<String> {
        let owner = self.owner.clone();
        let initial = initial.to_owned();
        *self
            .inner
            .lock().unwrap()
            .strings
            .entry(id)
            .or_insert_with(|| owner.with(|| RwSignal::new(initial)))
    }

    pub fn ensure_bool(&self, id: SignalId, initial: bool) -> RwSignal<bool> {
        let owner = self.owner.clone();
        *self
            .inner
            .lock().unwrap()
            .bools
            .entry(id)
            .or_insert_with(|| owner.with(|| RwSignal::new(initial)))
    }

    pub fn ensure_float(&self, id: SignalId, initial: f32) -> RwSignal<f32> {
        let owner = self.owner.clone();
        *self
            .inner
            .lock().unwrap()
            .floats
            .entry(id)
            .or_insert_with(|| owner.with(|| RwSignal::new(initial)))
    }

    pub fn ensure_float64(&self, id: SignalId, initial: f64) -> RwSignal<f64> {
        let owner = self.owner.clone();
        *self
            .inner
            .lock().unwrap()
            .floats64
            .entry(id)
            .or_insert_with(|| owner.with(|| RwSignal::new(initial)))
    }

    pub fn ensure_index(&self, id: SignalId, initial: usize) -> RwSignal<usize> {
        let owner = self.owner.clone();
        *self
            .inner
            .lock().unwrap()
            .indices
            .entry(id)
            .or_insert_with(|| owner.with(|| RwSignal::new(initial)))
    }

    pub fn total_count(&self) -> usize {
        let i = self.inner.lock().unwrap();
        i.strings.len() + i.bools.len() + i.floats.len()
            + i.floats64.len() + i.indices.len()
    }

    /// Iterate signal ids by type — used by the chaos loop to
    /// pick a victim.
    pub fn string_ids(&self) -> Vec<SignalId> {
        self.inner.lock().unwrap().strings.keys().copied().collect()
    }
    pub fn bool_ids(&self) -> Vec<SignalId> {
        self.inner.lock().unwrap().bools.keys().copied().collect()
    }
    pub fn float_ids(&self) -> Vec<SignalId> {
        self.inner.lock().unwrap().floats.keys().copied().collect()
    }
    pub fn float64_ids(&self) -> Vec<SignalId> {
        self.inner.lock().unwrap().floats64.keys().copied().collect()
    }
    pub fn index_ids(&self) -> Vec<SignalId> {
        self.inner.lock().unwrap().indices.keys().copied().collect()
    }

    pub fn get_string(&self, id: SignalId) -> Option<RwSignal<String>> {
        self.inner.lock().unwrap().strings.get(&id).copied()
    }
    pub fn get_bool(&self, id: SignalId) -> Option<RwSignal<bool>> {
        self.inner.lock().unwrap().bools.get(&id).copied()
    }
    pub fn get_float(&self, id: SignalId) -> Option<RwSignal<f32>> {
        self.inner.lock().unwrap().floats.get(&id).copied()
    }
    pub fn get_float64(&self, id: SignalId) -> Option<RwSignal<f64>> {
        self.inner.lock().unwrap().floats64.get(&id).copied()
    }
    pub fn get_index(&self, id: SignalId) -> Option<RwSignal<usize>> {
        self.inner.lock().unwrap().indices.get(&id).copied()
    }

    /// Snapshot current values for every registered signal. Used
    /// after the chaos loop to build the static comparison tree.
    pub fn snapshot(&self) -> SignalSnapshot {
        let inner = self.inner.lock().unwrap();
        SignalSnapshot {
            strings: inner.strings.iter().map(|(id, s)| (*id, s.get_untracked())).collect(),
            bools:   inner.bools  .iter().map(|(id, s)| (*id, s.get_untracked())).collect(),
            floats:  inner.floats .iter().map(|(id, s)| (*id, s.get_untracked())).collect(),
            floats64:inner.floats64.iter().map(|(id, s)| (*id, s.get_untracked())).collect(),
            indices: inner.indices.iter().map(|(id, s)| (*id, s.get_untracked())).collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SignalSnapshot {
    pub strings: HashMap<SignalId, String>,
    pub bools: HashMap<SignalId, bool>,
    pub floats: HashMap<SignalId, f32>,
    pub floats64: HashMap<SignalId, f64>,
    pub indices: HashMap<SignalId, usize>,
}
