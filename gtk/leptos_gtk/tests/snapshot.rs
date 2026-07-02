//! PR-0: node-tree snapshot oracle for the GTK port.
//!
//! Pins the *structure and geometry* of the layout arena so the
//! upcoming insertion-unification work (and the trait redesign after
//! it) has a mechanical regression net instead of "run counter_gtk and
//! eyeball it". Each test builds a tree, runs the Taffy pass, serializes
//! `(tag, child order, computed rect)` per node, and diffs against a
//! checked-in golden file under `tests/golden/`.
//!
//! These tests deliberately work at the **dom layer**
//! (`create_vstack` / `create_label` / `insert_node` / `remove_child`)
//! with **fixed leaf sizes**, for two reasons:
//!   1. They exercise exactly the code the insertion canary changes
//!      (`GtkElem::try_insert_node`'s Taffy mirroring), with no reliance
//!      on reactive-effect scheduling.
//!   2. Pinning every leaf's `Style.size` makes the computed rects pure
//!      Taffy arithmetic — deterministic and identical across machines,
//!      so the goldens don't drift with the system font. Builder→dom
//!      wiring is already covered by `tests/layout.rs`.
//!
//! Generating / updating goldens: run with `UPDATE_GOLDEN=1` (or just
//! delete the golden file) and the test writes the current snapshot and
//! prints it for review. Once reviewed and committed, later runs assert
//! against it. Headless CI skips (see `common::run_tests`).

#![cfg(feature = "gtk")]

mod common;

use std::fmt::Write as _;

use leptos_gtk::dom::{GtkMakeView, GtkNodeExt};
use leptos_gtk::dom::{layout, layout::GtkBackend, GtkElem};
use leptos_native::renderer::{length, Backend, NodeId, Rect, Size};

const W: f32 = 320.0;
const H: f32 = 200.0;

// ---- snapshot serializer (generic-ready: only Backend accessors) -------

fn snapshot(root: GtkElem) -> String {
    let mut out = String::new();
    dump(root.id(), 0, &mut out);
    out
}

fn dump(id: NodeId, depth: usize, out: &mut String) {
    let tag = GtkBackend::debug_tag_name(id);
    let tag = if tag.is_empty() { "<node>" } else { tag };
    let l = GtkBackend::layout(id).expect("every live node has a computed layout");
    // Cast to i32: round_layout already integralises, and this keeps the
    // golden free of "16.0" vs "16" float-formatting noise.
    let _ = writeln!(
        out,
        "{:indent$}{tag} {w}x{h} @ {x},{y}",
        "",
        w = l.size.width as i32,
        h = l.size.height as i32,
        x = l.location.x as i32,
        y = l.location.y as i32,
        indent = depth * 2,
    );
    for c in GtkBackend::children(id) {
        dump(c, depth + 1, out);
    }
}

// ---- deterministic builders (fixed sizes => machine-independent goldens) ------

/// A fixed-size leaf (a `gtk::Label`, retagged for readability). Its
/// explicit `Style.size` means Taffy never calls the font-dependent
/// measure path for it.
fn leaf(tag: &'static str) -> GtkElem {
    let el = GtkElem::create_label().0.with_tag(tag);
    let mut s = GtkBackend::style(el.id()).expect("leaf has a style");
    s.size = Size { width: length(100.0), height: length(20.0) };
    GtkBackend::set_style(el.id(), s);
    el
}

/// A column container with fixed 320x200 size, 16px padding, 12px
/// row-gap — so children land at predictable integer positions.
fn column() -> GtkElem {
    let v = GtkElem::create_vstack().with_tag("vstack");
    let mut s = GtkBackend::style(v.id()).expect("vstack has a style");
    s.size = Size { width: length(W), height: length(H) };
    s.padding = Rect {
        left: length(16.0),
        right: length(16.0),
        top: length(16.0),
        bottom: length(16.0),
    };
    s.gap = Size { width: length(0.0), height: length(12.0) };
    GtkBackend::set_style(v.id(), s);
    v
}

// ---- golden compare ----------------------------------------------------------

fn check(name: &str, actual: String) {
    let path = format!("tests/golden/{name}.txt");
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    match std::fs::read_to_string(&path) {
        Ok(expected) if !update => {
            assert!(
                expected == actual,
                "snapshot mismatch for `{name}`.\n\
                 Run with UPDATE_GOLDEN=1 to re-record if this change is intended.\n\
                 --- expected (tests/golden/{name}.txt) ---\n{expected}\
                 --- actual ---\n{actual}",
            );
        }
        _ => {
            std::fs::create_dir_all("tests/golden").expect("create tests/golden");
            std::fs::write(&path, &actual).expect("write golden");
            println!(
                "  WROTE tests/golden/{name}.txt — REVIEW before committing:\n{actual}"
            );
        }
    }
}

// ---- the cases ---------------------------------------------------------------

// Every edge is built through `insert_node` — the native+Taffy path that
// `Mountable::mount` uses in production. (Do NOT mix in `layout::attach_child`,
// which mirrors into Taffy only and leaves the native tree unparented; the
// native-index readback in `insert_node` then disagrees with the Taffy order.
// `append` = a `None` marker.)
fn append(parent: GtkElem, child: GtkElem) {
    parent.insert_node(child, None);
}

/// Baseline: a static three-child column. General layout regression net.
fn static_tree() {
    let v = column();
    append(v, leaf("a"));
    append(v, leaf("b"));
    append(v, leaf("c"));
    layout::compute_layout(v, (W, H));
    check("static_tree", snapshot(v));
    v.remove();
}

/// Insert a child *before a mid-list marker* — the exact path the
/// insertion canary rewrites. `b` must land between `a` and `c`, in
/// both the native order and the Taffy order, or this golden moves.
fn insert_middle() {
    let v = column();
    let a = leaf("a");
    let c = leaf("c");
    append(v, a);
    append(v, c);
    v.insert_node(leaf("b"), Some(c)); // before c
    layout::compute_layout(v, (W, H));
    check("insert_middle", snapshot(v));
    v.remove();
}

/// Remove a mid-list child; the survivors must close the gap in order.
fn remove_middle() {
    let v = column();
    let a = leaf("a");
    let b = leaf("b");
    let c = leaf("c");
    append(v, a);
    append(v, b);
    append(v, c);
    v.remove_child(b);
    b.remove();
    layout::compute_layout(v, (W, H));
    check("remove_middle", snapshot(v));
    v.remove();
}

fn main() {
    common::run_tests(&[
        ("static_tree", static_tree),
        ("insert_middle", insert_middle),
        ("remove_middle", remove_middle),
    ]);
}
