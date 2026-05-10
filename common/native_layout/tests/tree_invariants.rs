//! Regression tests for `LayoutTree<B>` mutation invariants.
//!
//! Each test uses a tiny `TestBackend` whose `View` is `()` so the
//! tree shape is exercised without any platform glue.

use native_layout::{LayoutBackend, LayoutTree, Size, Style};

struct TestBackend;

impl LayoutBackend for TestBackend {
    type View = ();
    type NodeMeta = ();

    fn measure_leaf(
        _: &Self::View,
        known: Size<Option<f32>>,
        _: Size<native_layout::AvailableSpace>,
    ) -> Size<f32> {
        Size {
            width: known.width.unwrap_or(0.0),
            height: known.height.unwrap_or(0.0),
        }
    }

    fn first_baseline(_: &Self::View) -> Option<f32> {
        None
    }
}

fn fresh() -> std::rc::Rc<LayoutTree<TestBackend>> {
    LayoutTree::<TestBackend>::new()
}

fn leaf(tree: &LayoutTree<TestBackend>) -> native_layout::NodeId {
    tree.new_leaf(Style::default(), (), ())
}

// ---------------------------------------------------------------------
// remove() must mark the parent dirty
// ---------------------------------------------------------------------

/// REGRESSION: removing a child without dirtying the parent leaves
/// the parent's cached flex layout referencing a now-stale children
/// list. Subsequent `compute_layout` returns the cached output and
/// the removed child's space is still reserved.
#[test]
fn remove_dirties_former_parent() {
    let tree = fresh();
    let parent = leaf(&tree);
    let child = leaf(&tree);
    tree.add_child(parent, child);

    // Run a layout pass to populate the parent's cache.
    tree.run_layout_pass(
        parent,
        Size {
            width: native_layout::AvailableSpace::Definite(100.0),
            height: native_layout::AvailableSpace::Definite(100.0),
        },
    );
    assert!(!tree.dirty(parent), "parent should be clean after layout");

    tree.remove(child);

    assert!(
        tree.dirty(parent),
        "parent should be dirty after removing a child"
    );
}

// ---------------------------------------------------------------------
// add_child must detach from previous parent
// ---------------------------------------------------------------------

/// REGRESSION: re-parenting a node via `add_child(new_parent, child)`
/// while it's still under `old_parent` previously left the same
/// NodeId in both parents' children lists, since `add_child` only
/// touched `new_parent`'s list. AppKit/UIKit's `addSubview:` and
/// GTK's widget tree both maintain "one parent" invariant; our tree
/// must match.
#[test]
fn add_child_detaches_from_previous_parent() {
    let tree = fresh();
    let a = leaf(&tree);
    let b = leaf(&tree);
    let child = leaf(&tree);

    tree.add_child(a, child);
    assert_eq!(*tree.children(a), [child]);
    assert_eq!(tree.parent(child), Some(a));

    tree.add_child(b, child);

    assert!(
        tree.children(a).is_empty(),
        "a should have no children after re-parenting"
    );
    assert_eq!(*tree.children(b), [child]);
    assert_eq!(tree.parent(child), Some(b));
}

/// Both old and new parents should be marked dirty when re-parenting.
#[test]
fn add_child_dirties_both_parents_on_reparent() {
    let tree = fresh();
    let a = leaf(&tree);
    let b = leaf(&tree);
    let child = leaf(&tree);
    tree.add_child(a, child);

    let avail = Size {
        width: native_layout::AvailableSpace::Definite(100.0),
        height: native_layout::AvailableSpace::Definite(100.0),
    };
    tree.run_layout_pass(a, avail);
    tree.run_layout_pass(b, avail);
    assert!(!tree.dirty(a));
    assert!(!tree.dirty(b));

    tree.add_child(b, child);

    assert!(tree.dirty(a), "old parent should be dirty");
    assert!(tree.dirty(b), "new parent should be dirty");
}

// ---------------------------------------------------------------------
// add_child is a no-op when the edge already exists
// ---------------------------------------------------------------------

/// Idempotent re-add shouldn't bloat the children list.
#[test]
fn add_child_is_idempotent() {
    let tree = fresh();
    let parent = leaf(&tree);
    let child = leaf(&tree);

    tree.add_child(parent, child);
    tree.add_child(parent, child);
    tree.add_child(parent, child);

    assert_eq!(*tree.children(parent), [child]);
}

/// Idempotent re-add shouldn't dirty the tree (no work to redo).
#[test]
fn add_child_idempotent_re_add_doesnt_dirty() {
    let tree = fresh();
    let parent = leaf(&tree);
    let child = leaf(&tree);
    tree.add_child(parent, child);
    tree.run_layout_pass(
        parent,
        Size {
            width: native_layout::AvailableSpace::Definite(100.0),
            height: native_layout::AvailableSpace::Definite(100.0),
        },
    );
    assert!(!tree.dirty(parent));

    tree.add_child(parent, child);

    assert!(
        !tree.dirty(parent),
        "idempotent re-add should not invalidate cache"
    );
}

// ---------------------------------------------------------------------
// insert_child_at_index must detach from previous parent
// ---------------------------------------------------------------------

#[test]
fn insert_child_at_index_detaches_from_previous_parent() {
    let tree = fresh();
    let a = leaf(&tree);
    let b = leaf(&tree);
    let child = leaf(&tree);
    tree.add_child(a, child);

    tree.insert_child_at_index(b, 0, child);

    assert!(
        tree.children(a).is_empty(),
        "a should have no children after re-parenting"
    );
    assert_eq!(*tree.children(b), [child]);
    assert_eq!(tree.parent(child), Some(b));
}

#[test]
fn insert_child_at_index_can_reorder_within_same_parent() {
    let tree = fresh();
    let parent = leaf(&tree);
    let a = leaf(&tree);
    let b = leaf(&tree);
    let c = leaf(&tree);
    tree.add_child(parent, a);
    tree.add_child(parent, b);
    tree.add_child(parent, c);
    assert_eq!(*tree.children(parent), [a, b, c]);

    // Move `a` to the end.
    tree.insert_child_at_index(parent, 2, a);

    assert_eq!(*tree.children(parent), [b, c, a]);
    assert_eq!(tree.parent(a), Some(parent));
}

// ---------------------------------------------------------------------
// Tree shape integrity under reparenting bursts
// ---------------------------------------------------------------------

/// Equivalent of a keyed-`<For>` move: re-add several children that
/// were already under the same parent, in order. Should be a no-op
/// for the children list (Mountable cascades visit children in
/// construction order, which is the order we already have).
#[test]
fn add_child_burst_under_same_parent_preserves_order() {
    let tree = fresh();
    let parent = leaf(&tree);
    let a = leaf(&tree);
    let b = leaf(&tree);
    let c = leaf(&tree);
    tree.add_child(parent, a);
    tree.add_child(parent, b);
    tree.add_child(parent, c);

    // Cascade re-add (same order).
    tree.add_child(parent, a);
    tree.add_child(parent, b);
    tree.add_child(parent, c);

    assert_eq!(*tree.children(parent), [a, b, c]);
}

/// `tree.children(id)` should return an empty slice for a missing
/// node rather than panicking.
#[test]
fn children_of_missing_node_is_empty() {
    let tree = fresh();
    let bogus = native_layout::NodeId::from(usize::MAX);
    assert!(tree.children(bogus).is_empty());
}

/// `tree.layout(id)` returns `None` for a missing node.
#[test]
fn layout_of_missing_node_is_none() {
    let tree = fresh();
    let bogus = native_layout::NodeId::from(usize::MAX);
    assert!(tree.layout(bogus).is_none());
}
