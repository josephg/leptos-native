//! `<split_view>` + `<split_pane>` — native `NSSplitViewController`
//! integration with per-pane Taffy trees.
//!
//! Used to build Pages-style document UIs: a main canvas with a
//! collapsible inspector flyout on the right, or a sidebar +
//! content + inspector triptych. The split-view animation,
//! divider drag, and material vibrancy come from AppKit; layout
//! *inside* each pane is regular Taffy as everywhere else.
//!
//! ## API shape
//!
//! ```ignore
//! mount_to_split_window("Untitled", (1100.0, 720.0), || {
//!     view! {
//!         <split_view vertical=true>
//!             // Left/main pane: takes flex space, holds size last.
//!             <split_pane
//!                 holding_priority=199.0
//!             >
//!                 <Toolbar />
//!                 <Canvas />
//!             </split_pane>
//!             // Right inspector flyout. macOS animates the
//!             // collapse via `setCollapsed:`; we drive it from
//!             // a reactive signal.
//!             <split_pane
//!                 behavior=PaneBehavior::Inspector
//!                 preferred_thickness=300.0
//!                 minimum_thickness=240.0
//!                 maximum_thickness=420.0
//!                 can_collapse=true
//!                 collapsed=move || inspector.get() == Hidden
//!             >
//!                 <Inspector />
//!             </split_pane>
//!         </split_view>
//!     }
//! });
//! ```
//!
//! `<split_view>` must be used together with `mount_to_split_window`
//! (or [`SplitView::build_and_install`] for advanced callers) — it
//! sets the window's `contentViewController` to an
//! `NSSplitViewController`, which is *not* compatible with the
//! regular `mount_to_window` flow.

use crate::cocoa::attr::{install, IntoMaybeReactive, MaybeReactive};
use crate::Dom;
use cocoa_dom::split_window::{OpenedSplitWindow, PaneSpec};
use reactive_graph::effect::RenderEffect;
use renderer::view::{Mountable, Render};

// Re-export the cocoa-side enum so user code says
// `PaneBehavior::Inspector` without a separate import.
pub use cocoa_dom::split_window::{CollapseBehavior, PaneBehavior};

// ---------------------------------------------------------------------
// SplitPane builder
// ---------------------------------------------------------------------

/// One pane of a [`SplitView`]. Holds the pane's children + the
/// AppKit-level config (behavior, holding priority, sizing). Use
/// the chainable methods to configure; the `view!` macro emits
/// `<split_pane attr=val>...</split_pane>` calls.
///
/// Construction is **two-phase**: when `<split_pane>` first runs
/// inside the macro, it returns a `SplitPane<Children>` that
/// captures the children's *builder tuple* (not built yet). The
/// surrounding `<split_view>` extracts these at its own `build`
/// time, calls `split_window::open_split_window` to get a
/// real pane with a FlippedView + Taffy tree, then mounts each
/// pane's children into the pane's root.
pub struct SplitPane<Children> {
    pub(crate) behavior: PaneBehavior,
    pub(crate) preferred_thickness: Option<f64>,
    pub(crate) minimum_thickness:   Option<f64>,
    pub(crate) maximum_thickness:   Option<f64>,
    pub(crate) holding_priority:    Option<f32>,
    pub(crate) can_collapse:        Option<bool>,
    /// Reactive collapsed state. Static `bool` works too via
    /// `IntoMaybeReactive`. Driven by a signal in real apps so
    /// the toolbar toggle propagates here.
    pub(crate) collapsed:           Option<MaybeReactive<bool>>,
    pub(crate) collapse_behavior:   Option<CollapseBehavior>,
    pub(crate) children:            Children,
}

/// `split_pane()` — start configuring a pane. Defaults to
/// [`PaneBehavior::Default`] and no constraints.
pub fn split_pane() -> SplitPane<()> {
    SplitPane {
        behavior: PaneBehavior::Default,
        preferred_thickness: None,
        minimum_thickness:   None,
        maximum_thickness:   None,
        holding_priority:    None,
        can_collapse:        None,
        collapsed:           None,
        collapse_behavior:   None,
        children:            (),
    }
}

impl<Ch> SplitPane<Ch> {
    pub fn behavior(mut self, b: PaneBehavior) -> Self {
        self.behavior = b;
        self
    }

    /// Initial / restored width (vertical split) or height
    /// (horizontal). Maps to `NSSplitViewItem.preferredThicknessFraction`.
    pub fn preferred_thickness(mut self, n: f64) -> Self {
        self.preferred_thickness = Some(n);
        self
    }

    pub fn minimum_thickness(mut self, n: f64) -> Self {
        self.minimum_thickness = Some(n);
        self
    }

    pub fn maximum_thickness(mut self, n: f64) -> Self {
        self.maximum_thickness = Some(n);
        self
    }

    /// Auto-Layout holding priority. Lower-priority panes lose
    /// width first when the split-view resizes. For "fixed
    /// inspector + fluid content", give the content a *lower*
    /// number than the inspector (Apple's sample uses 199 / 200,
    /// but only the relative order matters — `NSLayoutPriority`'s
    /// `defaultLow` is `250`).
    pub fn holding_priority(mut self, p: f32) -> Self {
        self.holding_priority = Some(p);
        self
    }

    /// Whether user interactions can collapse this pane. Sidebar /
    /// Inspector behaviors default this to true; explicit
    /// `false` locks the pane open.
    pub fn can_collapse(mut self, b: bool) -> Self {
        self.can_collapse = Some(b);
        self
    }

    /// Reactive collapsed state. Pass `move || signal.get()` and
    /// the pane animates collapse/expand whenever the signal flips.
    pub fn collapsed<V>(mut self, c: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.collapsed = Some(c.into_maybe_reactive());
        self
    }

    /// How the surrounding layout reacts to this pane's collapse /
    /// expand. The two interesting choices:
    ///
    /// - [`CollapseBehavior::PreferResizingSiblingsWithFixedSplitView`] —
    ///   the window stays put; siblings absorb the freed space.
    ///   This is the Preview / Notes feel.
    /// - [`CollapseBehavior::PreferResizingSplitViewWithFixedSiblings`] —
    ///   siblings stay onscreen; the window grows / shrinks
    ///   instead. This is the Mail / Finder feel and is AppKit's
    ///   default for sidebar / inspector panes.
    ///
    /// Leaving this unset preserves AppKit's default.
    pub fn collapse_behavior(mut self, cb: CollapseBehavior) -> Self {
        self.collapse_behavior = Some(cb);
        self
    }

    /// `view!{}`-emitted child accumulator. Append `child` to the
    /// existing children tuple.
    pub fn child<NewCh>(self, child: NewCh) -> SplitPane<(Ch, NewCh)> {
        SplitPane {
            behavior:            self.behavior,
            preferred_thickness: self.preferred_thickness,
            minimum_thickness:   self.minimum_thickness,
            maximum_thickness:   self.maximum_thickness,
            holding_priority:    self.holding_priority,
            can_collapse:        self.can_collapse,
            collapsed:           self.collapsed,
            collapse_behavior:   self.collapse_behavior,
            children:            (self.children, child),
        }
    }

    /// Convert builder fields → caller-side `PaneSpec`. The
    /// reactive `collapsed` closure is sampled once for the
    /// initial value via `untrack` so the read doesn't register
    /// the SplitView's *build* context as a subscriber; the live
    /// subscription is wired separately at mount time inside
    /// `mount_one_pane`'s `install` call.
    fn to_spec(&self) -> PaneSpec {
        use reactive_graph::graph::untrack;
        let initial_collapsed = match &self.collapsed {
            Some(MaybeReactive::Static(b)) => *b,
            Some(MaybeReactive::Reactive(f)) => untrack(|| f()),
            None => false,
        };
        PaneSpec {
            behavior:            self.behavior,
            collapsed:           initial_collapsed,
            can_collapse:        self.can_collapse,
            preferred_thickness: self.preferred_thickness,
            minimum_thickness:   self.minimum_thickness,
            maximum_thickness:   self.maximum_thickness,
            holding_priority:    self.holding_priority,
            collapse_behavior:   self.collapse_behavior,
        }
    }
}

// ---------------------------------------------------------------------
// SplitView builder
// ---------------------------------------------------------------------

/// `<split_view>` element. Owns a list of `<split_pane>`s. Building
/// it creates the NSWindow + NSSplitViewController, mounts each
/// pane's children into its FlippedView root, and wires reactive
/// `collapsed` signals to `NSSplitViewItem.setCollapsed:`.
///
/// SplitView holds a `vertical` flag. By Cocoa convention,
/// `vertical=true` means the **dividers** are vertical (panes
/// are arranged side-by-side horizontally). `false` puts dividers
/// horizontal (panes stack top-to-bottom).
pub struct SplitView<Panes> {
    pub(crate) vertical: bool,
    pub(crate) panes:    Panes,
}

/// `split_view()` — start a split-view. Defaults to `vertical=true`
/// (side-by-side panes) since that's the dominant document-app
/// shape (main canvas + right inspector).
pub fn split_view() -> SplitView<()> {
    SplitView { vertical: true, panes: () }
}

impl<P> SplitView<P> {
    pub fn vertical(mut self, b: bool) -> Self {
        self.vertical = b;
        self
    }

    pub fn child<NewPane>(self, pane: NewPane) -> SplitView<(P, NewPane)> {
        SplitView { vertical: self.vertical, panes: (self.panes, pane) }
    }
}

// ---------------------------------------------------------------------
// SplitPaneList — trait for the panes tuple
// ---------------------------------------------------------------------

/// A flat tuple of `SplitPane`s — `()`, `(SplitPane,)`,
/// `(SplitPane, SplitPane)`, ... up to 8 elements. Implemented
/// per-arity rather than recursively because the `view!{}` macro
/// emits children as a flat tuple inside a single `.child(...)`
/// call (`split_view().child((p0, p1))`, not `.child(p0).child(p1)`).
pub trait Panes: Send + 'static {
    /// Mounted-children + reactive-effect state, kept for the
    /// SplitView's lifetime.
    type State: 'static;

    /// Number of panes in this tuple.
    fn pane_count(&self) -> usize;

    /// Push one [`PaneSpec`] per pane, in order.
    fn collect_specs(&self, out: &mut Vec<PaneSpec>);

    /// Build each pane's child view and mount it under the
    /// matching `OpenedSplitWindow::panes[i]`. Install the
    /// reactive-`collapsed` effects on each pane.
    fn mount_into(self, opened: &OpenedSplitWindow) -> Self::State;
}

/// Per-pane mount state — owns the child State and the optional
/// collapse-signal effect. Both fields exist solely to keep the
/// resources alive for the SplitView's lifetime; dropping this
/// struct unmounts the pane's children and cancels the collapsed
/// subscription.
pub struct PaneMountState<Ch: Render<Dom>> {
    pub _child_state: Ch::State,
    pub _collapsed_effect: Option<RenderEffect<()>>,
}

/// Internal helper: mount a single `SplitPane`'s children under
/// `pane` and wire its `collapsed` signal. Used by all the
/// per-arity `Panes` impls so the body is written once.
fn mount_one_pane<Ch>(
    pane: &cocoa_dom::split_window::Pane,
    sp: SplitPane<Ch>,
) -> PaneMountState<Ch>
where
    Ch: Render<Dom> + Send + 'static,
    <Ch as Render<Dom>>::State: Mountable<Dom> + 'static,
{
    let SplitPane { collapsed, children, .. } = sp;

    // Build and mount the children under the pane's FlippedView.
    // The returned State must be retained for the pane's lifetime
    // — dropping it would unmount and detach NSViews.
    let mut child_state = children.build(&pane.tree);
    child_state.mount(&pane.root, None);

    // Reactive collapse: install fires on every signal tick. The
    // closure animates the pane open/closed via the AppKit
    // animator proxy.
    let collapsed_effect = collapsed.and_then(|mr| {
        let item = pane.item.clone();
        install(mr, move |c: bool| {
            if item.isCollapsed() == c {
                return;
            }
            // Animator proxy → AppKit wraps the change in an
            // `NSAnimationContext` so it slides instead of
            // snapping. Same path the system's `toggleSidebar:` /
            // `toggleInspector:` actions take.
            unsafe {
                let animator: objc2::rc::Retained<
                    objc2_app_kit::NSSplitViewItem,
                > = objc2::msg_send![&*item, animator];
                animator.setCollapsed(c);
            }
        })
    });

    PaneMountState {
        _child_state: child_state,
        _collapsed_effect: collapsed_effect,
    }
}

// No `Panes for ()` impl: the empty-`<split_view></split_view>`
// case is handled by `SplitPaneList for ()` directly (see below),
// never reaching the `((), Panes)` flat-tuple path.

macro_rules! impl_panes_tuple {
    ($n:expr; $($idx:tt: $C:ident),+ $(,)?) => {
        impl<$($C),+> Panes for ( $(SplitPane<$C>,)+ )
        where
            $(
                $C: Render<Dom> + Send + 'static,
                <$C as Render<Dom>>::State: Mountable<Dom> + 'static,
            )+
        {
            type State = ( $(PaneMountState<$C>,)+ );

            fn pane_count(&self) -> usize { $n }

            fn collect_specs(&self, out: &mut Vec<PaneSpec>) {
                $( out.push(self.$idx.to_spec()); )+
            }

            fn mount_into(self, opened: &OpenedSplitWindow) -> Self::State {
                ( $( mount_one_pane(&opened.panes[$idx], self.$idx), )+ )
            }
        }
    };
}

impl_panes_tuple!(1; 0: C0);
impl_panes_tuple!(2; 0: C0, 1: C1);
impl_panes_tuple!(3; 0: C0, 1: C1, 2: C2);
impl_panes_tuple!(4; 0: C0, 1: C1, 2: C2, 3: C3);
impl_panes_tuple!(5; 0: C0, 1: C1, 2: C2, 3: C3, 4: C4);
impl_panes_tuple!(6; 0: C0, 1: C1, 2: C2, 3: C3, 4: C4, 5: C5);
impl_panes_tuple!(7; 0: C0, 1: C1, 2: C2, 3: C3, 4: C4, 5: C5, 6: C6);
impl_panes_tuple!(8; 0: C0, 1: C1, 2: C2, 3: C3, 4: C4, 5: C5, 6: C6, 7: C7);

/// `SplitPaneList` is the public bound on `SplitView<P>`. It
/// adapts the macro-folded `((), Panes)` shape to the flat
/// `Panes` tuple defined above: a single `<split_pane>` child
/// arrives as `((), SplitPane<C>)`, multiple children arrive as
/// `((), (SplitPane<C0>, SplitPane<C1>, ...))`.
pub trait SplitPaneList: Send + 'static {
    type State: 'static;
    fn pane_count(&self) -> usize;
    fn collect_specs(&self, out: &mut Vec<PaneSpec>);
    fn mount_into(self, opened: &OpenedSplitWindow) -> Self::State;
}

// Multi-pane case: macro emits `.child((p0, p1, ...))`.
impl<P> SplitPaneList for ((), P)
where
    P: Panes,
{
    type State = P::State;
    fn pane_count(&self) -> usize { self.1.pane_count() }
    fn collect_specs(&self, out: &mut Vec<PaneSpec>) {
        self.1.collect_specs(out)
    }
    fn mount_into(self, opened: &OpenedSplitWindow) -> Self::State {
        self.1.mount_into(opened)
    }
}

// Single-pane case: macro emits `.child(p0)` (no wrapping tuple).
// We need a separate impl because `(SplitPane<C>,)` (a 1-tuple) is
// distinct from `SplitPane<C>` (the value), and the macro produces
// the latter.
impl<C> SplitPaneList for ((), SplitPane<C>)
where
    C: Render<Dom> + Send + 'static,
    <C as Render<Dom>>::State: Mountable<Dom> + 'static,
{
    type State = (PaneMountState<C>,);
    fn pane_count(&self) -> usize { 1 }
    fn collect_specs(&self, out: &mut Vec<PaneSpec>) {
        out.push(self.1.to_spec());
    }
    fn mount_into(self, opened: &OpenedSplitWindow) -> Self::State {
        (mount_one_pane(&opened.panes[0], self.1),)
    }
}

// Empty SplitView — `<split_view></split_view>` with no children.
// The macro emits no `.child(...)` call at all, so `panes` keeps
// its initial `()`. Trivial impl.
impl SplitPaneList for () {
    type State = ();
    fn pane_count(&self) -> usize { 0 }
    fn collect_specs(&self, _out: &mut Vec<PaneSpec>) {}
    fn mount_into(self, _opened: &OpenedSplitWindow) -> Self::State {}
}

// ---------------------------------------------------------------------
// IntoSplitView — accept both `SplitView<P>` and the macro-wrapped
// `View<SplitView<P>>`
// ---------------------------------------------------------------------

/// Helper so [`crate::mount::mount_to_split_window`]'s closure can
/// return either the bare `SplitView` (for direct builder calls)
/// or the `view!`-macro-wrapped `View<SplitView<...>>`. Mirrors
/// the `Into<>` idiom but as a named trait so error messages name
/// it directly.
pub trait IntoSplitView<P>: 'static {
    fn into_split_view(self) -> SplitView<P>;
}

impl<P: 'static> IntoSplitView<P> for SplitView<P> {
    fn into_split_view(self) -> SplitView<P> {
        self
    }
}

impl<P: 'static> IntoSplitView<P> for leptos::View<SplitView<P>> {
    fn into_split_view(self) -> SplitView<P> {
        self.into_inner()
    }
}

// ---------------------------------------------------------------------
// Pure-Rust unit tests
// ---------------------------------------------------------------------
//
// These exercise the builder + trait-level logic without any
// AppKit calls. Integration tests covering the live mount path
// live in `cocoa/leptos_cocoa/tests/split_view.rs`.

#[cfg(test)]
mod tests {
    use super::*;
    use cocoa_dom::split_window::{PaneBehavior, PaneSpec};

    // ---- PaneSpec defaults --------------------------------------

    #[test]
    fn pane_spec_default_is_all_none_default_behavior() {
        let s = PaneSpec::default();
        assert_eq!(s.behavior, PaneBehavior::Default);
        assert_eq!(s.collapsed, false);
        assert!(s.can_collapse.is_none());
        assert!(s.preferred_thickness.is_none());
        assert!(s.minimum_thickness.is_none());
        assert!(s.maximum_thickness.is_none());
        assert!(s.holding_priority.is_none());
    }

    // ---- SplitPane builder --------------------------------------

    #[test]
    fn split_pane_default_builder_state() {
        let p: SplitPane<()> = split_pane();
        assert_eq!(p.behavior, PaneBehavior::Default);
        assert!(p.preferred_thickness.is_none());
        assert!(p.minimum_thickness.is_none());
        assert!(p.maximum_thickness.is_none());
        assert!(p.holding_priority.is_none());
        assert!(p.can_collapse.is_none());
        assert!(p.collapsed.is_none());
    }

    #[test]
    fn split_pane_builder_chaining_sets_each_field() {
        let p: SplitPane<()> = split_pane()
            .behavior(PaneBehavior::Inspector)
            .preferred_thickness(300.0)
            .minimum_thickness(200.0)
            .maximum_thickness(500.0)
            .holding_priority(199.0)
            .can_collapse(true)
            .collapsed(true);
        assert_eq!(p.behavior, PaneBehavior::Inspector);
        assert_eq!(p.preferred_thickness, Some(300.0));
        assert_eq!(p.minimum_thickness, Some(200.0));
        assert_eq!(p.maximum_thickness, Some(500.0));
        assert_eq!(p.holding_priority, Some(199.0));
        assert_eq!(p.can_collapse, Some(true));
        assert!(matches!(p.collapsed, Some(MaybeReactive::Static(true))));
    }

    // ---- to_spec translation ------------------------------------

    #[test]
    fn to_spec_forwards_all_static_fields() {
        let p: SplitPane<()> = split_pane()
            .behavior(PaneBehavior::Sidebar)
            .preferred_thickness(220.0)
            .minimum_thickness(180.0)
            .maximum_thickness(360.0)
            .holding_priority(200.0)
            .can_collapse(false)
            .collapsed(false);
        let s = p.to_spec();
        assert_eq!(s.behavior, PaneBehavior::Sidebar);
        assert_eq!(s.preferred_thickness, Some(220.0));
        assert_eq!(s.minimum_thickness, Some(180.0));
        assert_eq!(s.maximum_thickness, Some(360.0));
        assert_eq!(s.holding_priority, Some(200.0));
        assert_eq!(s.can_collapse, Some(false));
        assert_eq!(s.collapsed, false);
    }

    #[test]
    fn to_spec_with_no_options_set_returns_default_shape() {
        let p: SplitPane<()> = split_pane();
        let s = p.to_spec();
        assert_eq!(s.behavior, PaneBehavior::Default);
        assert_eq!(s.collapsed, false);
        assert!(s.preferred_thickness.is_none());
        assert!(s.minimum_thickness.is_none());
        assert!(s.maximum_thickness.is_none());
        assert!(s.holding_priority.is_none());
        assert!(s.can_collapse.is_none());
    }

    #[test]
    fn to_spec_samples_reactive_collapsed_via_untrack() {
        // The reactive closure is sampled once at to_spec-time so
        // the initial PaneSpec.collapsed matches the signal's
        // current value. The sample happens inside `untrack(...)`
        // — the to_spec call doesn't subscribe the surrounding
        // reactive context. We verify by reading the sampled bool.
        use reactive_graph::owner::Owner;
        use reactive_graph::signal::RwSignal;
        use reactive_graph::traits::{Get, Set};

        let _owner = Owner::new();
        _owner.set();

        let sig = RwSignal::new(true);
        let p: SplitPane<()> = split_pane()
            .collapsed(move || sig.get());
        assert!(p.to_spec().collapsed, "reactive=true sampled as true");

        sig.set(false);
        // to_spec is called fresh, so the second sample sees the
        // new value.
        let p: SplitPane<()> = split_pane()
            .collapsed(move || sig.get());
        assert!(!p.to_spec().collapsed, "reactive=false sampled as false");
    }

    // ---- Panes trait — pane_count, collect_specs ---------------

    fn make_pane(behavior: PaneBehavior, preferred: f64) -> SplitPane<()> {
        split_pane()
            .behavior(behavior)
            .preferred_thickness(preferred)
    }

    #[test]
    fn panes_arity_1_reports_one_pane() {
        let tuple = (make_pane(PaneBehavior::Default, 100.0),);
        assert_eq!(tuple.pane_count(), 1);
    }

    #[test]
    fn panes_arity_2_reports_two() {
        let tuple = (
            make_pane(PaneBehavior::Default, 100.0),
            make_pane(PaneBehavior::Inspector, 200.0),
        );
        assert_eq!(tuple.pane_count(), 2);
    }

    #[test]
    fn panes_arity_8_reports_eight() {
        let tuple = (
            make_pane(PaneBehavior::Default, 1.0),
            make_pane(PaneBehavior::Default, 2.0),
            make_pane(PaneBehavior::Default, 3.0),
            make_pane(PaneBehavior::Default, 4.0),
            make_pane(PaneBehavior::Default, 5.0),
            make_pane(PaneBehavior::Default, 6.0),
            make_pane(PaneBehavior::Default, 7.0),
            make_pane(PaneBehavior::Default, 8.0),
        );
        assert_eq!(tuple.pane_count(), 8);
    }

    #[test]
    fn collect_specs_produces_specs_left_to_right() {
        let tuple = (
            make_pane(PaneBehavior::Sidebar,     100.0),
            make_pane(PaneBehavior::Default,     200.0),
            make_pane(PaneBehavior::Inspector,   300.0),
        );
        let mut out = Vec::new();
        tuple.collect_specs(&mut out);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].behavior, PaneBehavior::Sidebar);
        assert_eq!(out[0].preferred_thickness, Some(100.0));
        assert_eq!(out[1].behavior, PaneBehavior::Default);
        assert_eq!(out[1].preferred_thickness, Some(200.0));
        assert_eq!(out[2].behavior, PaneBehavior::Inspector);
        assert_eq!(out[2].preferred_thickness, Some(300.0));
    }

    // ---- SplitPaneList adapter — single / multi / empty --------

    #[test]
    fn split_pane_list_single_routes_to_one_pane() {
        // Macro-emitted shape for `<split_view><split_pane/></split_view>`.
        let panes = ((), make_pane(PaneBehavior::Inspector, 250.0));
        assert_eq!(panes.pane_count(), 1);
        let mut out = Vec::new();
        panes.collect_specs(&mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].behavior, PaneBehavior::Inspector);
        assert_eq!(out[0].preferred_thickness, Some(250.0));
    }

    #[test]
    fn split_pane_list_multi_routes_to_panes_trait() {
        // Macro-emitted shape for two children.
        let panes = (
            (),
            (
                make_pane(PaneBehavior::Default, 100.0),
                make_pane(PaneBehavior::Inspector, 300.0),
            ),
        );
        assert_eq!(panes.pane_count(), 2);
        let mut out = Vec::new();
        panes.collect_specs(&mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].behavior, PaneBehavior::Default);
        assert_eq!(out[1].behavior, PaneBehavior::Inspector);
    }

    #[test]
    fn split_pane_list_empty_reports_zero() {
        let panes: () = ();
        assert_eq!(panes.pane_count(), 0);
        let mut out = Vec::new();
        panes.collect_specs(&mut out);
        assert!(out.is_empty());
    }

    // ---- SplitView builder shape -------------------------------

    #[test]
    fn split_view_default_is_vertical_no_panes() {
        let s = split_view();
        assert_eq!(s.vertical, true);
        // panes: () — confirm by constraining the type.
        let _: SplitView<()> = s;
    }

    #[test]
    fn split_view_vertical_setter() {
        let s = split_view().vertical(false);
        assert_eq!(s.vertical, false);
    }

    #[test]
    fn split_view_child_appends_to_panes_tuple() {
        let p = make_pane(PaneBehavior::Inspector, 200.0);
        let sv = split_view().child(p);
        // panes type is ((), SplitPane<()>) — confirm via pane_count
        // (which goes through the SplitPaneList single-pane impl).
        assert_eq!(sv.panes.pane_count(), 1);
    }
}

// ---------------------------------------------------------------------
// SplitView::build_and_install — the top-level entry point
// ---------------------------------------------------------------------

impl<P> SplitView<P>
where
    P: SplitPaneList,
{
    /// Open a split-view window and mount this `SplitView` into it.
    /// Returns the `OpenedSplitWindow` (for `toggle_inspector` /
    /// `toggle_sidebar` calls) + the per-pane mount state (kept
    /// alive for the app's lifetime by the mount entry point).
    pub fn build_and_install(
        self,
        title: &str,
        size: (f64, f64),
        mtm: cocoa_dom::MainThreadMarker,
    ) -> (OpenedSplitWindow, P::State) {
        let mut specs = Vec::new();
        self.panes.collect_specs(&mut specs);

        let opened = cocoa_dom::split_window::open_split_window(
            title,
            size,
            self.vertical,
            specs,
            mtm,
        );

        let state = self.panes.mount_into(&opened);
        (opened, state)
    }
}
