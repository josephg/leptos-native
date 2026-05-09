//! mark_dirty discipline tests.
//!
//! Per CLAUDE.md (and `feedback_taffy_mark_dirty.md`), every
//! cocoa_dom layout mutation must explicitly mark_dirty its parent
//! before scheduling relayout. These tests pin that contract by
//! reading Taffy's dirty bit directly after each kind of mutation.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{
    layout::{compute_layout, register_in_tree, TreeRef},
    Element,
};
use objc2_foundation::NSSize;

fn fresh_tree() -> TreeRef {
    cocoa_dom::layout::new_tree()
}

fn dirty_for(tree: &TreeRef, el: &Element) -> bool {
    let lh = el
        .as_node()
        .layout_slot()
        .borrow()
        .handle
        .clone()
        .expect("element has no LayoutHandle — wasn't registered");
    tree.tree.borrow().dirty(lh.node_id).unwrap_or(true)
}

/// After `compute_layout`, the root's dirty bit is cleared.
/// Subsequent mutations need to set it back.
fn baseline_compute_clears_dirty() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_with("vstack", mtm);
    register_in_tree(root.as_node(), &tree);

    compute_layout(root.as_node(), NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&tree, &root), "root still dirty after compute");
}

/// Inserting a child marks the parent dirty.
fn attach_child_marks_parent_dirty() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_with("vstack", mtm);
    register_in_tree(root.as_node(), &tree);
    compute_layout(root.as_node(), NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&tree, &root));

    let child = Element::create_with("button", mtm);
    cocoa_dom::layout::attach_child(root.as_node(), child.as_node());

    assert!(
        dirty_for(&tree, &root),
        "parent not marked dirty after attach_child"
    );
}

/// Detaching a child marks the parent dirty (so the parent re-runs
/// flexbox without the removed child).
fn detach_child_marks_parent_dirty() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_with("vstack", mtm);
    register_in_tree(root.as_node(), &tree);
    let child = Element::create_with("button", mtm);
    cocoa_dom::layout::attach_child(root.as_node(), child.as_node());
    compute_layout(root.as_node(), NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&tree, &root));

    cocoa_dom::layout::detach_child(root.as_node(), child.as_node());

    assert!(
        dirty_for(&tree, &root),
        "parent not marked dirty after detach_child"
    );
}

/// Setting an attribute that affects size/text marks the node
/// dirty so its measure callback re-runs.
fn set_text_marks_node_dirty() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_with("vstack", mtm);
    register_in_tree(root.as_node(), &tree);
    let child = Element::create_with("label", mtm);
    cocoa_dom::layout::attach_child(root.as_node(), child.as_node());
    compute_layout(root.as_node(), NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&tree, &child));

    child.set_attribute("value", "now I have content");

    assert!(
        dirty_for(&tree, &child),
        "label not marked dirty after text change — measure cache will \
         be stale"
    );
}

/// `set_style` (e.g. width / padding) marks the node dirty.
fn set_style_width_marks_node_dirty() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_with("vstack", mtm);
    register_in_tree(root.as_node(), &tree);
    compute_layout(root.as_node(), NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&tree, &root));

    cocoa_dom::layout::set_width(root.as_node(), 150.0);

    assert!(
        dirty_for(&tree, &root),
        "node not marked dirty after set_width"
    );
}

fn main() {
    common::run_tests(&[
        ("baseline_compute_clears_dirty", baseline_compute_clears_dirty),
        ("attach_child_marks_parent_dirty", attach_child_marks_parent_dirty),
        ("detach_child_marks_parent_dirty", detach_child_marks_parent_dirty),
        ("set_text_marks_node_dirty", set_text_marks_node_dirty),
        ("set_style_width_marks_node_dirty", set_style_width_marks_node_dirty),
    ]);
}

