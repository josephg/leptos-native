//! Chaos loop: repeatedly mutate a random signal in the
//! [`SignalStore`] to a fresh random value. Deterministic for a
//! given seed.

use crate::signals::SignalStore;
use rand::prelude::*;
use rand::rngs::ChaCha8Rng;
use reactive_graph::traits::Set;

pub struct Chaos<'a> {
    pub rng: &'a mut ChaCha8Rng,
    pub iterations: usize,
}

impl<'a> Chaos<'a> {
    /// Runs `iterations` mutations against `store`. Each iteration
    /// picks one of the three signal type-maps weighted by size
    /// and mutates one random entry. No-op if the store has no
    /// signals.
    pub fn run(&mut self, store: &SignalStore) {
        self.run_with_callback(store, |_| {});
    }

    /// Same as [`run`] but invokes `between` after each mutation
    /// with the iteration index. Useful for per-iteration leak
    /// checks (the caller pumps the run loop and snapshots store
    /// sizes).
    pub fn run_with_callback(
        &mut self,
        store: &SignalStore,
        mut between: impl FnMut(usize),
    ) {
        // Snapshot the id lists up front. Mutating a signal can
        // toggle a `Show` branch into/out of existence, which
        // adds new signals to the store mid-loop — but we want
        // chaos to operate on the population that existed at
        // mount time. Skipping a victim id that has since been
        // removed (none, in practice — signals aren't dropped)
        // is fine.
        let strings = store.string_ids();
        let bools = store.bool_ids();
        let floats = store.float_ids();
        let total = strings.len() + bools.len() + floats.len();
        if total == 0 {
            return;
        }
        for iter in 0..self.iterations {
            let pick = self.rng.random_range(0..total);
            if pick < strings.len() {
                let id = *strings.choose(self.rng).unwrap();
                if let Some(sig) = store.get_string(id) {
                    sig.set(self.gen_string());
                }
            } else if pick < strings.len() + bools.len() {
                let id = *bools.choose(self.rng).unwrap();
                if let Some(sig) = store.get_bool(id) {
                    sig.set(self.rng.random_bool(0.5));
                }
            } else {
                let id = *floats.choose(self.rng).unwrap();
                if let Some(sig) = store.get_float(id) {
                    let v = *[0.0_f32, 4.0, 8.0, 12.0, 16.0]
                        .choose(self.rng)
                        .unwrap();
                    sig.set(v);
                }
            }
            between(iter);
        }
    }

    fn gen_string(&mut self) -> String {
        let dict = [
            "OK", "go", "save", "load", "edit", "new", "next",
            "back", "yes", "no", "X", "tap", "value", "field",
            "label", "click",
        ];
        let n = self.rng.random_range(1..=3);
        let mut s = String::new();
        for i in 0..n {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(dict.choose(self.rng).unwrap());
        }
        s
    }
}
