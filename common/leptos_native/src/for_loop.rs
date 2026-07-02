//! `<For>` — keyed iteration over a collection.
//!
//! When the input list reorders, each row's view state follows its
//! key. Per-row signals continue reading from the right item; per-row
//! `NodeRef`s and reactive owners stay attached. The `key` function
//! must produce a value that's `Eq + Hash` and unique per row;
//! duplicate keys silently coalesce into one rendered row.
//!
//! Implementation: delegates to [`renderer::view::keyed`], the
//! `IndexSet`-backed diff ported from upstream `tachys`.

use crate::into_view::IntoView;
use leptos_macro::component;
use reactive_graph::owner::Owner;
use crate::renderer::{
    reactive_graph::OwnedView, Backend, view::keyed,
};
use std::{hash::Hash, marker::PhantomData};

/// Iterates over children, keyed by `key`.
///
/// `each` must produce an iterator. `key` extracts a hashable key per
/// item — when the list rebuilds (any signal `each` reads of changes),
/// rows whose keys match between old and new lists keep their built
/// state; new keys produce new rows; missing keys unmount their rows.
#[component]
pub fn For<IF, I, T, EF, N, KF, K, R>(
    /// Items to iterate over.
    each: IF,
    /// A key function applied to each item. Used to match rows
    /// between rebuilds.
    key: KF,
    /// A function from item to view.
    children: EF,
    /// Marker for the renderer type parameter. Ignore.
    #[prop(optional)]
    _marker: PhantomData<R>,
) -> impl IntoView<R>
where
    R: Backend,
    IF: Fn() -> I + Send + 'static,
    I: IntoIterator<Item = T> + Send + 'static,
    EF: Fn(T) -> N + Send + Clone + 'static,
    N: IntoView<R> + 'static,
    KF: Fn(&T) -> K + Send + Clone + 'static,
    K: Eq + Hash + 'static,
    T: Send + 'static,
{
    // Each row gets its own reactive Owner under the For's own. When a
    // row's key disappears between rebuilds, dropping the row's state
    // drops its Owner — which fires that subtree's cleanup callbacks
    // and unsubscribes its Effects.
    let parent = Owner::current().expect("no reactive owner");

    move || {
        let key_fn = key.clone();
        let children_fn = children.clone();
        let parent = parent.clone();
        keyed(
            each(),
            move |item: &T| key_fn(item),
            move |_idx, item| {
                let owner = parent.with(Owner::new);
                let view = owner.with(|| children_fn(item));
                OwnedView::new_with_owner(view, owner)
            },
        )
    }
}
