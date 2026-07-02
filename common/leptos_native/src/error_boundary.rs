//! `<ErrorBoundary>` — catches `Result::Err` thrown by descendant
//! views (via the `throw_error` hook system, see `Render<R>` for
//! `Result<T, E>` in `renderer::view::result`) and renders a fallback.
//!
//! Pared down from upstream's 726-line version, which carried
//! `RenderHtml`/`SharedContext`/SSR machinery the native ports don't
//! need. What's preserved:
//!  - The same prop signature: `fallback: F where F: FnMut(ArcRwSignal<Errors>) -> Fal`.
//!  - The same `Errors` map shape: `FxHashMap<ErrorId, Error>`.
//!  - The reactive flip between children and fallback driven by
//!    `errors_empty` memo + `RenderEffect`.

use crate::renderer::node::Node;
use crate::{
    children::TypedChildren,
    into_view::IntoView,
};
use leptos_macro::component;
use reactive_graph::{
    computed::ArcMemo,
    effect::RenderEffect,
    owner::Owner,
    signal::ArcRwSignal,
    traits::{Get, Update, With},
};
use crate::renderer::{
    reactive_graph::{OwnedView, RenderEffectState},
    Backend,
    view::{AddAnyAttr, ApplyAttr, Mountable, Render},
};
use rustc_hash::FxHashMap;
use std::{fmt::Debug, marker::PhantomData, sync::Arc};
use throw_error::{Error, ErrorHook, ErrorId};

/// `<ErrorBoundary>` — when a descendant `Result<T, E>` view evaluates
/// to `Err`, the error is caught here and the fallback is rendered.
///
/// `fallback` receives an [`ArcRwSignal<Errors>`] it can read to
/// display per-error messages.
#[component]
pub fn ErrorBoundary<FalFn, Fal, Chil, R>(
    /// The elements that will be rendered, which may include one or more `Result<_>` types.
    children: TypedChildren<Chil, R>,
    /// A fallback that will be shown if an error occurs.
    fallback: FalFn,
    /// Marker for the renderer type parameter. Ignore.
    #[prop(optional)]
    _marker: PhantomData<R>,
) -> impl IntoView<R>
where
    R: Backend,
    FalFn: FnMut(ArcRwSignal<Errors>) -> Fal + Send + 'static,
    Fal: IntoView<R> + Send + 'static,
    Chil: IntoView<R> + Send + 'static,
{
    let hook = Arc::new(ErrorBoundaryErrorHook::new([]));
    let errors = hook.errors.clone();
    let errors_empty = ArcMemo::new({
        let errors = errors.clone();
        move |_| errors.with(|map| map.is_empty())
    });
    let hook_dyn: Arc<dyn ErrorHook> = hook.clone();

    // Set the hook so children's `Result<T, E>: Render` impls register
    // their errors against THIS boundary, not whatever ancestor hook
    // was active.
    let _guard = throw_error::set_error_hook(Arc::clone(&hook_dyn));

    let owner = Owner::new();
    let children = owner.with(|| children.into_inner()());

    OwnedView::new_with_owner(
        ErrorBoundaryView {
            hook: hook_dyn,
            errors_empty,
            children,
            fallback,
            errors,
            _marker: PhantomData::<R>,
        },
        owner,
    )
}

struct ErrorBoundaryView<Chil, FalFn, R> {
    hook: Arc<dyn ErrorHook>,
    errors_empty: ArcMemo<bool>,
    children: Chil,
    fallback: FalFn,
    errors: ArcRwSignal<Errors>,
    _marker: PhantomData<R>,
}

struct ErrorBoundaryViewState<Chil, Fal> {
    children: Chil,
    fallback: Option<Fal>,
}

impl<R, Chil, Fal> Mountable<R> for ErrorBoundaryViewState<Chil, Fal>
where
    R: Backend,
    Chil: Mountable<R>,
    Fal: Mountable<R>,
{
    fn unmount(&mut self) {
        if let Some(fallback) = &mut self.fallback {
            fallback.unmount();
        } else {
            self.children.unmount();
        }
    }

    fn mount(&mut self, parent: Node<R>, marker: Option<Node<R>>) {
        if let Some(fallback) = &mut self.fallback {
            fallback.mount(parent, marker);
        } else {
            self.children.mount(parent, marker);
        }
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(fallback) = &self.fallback {
            fallback.insert_before_this(child)
        } else {
            self.children.insert_before_this(child)
        }
    }

    fn elements(&self) -> Vec<Node<R>> {
        if let Some(fallback) = &self.fallback {
            fallback.elements()
        } else {
            self.children.elements()
        }
    }
}

impl<R, Chil, FalFn, Fal> Render<R> for ErrorBoundaryView<Chil, FalFn, R>
where
    R: Backend,
    Chil: Render<R> + 'static,
    FalFn: FnMut(ArcRwSignal<Errors>) -> Fal + Send + 'static,
    Fal: Render<R> + 'static,
{
    type State = RenderEffectState<
        ErrorBoundaryViewState<Chil::State, Fal::State>,
        R,
    >;

    fn build(mut self) -> Self::State {
        let hook = Arc::clone(&self.hook);
        let _hook_guard = throw_error::set_error_hook(Arc::clone(&hook));
        let mut children = Some(self.children.build());
        let effect = RenderEffect::new(
            move |prev: Option<
                ErrorBoundaryViewState<Chil::State, Fal::State>,
            >| {
                let _hook_guard =
                    throw_error::set_error_hook(Arc::clone(&hook));
                if let Some(mut state) = prev {
                    match (self.errors_empty.get(), &mut state.fallback) {
                        // no errors, fallback was showing -> swap back
                        (true, Some(fallback)) => {
                            fallback.insert_before_this(&mut state.children);
                            fallback.unmount();
                            state.fallback = None;
                        }
                        // errors appeared, was showing children -> swap
                        (false, None) => {
                            state.fallback = Some(
                                (self.fallback)(self.errors.clone())
                                    .build(),
                            );
                            state
                                .children
                                .insert_before_this(&mut state.fallback);
                            state.children.unmount();
                        }
                        // unchanged in either direction
                        _ => {}
                    }
                    state
                } else {
                    let fallback = (!self.errors_empty.get())
                        .then(|| {
                            (self.fallback)(self.errors.clone())
                                .build()
                        });
                    ErrorBoundaryViewState {
                        children: children.take().unwrap(),
                        fallback,
                    }
                }
            },
        );
        RenderEffectState::from_parts(effect)
    }

    fn rebuild(self, state: &mut Self::State) {
        let new = self.build();
        let mut old = std::mem::replace(state, new);
        old.insert_before_this(state);
        old.unmount();
    }
}

#[derive(Debug)]
struct ErrorBoundaryErrorHook {
    errors: ArcRwSignal<Errors>,
}

impl ErrorBoundaryErrorHook {
    pub fn new(
        initial_errors: impl IntoIterator<Item = (ErrorId, Error)>,
    ) -> Self {
        Self {
            errors: ArcRwSignal::new(Errors(
                initial_errors.into_iter().collect(),
            )),
        }
    }
}

impl ErrorHook for ErrorBoundaryErrorHook {
    fn throw(&self, error: Error) -> ErrorId {
        // Hash the boundary's pointer + a counter to mint a unique ID
        // without needing a SharedContext (the upstream native source).
        // ErrorId implements Default; use sequential variants here.
        let key: ErrorId = self.errors.with(|m| m.0.len()).into();

        self.errors.update(|map| {
            map.insert(key.clone(), error);
        });

        key
    }

    fn clear(&self, id: &ErrorId) {
        self.errors.update(|map| {
            map.remove(id);
        });
    }
}

/// A holder for all the errors registered against an [`ErrorBoundary`].
#[derive(Debug, Clone, Default)]
#[repr(transparent)]
pub struct Errors(FxHashMap<ErrorId, Error>);

impl Errors {
    /// Returns `true` if there are no errors.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Add an error.
    pub fn insert<E>(&mut self, key: ErrorId, error: E)
    where
        E: Into<Error>,
    {
        self.0.insert(key, error.into());
    }

    /// Add an error with the default key.
    pub fn insert_with_default_key<E>(&mut self, error: E)
    where
        E: Into<Error>,
    {
        self.0.insert(Default::default(), error.into());
    }

    /// Remove an error by key.
    pub fn remove(&mut self, key: &ErrorId) -> Option<Error> {
        self.0.remove(key)
    }

    /// Iterate over errors.
    #[inline(always)]
    pub fn iter(&self) -> Iter<'_> {
        Iter(self.0.iter())
    }
}

impl IntoIterator for Errors {
    type Item = (ErrorId, Error);
    type IntoIter = IntoIter;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter())
    }
}

/// Owning iterator over [`Errors`].
#[repr(transparent)]
pub struct IntoIter(std::collections::hash_map::IntoIter<ErrorId, Error>);

impl Iterator for IntoIter {
    type Item = (ErrorId, Error);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Borrowing iterator over [`Errors`].
#[repr(transparent)]
pub struct Iter<'a>(std::collections::hash_map::Iter<'a, ErrorId, Error>);

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a ErrorId, &'a Error);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// `<ErrorBoundary on:click=…>` — branching wrapper, panic with a
/// clear message rather than silently dropping the handler.
impl<Chil, FalFn, R> AddAnyAttr<R> for ErrorBoundaryView<Chil, FalFn, R>
where
    R: Backend,
{
    #[track_caller]
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        panic!(
            "AddAnyAttr<R>::add_any_attr called on `<ErrorBoundary>`. \
             ErrorBoundary swaps between children and fallback at \
             runtime, so attaching a spread attribute to the boundary \
             itself isn't well-defined. Attach the attribute to the \
             inner element (the children OR the fallback) instead."
        )
    }
}
