//! `<For>` — iterate over a collection and render each item.
//!
//! Currently the **unkeyed** variant only. Upstream's `<For>` did
//! keyed diffing via `tachys::view::keyed::keyed(...)` (959 lines of
//! position-vs-key bookkeeping); a port to
//! `common/renderer/src/view/keyed.rs` is on the punch list.
//!
//! Unkeyed semantics: items diff by position. If your list reorders,
//! every retained row gets `T::rebuild` called with whatever data
//! ended up at its old position — which means signal-keyed children
//! will re-read from the wrong row. Use stable positions only until
//! keyed `<For>` lands.
//!
//! `key=` is accepted but currently ignored. The prop signature is
//! kept stable so user code doesn't need editing once keyed lands.

use crate::into_view::IntoView;
use leptos_macro::component;
use reactive_graph::owner::Owner;
use renderer::{reactive_graph::OwnedView, renderer::Renderer};
use std::{hash::Hash, marker::PhantomData};

/// Iterates over children and displays them.
///
/// Unkeyed (see module docs). The `key` function is accepted for
/// forward-compatibility but not yet used.
#[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip_all))]
#[component]
pub fn For<IF, I, T, EF, N, KF, K, R>(
    /// Items to iterate over.
    each: IF,
    /// A key function. Currently unused; reserved for keyed diffing.
    #[allow(unused_variables)]
    key: KF,
    /// A function from item to view.
    children: EF,
    /// Marker for the renderer type parameter. Ignore.
    #[prop(optional)]
    _marker: PhantomData<R>,
) -> impl IntoView<R>
where
    R: Renderer,
    IF: Fn() -> I + Send + 'static,
    I: IntoIterator<Item = T> + Send + 'static,
    EF: Fn(T) -> N + Send + Clone + 'static,
    N: IntoView<R> + 'static,
    KF: Fn(&T) -> K + Send + Clone + 'static,
    K: Eq + Hash + 'static,
    T: Send + 'static,
{
    let parent = Owner::current().expect("no reactive owner");
    let _ = key; // suppress unused; reserved for keyed diff
    move || {
        each()
            .into_iter()
            .map(|item| {
                let owner = parent.with(Owner::new);
                let view = owner.with(|| children(item));
                OwnedView::new_with_owner(view, owner)
            })
            .collect::<Vec<_>>()
    }
}
