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
use crate::cocoa::element::ElementState;
use crate::Dom;
use cocoa_dom::split_window::{OpenedSplitWindow, PaneSpec};
use reactive_graph::effect::RenderEffect;
use renderer::view::{Mountable, Render};

// Re-export the cocoa-side enum so user code says
// `PaneBehavior::Inspector` without a separate import.
pub use cocoa_dom::split_window::PaneBehavior;

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
    /// width first when the split-view resizes. Use 199 (one less
    /// than `defaultLow=200`) on the content pane to keep
    /// sidebars / inspectors at their preferred width.
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
/// collapse-signal effect.
pub struct PaneMountState<Ch: Render<Dom>> {
    pub _child_state: ElementState<(), Ch::State>,
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

    let mut child_state = children.build();
    child_state.mount(&pane.root, None);
    let wrapped = ElementState {
        el: pane.root.clone(),
        _effects: Vec::new(),
        _attrs: std::marker::PhantomData,
        children: child_state,
    };

    let collapsed_effect = collapsed.and_then(|mr| {
        let item = pane.item.clone();
        install(mr, move |c: bool| {
            if item.isCollapsed() == c {
                return;
            }
            // Use the animator proxy so signal-driven collapse
            // slides instead of snapping. The controller's built-
            // in `toggleSidebar:` / `toggleInspector:` actions
            // also animate via this path.
            unsafe {
                let animator: objc2::rc::Retained<
                    objc2_app_kit::NSSplitViewItem,
                > = objc2::msg_send![&*item, animator];
                animator.setCollapsed(c);
            }
        })
    });

    PaneMountState {
        _child_state: wrapped,
        _collapsed_effect: collapsed_effect,
    }
}

// Empty pane list — degenerate case (`<split_view></split_view>`).
// Unlikely in real code but trivial to support and keeps the
// generic-`Panes` bound satisfiable for 0-pane SplitViews.
impl Panes for () {
    type State = ();
    fn pane_count(&self) -> usize { 0 }
    fn collect_specs(&self, _out: &mut Vec<PaneSpec>) {}
    fn mount_into(self, _opened: &OpenedSplitWindow) -> Self::State {}
}

macro_rules! impl_panes_tuple {
    ($($n:tt: $C:ident),+ $(,)?) => {
        impl<$($C),+> Panes for ( $(SplitPane<$C>,)+ )
        where
            $(
                $C: Render<Dom> + Send + 'static,
                <$C as Render<Dom>>::State: Mountable<Dom> + 'static,
            )+
        {
            type State = ( $(PaneMountState<$C>,)+ );

            fn pane_count(&self) -> usize {
                let mut count = 0;
                $( let _ = self.$n; count += 1; )+
                count
            }

            fn collect_specs(&self, out: &mut Vec<PaneSpec>) {
                $( out.push(self.$n.to_spec()); )+
            }

            fn mount_into(self, opened: &OpenedSplitWindow) -> Self::State {
                // Pull each pane out of `self` and mount into the
                // matching opened-pane index.
                ( $( mount_one_pane(&opened.panes[$n], self.$n), )+ )
            }
        }
    };
}

impl_panes_tuple!(0: C0);
impl_panes_tuple!(0: C0, 1: C1);
impl_panes_tuple!(0: C0, 1: C1, 2: C2);
impl_panes_tuple!(0: C0, 1: C1, 2: C2, 3: C3);
impl_panes_tuple!(0: C0, 1: C1, 2: C2, 3: C3, 4: C4);
impl_panes_tuple!(0: C0, 1: C1, 2: C2, 3: C3, 4: C4, 5: C5);
impl_panes_tuple!(0: C0, 1: C1, 2: C2, 3: C3, 4: C4, 5: C5, 6: C6);
impl_panes_tuple!(0: C0, 1: C1, 2: C2, 3: C3, 4: C4, 5: C5, 6: C6, 7: C7);

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
