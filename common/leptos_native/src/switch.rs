//! `<Switch>` + `<Match>` — first-match branching control flow.
//!
//! Like upstream leptos's `Switch`/`Match`, but without AnyView. The
//! `<Switch>` accepts a heterogeneous tuple of `<Match>` siblings
//! (1..=8 branches); each `<Match>` carries its own `when: Fn() -> bool`
//! closure and child view. At runtime the first match whose `when`
//! returns `true` is rendered; if none match, nothing is rendered.
//!
//! ```rust,ignore
//! view! {
//!     <Switch>
//!         <Match when=move || page() == Page::Home><HomePage/></Match>
//!         <Match when=move || page() == Page::Settings><Settings/></Match>
//!         // No fallback: nothing renders when no `when` is true.
//!     </Switch>
//! }
//! ```
//!
//! The `<Match>` arms can have **different child types** — each branch
//! is wrapped in the appropriate `EitherOf{N}` variant internally so
//! the renderer sees a single sum-typed view that updates reactively.
//!
//! Limit of 8 is arbitrary; bump [`switch_tuple_impl!`] to grow it.
//! "What about 9?" — at that point your view function is doing too
//! much. Factor into nested switches or sub-components.

use crate::{
    children::TypedChildren,
    into_view::{IntoView, View},
};
use either_of::{Either, EitherOf3, EitherOf4, EitherOf5, EitherOf6, EitherOf7, EitherOf8};
use leptos_macro::component;
use renderer::{
    layout::TreeRef,
    renderer::Renderer,
    view::{AddAnyAttr, ApplyAttr, Render, UnitState},
};

// ---------------------------------------------------------------------
// Empty sentinel — the "no arm matched" branch
// ---------------------------------------------------------------------

/// Sentinel for an empty branch in control-flow components
/// (`<Switch>`'s no-arm-matched case, `<Show>`'s no-fallback +
/// `when == false` case). Its `Render` impl builds a real
/// platform `Placeholder` (via `UnitState<R>`) so the rendered
/// tree has a stable mount anchor for transitioning between
/// "empty" and "populated".
///
/// We can't use the bare `()` type here: `() : Render<R>` is a
/// no-op (no platform node), which means the rebuild path has
/// no anchor to splice content in front of when flipping
/// `empty → populated` — `Either::rebuild` calls
/// `old.insert_before_this(&mut new)` and `()` returns `false`
/// without mounting, so the new state is silently abandoned.
///
/// The `()`/no-op design is necessary for container builders to
/// seed empty child lists without producing a placeholder per
/// empty list; control-flow primitives are the niche where we
/// *want* the placeholder.
#[derive(Clone, Copy)]
pub struct EmptyBranch;

impl<R: Renderer> Render<R> for EmptyBranch {
    type State = UnitState<R>;
    fn build(self, tree: &TreeRef<R::Backend>) -> Self::State { UnitState::new(tree) }
    fn rebuild(self, _state: &mut Self::State) {}
}

impl<R: Renderer> AddAnyAttr<R> for EmptyBranch {
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        // Never emitted to user code; this impl exists only so the
        // type satisfies `IntoView`'s bound chain.
        self
    }
}

// ---------------------------------------------------------------------
// Match — a single branch
// ---------------------------------------------------------------------

/// One arm of a [`Switch`]. Carries a `when` predicate and the child
/// view to render when the predicate returns `true`.
///
/// You normally don't construct this directly; use `<Match>` inside a
/// `<Switch>`.
///
/// The struct is generic over `C` (child view type) and `R` (renderer)
/// only — the `when` closure is type-erased into a `Box<dyn Fn>` at
/// construction so its concrete type doesn't pollute call sites or
/// inflate monomorphization.
///
/// The boxed closures need `Send` (so that `Match` itself satisfies
/// `Render`'s `Send` bound, and ultimately `IntoView<R>`) but **not
/// `Sync`**: rendering happens on a single thread and the closures
/// are owned by their `Match`, never shared. Users can freely
/// capture `!Sync` state (e.g. `RefCell`-wrapped local state)
/// inside `when` / children.
pub struct Match<C, R>
where
    R: Renderer,
{
    pub(crate) when:     Box<dyn Fn() -> bool + Send + 'static>,
    pub(crate) children: Box<dyn Fn() -> View<C> + Send + 'static>,
    // `fn() -> R` makes the phantom Send regardless of R, and
    // avoids implying any drop relation to R.
    _marker:             std::marker::PhantomData<fn() -> R>,
}

// Match implements Render so the macro-emitted tuple `(M1, M2, …)`
// is itself Render via the existing tuple-of-Render impl. In
// practice Switch extracts each Match's `when` / `children` fields
// and never asks for these — the only way these get called is if
// someone writes `<Match/>` *outside* a `<Switch>`, which is user
// error. Panic with a clear message.
impl<R: Renderer, C> Render<R> for Match<C, R> {
    type State = ();
    #[track_caller]
    fn build(self, _tree: &TreeRef<R::Backend>) -> Self::State {
        panic!(
            "`<Match>` used outside of a `<Switch>`. `<Match>` is \
             only meaningful as a direct child of `<Switch>`; render \
             your view directly if you want it unconditionally."
        )
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

impl<R: Renderer, C> AddAnyAttr<R> for Match<C, R> {
    #[track_caller]
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        panic!(
            "`<Match on:click=...>` (or any attribute spread on \
             `<Match>`). `<Match>` is a control-flow marker, not a \
             real view — attach attributes to its child instead."
        )
    }
}

/// `<Match when=move || cond>...</Match>` — one branch of [`Switch`].
///
/// `transparent`: the component returns a [`Match`] value that the
/// surrounding `<Switch>` walks; no DOM node is produced here.
#[component(transparent)]
pub fn Match<W, C, R>(
    /// Condition for this branch. The first sibling-`Match` whose
    /// `when` returns `true` wins.
    when: W,
    /// View rendered when `when` matches. `TypedChildrenFn` so the
    /// child closure can be invoked many times — Switch re-renders
    /// when a `when` flips, and each render rebuilds the matched
    /// arm's view from this closure.
    children: crate::children::TypedChildrenFn<C, R>,
) -> Match<C, R>
where
    R: Renderer,
    W: Fn() -> bool + Send + 'static,
    C: IntoView<R> + 'static,
{
    // Unwrap the Arc<dyn Fn> into a Box<dyn Fn> matching Match's
    // storage. We forfeit `TypedChildrenFn`'s cheap clone — Match
    // can't be Clone anyway since it stores Box, not Arc.
    let children = children.into_inner();
    Match {
        when:     Box::new(when),
        children: Box::new(move || children()),
        _marker:  std::marker::PhantomData,
    }
}

// ---------------------------------------------------------------------
// SwitchBranches — selects + renders the first matching branch
// ---------------------------------------------------------------------

/// Implemented for tuples of [`Match`] values (1..=8). Two methods
/// split selection (a cheap `Option<usize>` we could memoize) from
/// rendering (recomposes the sum-typed view each tick).
///
/// `Send + 'static` only — UI rendering happens on the main thread,
/// and Switch never moves branches across threads. The bound stays
/// `Send` because [`IntoView`] requires it (Match's `Box<dyn Fn>`
/// storage is `Send` by construction, so users almost never see
/// this bound surface as a compile error).
pub trait SwitchBranches<R: Renderer>: Send + 'static {
    /// Sum-type view produced by [`Self::render`]. `None` means no
    /// branch matched.
    type Output: IntoView<R>;

    /// Returns `Some(i)` for the first matching branch, `None`
    /// otherwise. Reads the per-branch `when` closures and picks up
    /// reactive subscriptions on whatever signals they touch.
    fn select_index(&self) -> Option<usize>;

    /// Render the branch with the given index. Re-evaluating the
    /// child closure each call is intentional — it builds a fresh
    /// view that the reactive runtime will diff against the old.
    fn render(&self, idx: usize) -> Self::Output;
}

// One-branch case: a single `<Match>` collapses to `Option<View<C>>`
// (no need for a 1-variant Either). Switch returns `Option<Output>`
// for "no arm matched"; if Output is itself an Option that's
// `Option<Option<View<C>>>` — fine, both Render impls behave the
// same when the inner is None.
impl<C1, R> SwitchBranches<R> for (Match<C1, R>,)
where
    R: Renderer,
    C1: IntoView<R> + 'static,
{
    type Output = View<C1>;
    fn select_index(&self) -> Option<usize> {
        if (self.0.when)() { Some(0) } else { None }
    }
    fn render(&self, _idx: usize) -> Self::Output {
        (self.0.children)()
    }
}

macro_rules! switch_tuple_impl {
    (
        $( ( $either:ident, $( ($var:ident, $idx:tt, $c:ident) ),+ $(,)? ) ),+ $(,)?
    ) => {
        $(
            impl<$($c),+, R> SwitchBranches<R> for ( $(Match<$c, R>,)+ )
            where
                R: Renderer,
                $($c: IntoView<R> + 'static,)+
            {
                type Output = $either< $(View<$c>),+ >;

                fn select_index(&self) -> Option<usize> {
                    $(
                        if (self.$idx.when)() {
                            return Some($idx);
                        }
                    )+
                    None
                }

                fn render(&self, idx: usize) -> Self::Output {
                    $(
                        if idx == $idx {
                            return $either::$var((self.$idx.children)());
                        }
                    )+
                    // Caller is expected to only invoke `render` with
                    // an index returned by `select_index`. Hitting this
                    // means we updated one and forgot the other.
                    unreachable!("switch: invalid branch index {}", idx);
                }
            }
        )+
    };
}

// EitherOf2 lives in either_of as `Either<A, B>`. Spell it out so the
// macro impls all line up on a single Either-shaped type.
use either_of::Either as EitherOf2;
switch_tuple_impl! {
    (EitherOf2,
        (Left,  0, C1),
        (Right, 1, C2),
    ),
    (EitherOf3,
        (A, 0, C1),
        (B, 1, C2),
        (C, 2, C3),
    ),
    (EitherOf4,
        (A, 0, C1),
        (B, 1, C2),
        (C, 2, C3),
        (D, 3, C4),
    ),
    (EitherOf5,
        (A, 0, C1),
        (B, 1, C2),
        (C, 2, C3),
        (D, 3, C4),
        (E, 4, C5),
    ),
    (EitherOf6,
        (A, 0, C1),
        (B, 1, C2),
        (C, 2, C3),
        (D, 3, C4),
        (E, 4, C5),
        (F, 5, C6),
    ),
    (EitherOf7,
        (A, 0, C1),
        (B, 1, C2),
        (C, 2, C3),
        (D, 3, C4),
        (E, 4, C5),
        (F, 5, C6),
        (G, 6, C7),
    ),
    (EitherOf8,
        (A, 0, C1),
        (B, 1, C2),
        (C, 2, C3),
        (D, 3, C4),
        (E, 4, C5),
        (F, 5, C6),
        (G, 6, C7),
        (H, 7, C8),
    ),
}

// ---------------------------------------------------------------------
// Switch component
// ---------------------------------------------------------------------

/// A control-flow component that renders the first child `<Match>`
/// whose `when` returns `true`.
///
/// Place `<Match when=...>` siblings inside. Up to 8 branches today
/// (extend the macro if more are genuinely warranted).
///
/// ```rust,ignore
/// <Switch>
///     <Match when=move || tab() == Tab::Home><Home/></Match>
///     <Match when=move || tab() == Tab::Search><Search/></Match>
/// </Switch>
/// ```
#[component(transparent)]
pub fn Switch<B, R>(
    /// Tuple of `<Match>` children. `TypedChildren` (FnOnce-based)
    /// rather than `TypedChildrenFn` (Fn-based) because the tuple
    /// is invariant — Switch evaluates the children-producer
    /// exactly once at component build, then captures the
    /// resulting `Match` tuple in a reactive closure. The FnOnce
    /// shape lets users pass `move || (arms,)` closures that
    /// capture `!Sync` state.
    children: TypedChildren<B, R>,
) -> impl IntoView<R>
where
    R: Renderer,
    B: SwitchBranches<R>,
{
    // The tuple of Match values is invariant; only their `when`
    // outputs change. Pull it out once and move it into the
    // reactive closure — Match's `Send`-but-not-`Sync` storage
    // means we can't share it via `Arc`, but we don't need to.
    let branches: B = children.into_inner()().into_inner();
    move || {
        // Re-evaluate the per-branch `when` closures each tick.
        // The reactive runtime subscribes to whatever signals
        // they touch, so when any of those signals changes this
        // closure re-runs and we may select a different branch.
        //
        // The result is wrapped in `Either<EitherOf{N}, ()>` rather
        // than `Option<EitherOf{N}>` because `Either`'s rebuild
        // path handles variant-swap mounting via the previous
        // state's `insert_before_this` (the unit branch produces
        // a Placeholder, which serves as the mount anchor when
        // transitioning from "no arm matches" to "some arm
        // matches"). `Option<T>` lacks an anchor in its `None`
        // state and silently fails to mount on None→Some — see
        // `Option<T>`'s Render impl doc.
        //
        // No explicit memoization: `Either{N}::rebuild` already
        // short-circuits when the same variant is rebuilt, and the
        // index check is cheap (`when` closures are typically
        // single signal reads + a `==`).
        match branches.select_index() {
            Some(i) => Either::Left(branches.render(i)),
            None => Either::Right(EmptyBranch),
        }
    }
}
