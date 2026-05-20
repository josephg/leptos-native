//! Keyed list-of-views diffing — minimal `Render<R>` port of upstream
//! tachys's `keyed.rs`.
//!
//! ## What this is for
//!
//! `<For>` accepts a `key=` function. With keyed diffing, when the
//! list reorders, each row's view state follows its key — signal-bound
//! children re-read from the right row, NodeRefs stay attached, etc.
//! The unkeyed [`Vec<T>: Render<R>`](crate::view::iterators) impl
//! diffs by position; if list elements get reordered, every retained
//! row gets `T::rebuild` called with whatever value ended up at its
//! old position, scrambling per-row state.
//!
//! ## What's different from upstream
//!
//! - No `RenderHtml` / `AddAnyAttr` / SSR / hydration. Native-only.
//! - No `SerializableKey` trait. Upstream needed it for the
//!   "islands routing" SSR feature; native has no analogue. Just
//!   `Eq + Hash` on the key type.
//! - No `(VFS, V)` set-index tuple. Upstream's `Keyed` carries an
//!   index-setter callback per row so `<ForEnumerate>` can reactively
//!   update each row's index when the list reorders. We don't have
//!   `<ForEnumerate>` yet — when it lands, the tuple shape can come
//!   back.
//!
//! ## Implementation
//!
//! On rebuild:
//! 1. Hash the new key list into an `IndexSet<K>`.
//! 2. Diff against the old `IndexSet<K>` → a list of removes, moves,
//!    and adds.
//! 3. Apply: unmount removed, take-and-replace moved, build-and-mount
//!    added.
//!
//! Adapted from `tachys/src/view/keyed.rs` (Greg Johnston / leptos).
//! The diff algorithm is unchanged; only the surrounding glue is.

use crate::{
    layout::TreeRef,
    renderer::Renderer,
    view::{Mountable, Render},
};
use indexmap::IndexSet;
use rustc_hash::FxHasher;
use std::hash::{BuildHasherDefault, Hash};

type FxIndexSet<T> = IndexSet<T, BuildHasherDefault<FxHasher>>;

/// Build a keyed list of views.
///
/// `items` is the iterable of data; `key_fn` extracts a hashable key
/// per item; `view_fn` produces the view for each item. View state is
/// re-used across rebuilds for items whose key matches.
pub fn keyed<T, I, K, KF, VF, V>(
    items: I,
    key_fn: KF,
    view_fn: VF,
) -> Keyed<T, I, K, KF, VF, V>
where
    I: IntoIterator<Item = T>,
    K: Eq + Hash + 'static,
    KF: Fn(&T) -> K,
    VF: Fn(usize, T) -> V,
{
    Keyed {
        items: Some(items),
        key_fn,
        view_fn,
        _marker: std::marker::PhantomData,
    }
}

/// A keyed list of views. Created by [`keyed`].
pub struct Keyed<T, I, K, KF, VF, V>
where
    I: IntoIterator<Item = T>,
    K: Eq + Hash + 'static,
    KF: Fn(&T) -> K,
    VF: Fn(usize, T) -> V,
{
    items: Option<I>,
    key_fn: KF,
    view_fn: VF,
    _marker: std::marker::PhantomData<fn() -> (T, V)>,
}

/// Retained view state for a [`Keyed`] list.
pub struct KeyedState<K, V, R>
where
    K: Eq + Hash + 'static,
    V: Render<R>,
    R: Renderer,
{
    tree: send_wrapper::SendWrapper<TreeRef<R::Backend>>,
    parent: Option<R::Node>,
    marker: R::Node,
    hashed_items: FxIndexSet<K>,
    rendered_items: Vec<Option<V::State>>,
}

impl<R, T, I, K, KF, VF, V> Render<R> for Keyed<T, I, K, KF, VF, V>
where
    R: Renderer,
    I: IntoIterator<Item = T>,
    K: Eq + Hash + 'static,
    KF: Fn(&T) -> K,
    VF: Fn(usize, T) -> V,
    V: Render<R>,
{
    type State = KeyedState<K, V, R>;

    fn build(self, tree: &TreeRef<R::Backend>) -> Self::State {
        let items = self.items.into_iter().flatten();
        let (capacity, _) = items.size_hint();
        let mut hashed_items =
            FxIndexSet::with_capacity_and_hasher(capacity, Default::default());
        let mut rendered_items = Vec::with_capacity(capacity);
        for (index, item) in items.enumerate() {
            hashed_items.insert((self.key_fn)(&item));
            let view = (self.view_fn)(index, item);
            rendered_items.push(Some(view.build(tree)));
        }
        KeyedState {
            tree: send_wrapper::SendWrapper::new(tree.clone()),
            parent: None,
            marker: R::create_placeholder(tree),
            hashed_items,
            rendered_items,
        }
    }

    fn rebuild(self, state: &mut Self::State) {
        let KeyedState {
            tree: tree_wrap,
            parent,
            marker,
            hashed_items,
            rendered_items,
        } = state;
        let tree: &TreeRef<R::Backend> = &**tree_wrap;
        let new_items = self.items.into_iter().flatten();
        let (capacity, _) = new_items.size_hint();
        let mut new_hashed_items =
            FxIndexSet::with_capacity_and_hasher(capacity, Default::default());

        let mut items = Vec::new();
        for item in new_items {
            new_hashed_items.insert((self.key_fn)(&item));
            items.push(Some(item));
        }

        let cmds = diff(hashed_items, &new_hashed_items);

        apply_diff::<R, T, V, VF>(
            tree,
            parent.as_ref(),
            marker,
            cmds,
            rendered_items,
            &self.view_fn,
            items,
        );

        *hashed_items = new_hashed_items;
    }
}

impl<R, K, V> Mountable<R> for KeyedState<K, V, R>
where
    R: Renderer,
    K: Eq + Hash + 'static,
    V: Render<R>,
{
    fn unmount(&mut self) {
        for state in self.rendered_items.iter_mut().flatten() {
            state.unmount();
        }
        self.marker.unmount();
        self.parent = None;
    }

    fn mount(&mut self, parent: &R::Node, marker: Option<&R::Node>) {
        self.parent = Some(parent.clone());
        self.marker.mount(parent, marker);
        for state in self.rendered_items.iter_mut().flatten() {
            // Insert each row before the trailing marker (so subsequent
            // adds keep the marker at the end of the rendered range).
            state.mount(parent, Some(self.marker.as_ref()));
        }
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        for state in self.rendered_items.iter().flatten() {
            if state.insert_before_this(child) {
                return true;
            }
        }
        self.marker.insert_before_this(child)
    }

    fn elements(&self) -> Vec<R::Node> {
        self.rendered_items
            .iter()
            .flatten()
            .flat_map(|s| s.elements())
            .collect()
    }
}

// ---------------------------------------------------------------------
// Diff algorithm (adapted from upstream tachys/view/keyed.rs)
// ---------------------------------------------------------------------

#[derive(Debug, Default, PartialEq, Eq)]
struct Diff {
    removed: Vec<DiffOpRemove>,
    moved: Vec<DiffOpMove>,
    items_to_move: usize,
    added: Vec<DiffOpAdd>,
    clear: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DiffOpMove {
    from: usize,
    len: usize,
    to: usize,
    move_in_dom: bool,
}

impl Default for DiffOpMove {
    fn default() -> Self {
        Self { from: 0, to: 0, len: 1, move_in_dom: true }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DiffOpAdd {
    at: usize,
    mode: DiffOpAddMode,
}

#[derive(Debug, PartialEq, Eq)]
struct DiffOpRemove {
    at: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum DiffOpAddMode {
    #[default]
    Normal,
    Append,
}

fn diff<K: Eq + Hash>(from: &FxIndexSet<K>, to: &FxIndexSet<K>) -> Diff {
    if from.is_empty() && to.is_empty() {
        return Diff::default();
    } else if to.is_empty() {
        return Diff { clear: true, ..Default::default() };
    } else if from.is_empty() {
        return Diff {
            added: to
                .iter()
                .enumerate()
                .map(|(at, _)| DiffOpAdd { at, mode: DiffOpAddMode::Append })
                .collect(),
            ..Default::default()
        };
    }

    let mut removed = vec![];
    let mut moved = vec![];
    let mut added = vec![];
    let max_len = std::cmp::max(from.len(), to.len());

    for index in 0..max_len {
        let from_item = from.get_index(index);
        let to_item = to.get_index(index);

        if from_item != to_item {
            if from_item.is_some() && !to.contains(from_item.unwrap()) {
                removed.push(DiffOpRemove { at: index });
            }
            if to_item.is_some() && !from.contains(to_item.unwrap()) {
                added.push(DiffOpAdd {
                    at: index,
                    mode: DiffOpAddMode::Normal,
                });
            }
            if let Some(from_item) = from_item {
                if let Some(to_item) = to.get_full(from_item) {
                    let moves_forward_by = (to_item.0 as i32) - (index as i32);
                    let move_in_dom = moves_forward_by
                        != (added.len() as i32) - (removed.len() as i32);
                    moved.push(DiffOpMove {
                        from: index,
                        len: 1,
                        to: to_item.0,
                        move_in_dom,
                    });
                }
            }
        }
    }

    moved = group_adjacent_moves(moved);

    Diff {
        removed,
        items_to_move: moved.iter().map(|m| m.len).sum(),
        moved,
        added,
        clear: false,
    }
}

fn group_adjacent_moves(moved: Vec<DiffOpMove>) -> Vec<DiffOpMove> {
    let mut prev: Option<DiffOpMove> = None;
    let mut new_moved = Vec::with_capacity(moved.len());
    for m in moved {
        match prev {
            Some(mut p) => {
                if (m.from == p.from + p.len) && (m.to == p.to + p.len) {
                    p.len += 1;
                    prev = Some(p);
                } else {
                    new_moved.push(prev.take().unwrap());
                    prev = Some(m);
                }
            }
            None => prev = Some(m),
        }
    }
    if let Some(prev) = prev {
        new_moved.push(prev)
    }
    new_moved
}

fn apply_diff<R, T, V, VF>(
    tree: &TreeRef<R::Backend>,
    parent: Option<&R::Node>,
    marker: &R::Node,
    diff: Diff,
    children: &mut Vec<Option<V::State>>,
    view_fn: &VF,
    mut items: Vec<Option<T>>,
) where
    R: Renderer,
    V: Render<R>,
    VF: Fn(usize, T) -> V,
{
    // Order: clear → removes → move-out → resize → move-in → adds →
    // drop holes.
    if diff.clear {
        for mut child in children.drain(0..).flatten() {
            child.unmount();
        }
        if diff.added.is_empty() {
            return;
        }
    }

    for DiffOpRemove { at } in &diff.removed {
        let mut item_to_remove = children[*at].take().unwrap();
        item_to_remove.unmount();
    }

    let (move_cmds, add_cmds) = unpack_moves(&diff);

    let mut moved_children = move_cmds
        .iter()
        .map(|m| children[m.from].take())
        .collect::<Vec<_>>();

    children.resize_with(children.len() + diff.added.len(), || None);

    // Logical moves (no DOM/UI churn — index changed but ordering is
    // preserved relative to siblings on the same axis).
    for (i, DiffOpMove { to, .. }) in move_cmds
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.move_in_dom)
    {
        children[*to] = moved_children[i].take();
    }

    // Real moves (need to re-mount in the platform tree).
    for (i, DiffOpMove { to, .. }) in
        move_cmds.into_iter().enumerate().filter(|(_, m)| m.move_in_dom)
    {
        let mut each_item = moved_children[i].take().unwrap();
        if let Some(parent) = parent {
            if let Some(Some(state)) =
                get_next_closest_mounted_sibling(children, to)
            {
                state.insert_before_this_or_marker(
                    parent,
                    &mut each_item,
                    Some(marker.as_ref()),
                );
            } else {
                each_item.try_mount(parent, Some(marker.as_ref()));
            }
        }
        children[to] = Some(each_item);
    }

    for DiffOpAdd { at, mode } in add_cmds {
        let item = items[at].take().unwrap();
        let view = view_fn(at, item);
        let mut state = view.build(tree);
        if let Some(parent) = parent {
            match mode {
                DiffOpAddMode::Normal => {
                    if let Some(Some(sibling)) =
                        get_next_closest_mounted_sibling(children, at)
                    {
                        sibling.insert_before_this_or_marker(
                            parent,
                            &mut state,
                            Some(marker.as_ref()),
                        );
                    } else {
                        state.try_mount(parent, Some(marker.as_ref()));
                    }
                }
                DiffOpAddMode::Append => {
                    state.try_mount(parent, Some(marker.as_ref()));
                }
            }
        }
        children[at] = Some(state);
    }

    children.retain(|c| c.is_some());
}

fn get_next_closest_mounted_sibling<T>(
    v: &Vec<Option<T>>,
    start_at: usize,
) -> Option<&Option<T>> {
    v[start_at..].iter().find(|s| s.is_some())
}

fn unpack_moves(diff: &Diff) -> (Vec<DiffOpMove>, Vec<DiffOpAdd>) {
    let mut moves = Vec::with_capacity(diff.items_to_move);
    let mut adds = Vec::with_capacity(diff.added.len());

    let mut removes_iter = diff.removed.iter();
    let mut adds_iter = diff.added.iter();
    let mut moves_iter = diff.moved.iter();

    let mut removes_next = removes_iter.next();
    let mut adds_next = adds_iter.next();
    let mut moves_next = moves_iter.next().copied();

    for i in 0..diff.items_to_move + diff.added.len() + diff.removed.len() {
        if let Some(DiffOpRemove { at }) = removes_next {
            if i == *at {
                removes_next = removes_iter.next();
                continue;
            }
        }

        match (adds_next, &mut moves_next) {
            (Some(add), Some(move_)) => {
                if add.at == i {
                    adds.push(*add);
                    adds_next = adds_iter.next();
                } else {
                    let mut single_move = *move_;
                    single_move.len = 1;
                    moves.push(single_move);
                    move_.len -= 1;
                    move_.from += 1;
                    move_.to += 1;
                    if move_.len == 0 {
                        moves_next = moves_iter.next().copied();
                    }
                }
            }
            (Some(add), None) => {
                adds.push(*add);
                adds_next = adds_iter.next();
            }
            (None, Some(move_)) => {
                let mut single_move = *move_;
                single_move.len = 1;
                moves.push(single_move);
                move_.len -= 1;
                move_.from += 1;
                move_.to += 1;
                if move_.len == 0 {
                    moves_next = moves_iter.next().copied();
                }
            }
            (None, None) => break,
        }
    }

    (moves, adds)
}

#[cfg(test)]
mod tests {
    //! Pure-Rust tests of the diff algorithm. No platform Renderer
    //! involved — we test what `diff()` produces from `IndexSet` →
    //! `IndexSet`. The `apply_diff` path that touches platform
    //! `Mountable` impls is exercised by the cocoa/iOS counters
    //! examples + their layout tests.

    use super::*;

    fn set<I: IntoIterator<Item = i32>>(it: I) -> FxIndexSet<i32> {
        it.into_iter().collect()
    }

    #[test]
    fn empty_to_empty() {
        let d = diff::<i32>(&set([]), &set([]));
        assert!(!d.clear);
        assert!(d.added.is_empty());
        assert!(d.removed.is_empty());
        assert!(d.moved.is_empty());
    }

    #[test]
    fn empty_to_nonempty_appends_all() {
        let d = diff(&set([]), &set([1, 2, 3]));
        assert_eq!(d.added.len(), 3);
        assert!(d.added.iter().all(|a| a.mode == DiffOpAddMode::Append));
        assert!(d.removed.is_empty());
        assert!(d.moved.is_empty());
    }

    #[test]
    fn nonempty_to_empty_clears() {
        let d = diff(&set([1, 2, 3]), &set([]));
        assert!(d.clear);
        assert!(d.added.is_empty());
    }

    #[test]
    fn append_one() {
        let d = diff(&set([1, 2, 3]), &set([1, 2, 3, 4]));
        assert_eq!(d.added.len(), 1);
        assert_eq!(d.added[0].at, 3);
        assert!(d.removed.is_empty());
        assert!(d.moved.is_empty());
    }

    #[test]
    fn remove_one() {
        let d = diff(&set([1, 2, 3]), &set([1, 3]));
        assert_eq!(d.removed.len(), 1);
        assert_eq!(d.removed[0].at, 1); // index 1 = "2" was removed
    }

    #[test]
    fn swap_two() {
        // [1, 2, 3, 4, 5] -> [1, 4, 3, 2, 5] swaps positions 1 and 3.
        let d = diff(&set([1, 2, 3, 4, 5]), &set([1, 4, 3, 2, 5]));
        assert!(d.removed.is_empty());
        assert!(d.added.is_empty());
        // Two moves: 2 (at 1 -> 3) and 4 (at 3 -> 1).
        assert_eq!(d.moved.len(), 2);
    }

    #[test]
    fn reverse() {
        let d = diff(&set([1, 2, 3]), &set([3, 2, 1]));
        assert!(d.removed.is_empty());
        assert!(d.added.is_empty());
        // 1 and 3 swap; 2 stays put (so only 2 moves recorded).
        assert_eq!(d.moved.len(), 2);
    }

    #[test]
    fn group_adjacent() {
        // [1, 2, 3, 4] -> [3, 4, 1, 2]: two pairs each move as a group.
        let d = diff(&set([1, 2, 3, 4]), &set([3, 4, 1, 2]));
        // After grouping: [{1,2 -> 2..4}, {3,4 -> 0..2}]
        assert_eq!(d.moved.len(), 2);
        assert!(d.moved.iter().any(|m| m.len == 2));
    }
}
