//! Deterministic, seed-driven random generation of [`Node`] trees.
//!
//! The generator chooses element types, attributes, depth, and the
//! reactive-vs-static treatment of each attribute pseudorandomly
//! from a [`ChaCha8Rng`] seeded by the user's seed. Two runs with
//! the same seed produce identical trees and identical chaos
//! sequences, which is the property the fuzzer needs for
//! reproducible failures.

use crate::spec::{Attr, ContainerKind, Node, SignalId};
use rand::prelude::*;
use rand::rngs::ChaCha8Rng;

pub struct Generator<'a> {
    rng: &'a mut ChaCha8Rng,
    /// Monotonically-increasing ids handed out to reactive attrs.
    next_signal_id: SignalId,
    /// Configured fraction of attrs that should be reactive
    /// (0.0–1.0). Defaults to 0.5.
    pub reactive_fraction: f64,
    /// Maximum depth (counting root as 0).
    pub max_depth: u32,
    /// Children per container clamp.
    pub max_children: u32,
    /// Probability of wrapping a generated node in a `Show`
    /// (conditional). Set to 0 to disable shape-changing nodes
    /// (only attr reactivity). Default 0.15.
    pub show_probability: f64,
    /// Probability of emitting a `DynamicList` (length-driven
    /// bulk-rebuild) at each gen_node call. Defaults to 0; opt in
    /// to exercise the AnyView::rebuild path on a vstack whose
    /// child vector mutates per chaos write.
    pub dynamic_list_probability: f64,
    /// Probability of emitting a `Grid` at each gen_container
    /// call. Defaults to 0; opt in to exercise grid placement +
    /// the Taffy grid solver.
    pub grid_probability: f64,
}

impl<'a> Generator<'a> {
    pub fn new(rng: &'a mut ChaCha8Rng) -> Self {
        Self {
            rng,
            next_signal_id: 0,
            reactive_fraction: 0.5,
            max_depth: 4,
            max_children: 5,
            // Disabled by default — the Show plumbing through
            // type-erased reactive closures hits async-task /
            // owner-disposal ordering issues that need more
            // careful drain logic before it's reliable. Set
            // explicitly via `Generator { show_probability:
            // 0.1, .. }` to opt in.
            show_probability: 0.0,
            dynamic_list_probability: 0.0,
            grid_probability: 0.0,
        }
    }

    pub fn generate(&mut self) -> Node {
        // Top level is always a container so the tree has some
        // shape worth diffing.
        self.gen_container(0)
    }

    fn fresh_id(&mut self) -> SignalId {
        let id = self.next_signal_id;
        self.next_signal_id += 1;
        id
    }

    fn attr_static<T>(&mut self, value: T) -> Attr<T> {
        Attr::Static(value)
    }

    fn maybe_reactive<T>(&mut self, value: T) -> Attr<T> {
        if self.rng.random_bool(self.reactive_fraction) {
            Attr::Reactive {
                id: self.fresh_id(),
                initial: value,
            }
        } else {
            self.attr_static(value)
        }
    }

    fn maybe_present_reactive<T>(&mut self, value: T) -> Option<Attr<T>> {
        if self.rng.random_bool(0.5) {
            Some(self.maybe_reactive(value))
        } else {
            None
        }
    }

    // -- value generators --------------------------------------------

    fn gen_string(&mut self) -> String {
        // ASCII words, 1-3 short tokens. Avoiding empty strings
        // and very long ones for clarity in diff output.
        let dict = [
            "ok", "save", "cancel", "undo", "delete", "add", "next",
            "back", "yes", "no", "hi", "hello", "click", "load",
            "go", "stop", "reset", "submit", "edit", "view",
        ];
        let n = self.rng.random_range(1..=3);
        let mut s = String::new();
        for i in 0..n {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(dict.choose(&mut self.rng).unwrap());
        }
        s
    }

    fn random_bool(&mut self) -> bool {
        self.rng.random_bool(0.5)
    }

    fn gen_padding(&mut self) -> f32 {
        // Small set of round numbers.
        *[0.0, 4.0, 8.0, 12.0, 16.0]
            .choose(&mut self.rng)
            .unwrap()
    }

    fn gen_gap(&mut self) -> f32 {
        *[0.0, 4.0, 8.0, 12.0].choose(&mut self.rng).unwrap()
    }

    // -- node generators ---------------------------------------------

    fn gen_container(&mut self, depth: u32) -> Node {
        let kind = *[
            ContainerKind::VStack,
            ContainerKind::HStack,
            ContainerKind::View,
            // scroll_view chosen rarely — must have a bounded
            // parent or it'll occupy unbounded space. Grandchild
            // of a stack/grid is usually fine.
            ContainerKind::ScrollView,
        ]
        .choose(&mut self.rng)
        .unwrap();

        let padding_v = self.gen_padding();
        let padding = self.maybe_present_reactive(padding_v);
        let gap_v = self.gen_gap();
        let gap = self.maybe_present_reactive(gap_v);

        let max_kids = if depth + 1 >= self.max_depth {
            // Only leaves at max depth.
            self.max_children.max(1)
        } else {
            self.max_children
        };
        let n = self.rng.random_range(1..=max_kids);

        let children = (0..n)
            .map(|_| self.gen_node(depth + 1))
            .collect();

        Node::Container {
            kind,
            padding,
            gap,
            children,
        }
    }

    fn gen_leaf(&mut self) -> Node {
        // Pick a leaf element uniformly across the full builder
        // surface. Each field is built from temporaries so the
        // borrow checker is happy about the `&mut self` chain in
        // `maybe_*` helpers.
        let kind: u8 = self.rng.random_range(0..13);
        match kind {
            0 => {
                let t = self.gen_string();
                let title = self.maybe_reactive(t);
                let e = self.random_bool();
                let enabled = self.maybe_present_reactive(e);
                let h = self.random_bool();
                let hidden = self.maybe_present_reactive(h);
                Node::Button { title, enabled, hidden }
            }
            1 => {
                let t = self.gen_string();
                let text = self.maybe_reactive(t);
                let h = self.random_bool();
                let hidden = self.maybe_present_reactive(h);
                Node::Label { text, hidden }
            }
            2 => {
                let t = self.gen_string();
                let title = self.maybe_reactive(t);
                let c = self.random_bool();
                let checked = self.maybe_present_reactive(c);
                Node::Checkbox { title, checked }
            }
            3 | 4 => {
                // text_field (3) / secure_text_field (4) — same
                // shape, different cocoa builder constructor.
                let secure = kind == 4;
                let v = self.gen_string();
                let value = self.maybe_reactive(v);
                let p = self.gen_string();
                let placeholder = self.maybe_present_reactive(p);
                let e = self.random_bool();
                let enabled = self.maybe_present_reactive(e);
                Node::TextField {
                    value,
                    placeholder,
                    enabled,
                    secure,
                }
            }
            5 => {
                let v = self.gen_string();
                let value = self.maybe_reactive(v);
                let e = self.random_bool();
                let enabled = self.maybe_present_reactive(e);
                Node::TextView { value, enabled }
            }
            6 => {
                // slider — value in [0,1].
                let v = self.gen_unit_f64();
                let value = self.maybe_reactive_f64(v);
                let e = self.random_bool();
                let enabled = self.maybe_present_reactive(e);
                let vertical = self.rng.random_bool(0.2);
                Node::Slider { value, enabled, vertical }
            }
            7 => {
                // stepper — value in [0, 100].
                let v = (self.rng.random_range(0..=100)) as f64;
                let value = self.maybe_reactive_f64(v);
                let e = self.random_bool();
                let enabled = self.maybe_present_reactive(e);
                Node::Stepper { value, enabled }
            }
            8 => {
                // progress_indicator
                let v = self.gen_unit_f64();
                let value = self.maybe_reactive_f64(v);
                let indeterminate = if self.rng.random_bool(0.3) {
                    let b = self.random_bool();
                    Some(self.maybe_reactive(b))
                } else {
                    None
                };
                Node::ProgressIndicator { value, indeterminate }
            }
            9 | 10 => {
                // pop_up_button (9) / segmented_control (10)
                let n_items = self.rng.random_range(2..=5);
                let items: Vec<String> =
                    (0..n_items).map(|_| self.gen_string()).collect();
                let initial = self.rng.random_range(0..n_items);
                let selection = self.maybe_reactive_index(initial);
                let e = self.random_bool();
                let enabled = self.maybe_present_reactive(e);
                if kind == 9 {
                    Node::PopUpButton { items, selection, enabled }
                } else {
                    Node::SegmentedControl { items, selection, enabled }
                }
            }
            11 => {
                let e = self.random_bool();
                let enabled = self.maybe_present_reactive(e);
                if self.rng.random_bool(0.5) {
                    Node::DatePicker { enabled }
                } else {
                    Node::ColorWell { enabled }
                }
            }
            _ => {
                // image_view — pick from a tiny SF symbol set
                // that's stable across macOS versions.
                let symbols = [
                    "star", "heart", "bolt", "moon", "sun.max",
                    "circle", "square", "triangle", "checkmark",
                ];
                let sym = *symbols.choose(&mut self.rng).unwrap();
                Node::ImageView {
                    sf_symbol: sym.to_owned(),
                }
            }
        }
    }

    fn gen_unit_f64(&mut self) -> f64 {
        *[0.0, 0.25, 0.5, 0.75, 1.0]
            .choose(&mut self.rng)
            .unwrap()
    }

    fn maybe_reactive_f64(&mut self, value: f64) -> Attr<f64> {
        if self.rng.random_bool(self.reactive_fraction) {
            Attr::Reactive {
                id: self.fresh_id(),
                initial: value,
            }
        } else {
            Attr::Static(value)
        }
    }

    fn maybe_reactive_index(&mut self, value: usize) -> Attr<usize> {
        if self.rng.random_bool(self.reactive_fraction) {
            Attr::Reactive {
                id: self.fresh_id(),
                initial: value,
            }
        } else {
            Attr::Static(value)
        }
    }

    fn gen_node(&mut self, depth: u32) -> Node {
        // Sometimes wrap in a Show — shape-changing conditional
        // driven by a bool signal. Allowed everywhere except at
        // max depth (Show holds at least one nested branch).
        if depth + 1 < self.max_depth
            && self.rng.random_bool(self.show_probability)
        {
            let init = self.random_bool();
            let when = self.maybe_reactive(init);
            // Force `when` to be reactive — a Show with a static
            // condition is useless (the chaos loop can't toggle
            // it). Re-roll until we get the reactive form.
            let when = match when {
                Attr::Reactive { .. } => when,
                Attr::Static(v) => Attr::Reactive {
                    id: self.fresh_id(),
                    initial: v,
                },
            };
            let on = Box::new(self.gen_node(depth + 1));
            let off = if self.rng.random_bool(0.5) {
                Some(Box::new(self.gen_node(depth + 1)))
            } else {
                None
            };
            return Node::Show { when, on, off };
        }
        // Sometimes emit a DynamicList — a length-driven
        // bulk-rebuild container. Like Show, requires room for at
        // least one nested generated leaf below.
        if depth + 1 < self.max_depth
            && self.rng.random_bool(self.dynamic_list_probability)
        {
            let max = self.rng.random_range(1..=4);
            // Force `count` reactive so chaos can drive it.
            let initial = self.rng.random_range(0..=max);
            let count = Attr::Reactive {
                id: self.fresh_id(),
                initial,
            };
            let template = Box::new(self.gen_node(depth + 1));
            return Node::DynamicList { count, max, template };
        }
        if depth >= self.max_depth {
            return self.gen_leaf();
        }
        // Sometimes emit a grid (only if there's depth budget for
        // the cell children).
        if depth + 1 < self.max_depth
            && self.rng.random_bool(self.grid_probability)
        {
            let columns = self.rng.random_range(1..=4);
            let rows = self.rng.random_range(1..=4);
            // Number of placed children — at least 1, up to
            // columns*rows*2 (with intentional collisions allowed).
            let n_kids =
                self.rng.random_range(1..=(columns * rows * 2).min(8));
            let mut kids = Vec::with_capacity(n_kids);
            for _ in 0..n_kids {
                let col = self.rng.random_range(1..=columns);
                let row = self.rng.random_range(1..=rows);
                kids.push((col, row, self.gen_node(depth + 1)));
            }
            return Node::Grid {
                columns,
                rows,
                children: kids,
            };
        }
        if self.rng.random_bool(0.4) {
            self.gen_container(depth)
        } else {
            self.gen_leaf()
        }
    }
}

/// Convenience: seed → freshly-generated `Node`.
pub fn generate_from_seed(seed: u64) -> Node {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut g = Generator::new(&mut rng);
    g.generate()
}

