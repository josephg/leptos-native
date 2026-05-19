//! `NSSplitViewController`-backed split view with a per-pane Taffy
//! tree. Used when the app wants a native Cocoa split — fly-out
//! sidebars and inspectors with the system's animation, divider
//! drag, vibrant material — without giving up the Taffy layout
//! inside each pane.
//!
//! ## Structure
//!
//! ```text
//! NSWindow
//!   └── contentViewController = NSSplitViewController
//!         ├── splitViewItem[0] (PaneViewController, e.g. main)
//!         │     └── view = FlippedView (its own Taffy tree)
//!         └── splitViewItem[1] (PaneViewController, e.g. inspector)
//!               └── view = FlippedView (its own Taffy tree)
//! ```
//!
//! ## Auto-Layout vs Taffy
//!
//! `NSSplitViewController` requires Auto-Layout for its panes to
//! size correctly through divider drags, window resizes, and
//! `toggleSidebar:` / `toggleInspector:` animations. That's
//! orthogonal to Taffy: Auto-Layout owns the *outer* frame of each
//! pane (the pane's position+size within the split), and Taffy
//! owns the layout *inside* the pane (the pane's children).
//!
//! We tell Auto-Layout the pane's preferred / minimum / maximum
//! thickness via `NSSplitViewItem` properties. The pane's
//! `FlippedView` has `translatesAutoresizingMaskIntoConstraints =
//! false` so AppKit doesn't try to install conflicting constraints
//! from the frame. Each `PaneViewController` overrides
//! `viewDidLayout` to run Taffy with the post-Auto-Layout
//! `bounds.size` and write child frames using
//! [`compute_layout_children`] (which **skips the root frame** —
//! Auto-Layout owns that).
//!
//! ## Lifecycle
//!
//! - [`open_split_window`] creates the window + `NSSplitViewController`
//!   + N panes. Each pane is returned as a `Pane` handle with its
//!   own [`Element`] root + [`TreeRef`] so callers can mount
//!   leptos views into them.
//! - The window's `contentViewController` is set to the split-view
//!   controller — AppKit then routes responder-chain actions
//!   (`toggleSidebar:`, `toggleInspector:`, menu validation) to it
//!   automatically.
//! - Sidebar / inspector collapse is driven via
//!   [`OpenedSplitWindow::toggle_inspector`] (or the equivalent
//!   for sidebars), which calls the controller's built-in toggle
//!   action — preserves the system's animation curve.

use crate::{
    layout::{self, TreeRef},
    node::{Element, Node},
};
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::NSObject,
    DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSBackingStoreType, NSLayoutConstraint, NSLayoutGuide, NSSplitViewController,
    NSSplitViewItem, NSView, NSViewController, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString};

// ---------------------------------------------------------------------
// PaneBehavior — mirrors NSSplitViewItem.Behavior
// ---------------------------------------------------------------------

/// Standard behavior of a split-view pane — controls its visual
/// material, animation curve, and default constraints.
///
/// - `Default`: no special chrome; behaves like a regular pane.
/// - `Sidebar`: left/source-list style with vibrancy material and
///   the standard sidebar collapse animation. Pair with
///   [`OpenedSplitWindow::toggle_sidebar`].
/// - `ContentList`: a content list (mail-app style middle column).
/// - `Inspector`: a flyout right-side inspector with the standard
///   inspector animation (macOS 11+). Pair with
///   [`OpenedSplitWindow::toggle_inspector`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneBehavior {
    Default,
    Sidebar,
    ContentList,
    Inspector,
}

// PaneBehavior intentionally has no `to_appkit` mapping yet — the
// `init(<behavior>WithViewController:)` constructors set the
// underlying NSSplitViewItem.behavior implicitly. If we later
// need to read the behavior back from the item, add a conversion
// here.

// ---------------------------------------------------------------------
// CollapseBehavior — mirrors NSSplitViewItem.CollapseBehavior
// ---------------------------------------------------------------------

/// What happens to the surrounding layout when this pane toggles
/// its collapsed state. Maps 1:1 to AppKit's
/// `NSSplitViewItem.CollapseBehavior`.
///
/// The two interesting cases:
///
/// - [`Self::PreferResizingSiblingsWithFixedSplitView`] — the
///   **split view (and hence the window) stays the same size**;
///   the OTHER panes grow / shrink to absorb the freed space.
///   This is the "Preview / Notes" feel — sidebar slides in from
///   the left without moving the window.
/// - [`Self::PreferResizingSplitViewWithFixedSiblings`] — the
///   other panes' onscreen positions stay fixed; the **split view
///   (and the window) resizes** to make room. This is the "Mail
///   / Finder" feel where the window grows when the sidebar
///   appears.
///
/// `Default` lets AppKit pick — historically this matches
/// `PreferResizingSplitViewWithFixedSiblings` for sidebar /
/// inspector panes (the window resizes). If you want
/// Preview-style behavior, set
/// `PreferResizingSiblingsWithFixedSplitView` explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapseBehavior {
    /// AppKit-picked default.
    Default,
    /// Keep the split view fixed; resize other panes to absorb.
    /// **The window-stays-put choice.**
    PreferResizingSiblingsWithFixedSplitView,
    /// Keep sibling panes' positions fixed; resize the split
    /// view (and hence the window) instead.
    PreferResizingSplitViewWithFixedSiblings,
    /// Defer to Auto-Layout constraints to determine behavior.
    UseConstraints,
}

impl CollapseBehavior {
    /// Convert to AppKit's `NSSplitViewItemCollapseBehavior`.
    pub fn to_appkit(self) -> objc2_app_kit::NSSplitViewItemCollapseBehavior {
        use objc2_app_kit::NSSplitViewItemCollapseBehavior as B;
        match self {
            Self::Default => B::Default,
            Self::PreferResizingSiblingsWithFixedSplitView => {
                B::PreferResizingSiblingsWithFixedSplitView
            }
            Self::PreferResizingSplitViewWithFixedSiblings => {
                B::PreferResizingSplitViewWithFixedSiblings
            }
            Self::UseConstraints => B::UseConstraints,
        }
    }
}

// ---------------------------------------------------------------------
// PaneSpec — caller-supplied configuration for a single pane
// ---------------------------------------------------------------------

/// Caller-supplied configuration for one pane of an
/// [`OpenedSplitWindow`]. Pass a `Vec<PaneSpec>` to
/// [`open_split_window`]; each one becomes an `NSSplitViewItem`
/// with the requested behavior + sizing constraints.
#[derive(Clone, Debug)]
pub struct PaneSpec {
    pub behavior: PaneBehavior,
    /// Initial collapsed state. The standard inspector / sidebar
    /// behaviors install constraints that make the pane collapsible;
    /// `Default` panes also honor this if `can_collapse` is true.
    pub collapsed: bool,
    /// Whether user interactions (toolbar buttons, divider drag)
    /// can collapse this pane. `Sidebar` / `Inspector` panes
    /// default to true; pass `Some(false)` to lock open.
    pub can_collapse: Option<bool>,
    /// Preferred thickness in points (width for vertical split,
    /// height for horizontal). Sets `preferredThicknessFraction`
    /// relative to the initial split-view width.
    pub preferred_thickness: Option<f64>,
    /// Lower bound. Below this, drag snaps closed (with `canCollapse`)
    /// or stops.
    pub minimum_thickness: Option<f64>,
    /// Upper bound. The pane can't grow past this.
    pub maximum_thickness: Option<f64>,
    /// Auto-Layout holding priority. Lower priority loses width
    /// first when the split view resizes — the typical pattern for
    /// "fixed sidebar / inspector, fluid content" is to set the
    /// content pane to a *lower* priority than the sidebar/
    /// inspector (so the content shrinks first). Apple's sample
    /// uses `199` and `200`; the absolute values don't matter,
    /// only the relative order. `NSLayoutPriorityDefaultLow` is
    /// `250`; `defaultHigh` is `750`; `required` is `1000`.
    pub holding_priority: Option<f32>,
    /// Collapse animation policy. `None` leaves AppKit's default
    /// in place (window resizes on sidebar toggle); set to
    /// [`CollapseBehavior::PreferResizingSiblingsWithFixedSplitView`]
    /// for Preview-style "window stays put."
    pub collapse_behavior: Option<CollapseBehavior>,
}

impl Default for PaneSpec {
    fn default() -> Self {
        Self {
            behavior: PaneBehavior::Default,
            collapsed: false,
            can_collapse: None,
            preferred_thickness: None,
            minimum_thickness: None,
            maximum_thickness: None,
            holding_priority: None,
            collapse_behavior: None,
        }
    }
}

// ---------------------------------------------------------------------
// PaneViewController — per-pane NSViewController
// ---------------------------------------------------------------------

/// Backing state for [`PaneViewController`] — the controller's
/// ivars. The struct itself must be `pub` because `define_class!`
/// exposes it through the controller's class definition, but the
/// fields are crate-private: callers go through the wrapping
/// [`Pane`] handle returned by [`open_split_window`].
pub struct PaneState {
    pub(crate) root: Node,
    #[allow(dead_code)] // retained to keep the Taffy tree alive for the pane's lifetime
    pub(crate) tree: TreeRef,
}

define_class!(
    /// `NSViewController` for one split-view pane. Owns the pane's
    /// FlippedView (Taffy root) as its `.view`, and re-runs Taffy
    /// on every `viewDidLayout` callback — that fires after each
    /// Auto-Layout pass settles the pane's frame, including window
    /// resizes, divider drags, and collapse/expand animations.
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PaneState]
    pub struct PaneViewController;

    impl PaneViewController {
        /// Install our pre-built FlippedView as the controller's
        /// view. NSViewController.loadView is called once on demand
        /// (e.g. when the split-view-item is added to its parent);
        /// we re-install on every call so a future remove / re-add
        /// cycle stays consistent.
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            let state = self.ivars();
            let view = state.root.ns_view();
            unsafe {
                let _: () = msg_send![self, setView: view];
            }
        }

        /// Auto-Layout has just settled this pane's frame. Re-run
        /// Taffy with the pane's bounds size; descendant frames get
        /// updated. The pane's own frame is owned by Auto-Layout
        /// and **not** touched (`compute_layout_children`).
        ///
        /// **Skips zero-sized passes.** Each frame of a collapse
        /// animation fires viewDidLayout with `size.width = 0` (or
        /// height for horizontal splits), and the fully-collapsed
        /// resting state has both at 0. Running Taffy at 0×0 emits
        /// zero-sized frames that get cached and have to be
        /// undone on uncollapse — and we'd waste CPU on a pane the
        /// user isn't looking at anyway.
        #[unsafe(method(viewDidLayout))]
        fn view_did_layout(&self) {
            unsafe {
                let _: () = msg_send![super(self), viewDidLayout];
            }
            let state = self.ivars();
            let size = state.root.ns_view().frame().size;
            if size.width <= 0.0 || size.height <= 0.0 {
                return;
            }
            layout::compute_layout_children(&state.root, size);
        }
    }
);

impl PaneViewController {
    fn new(
        root: Node,
        tree: TreeRef,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let alloc = Self::alloc(mtm).set_ivars(PaneState { root, tree });
        unsafe { msg_send![super(alloc), init] }
    }
}

// ---------------------------------------------------------------------
// Pane handle — what callers mount their views into
// ---------------------------------------------------------------------

/// One pane's mountable roots. Exposed so leptos-cocoa-level
/// builders can call `view_state.mount(&pane.root, None)` to attach
/// their view tree under the pane's FlippedView, and retain `tree`
/// for the pane's lifetime.
pub struct Pane {
    pub root: Element,
    pub tree: TreeRef,
    pub controller: Retained<PaneViewController>,
    pub item: Retained<NSSplitViewItem>,
}

// ---------------------------------------------------------------------
// OpenedSplitWindow — what `open_split_window` returns
// ---------------------------------------------------------------------

pub struct OpenedSplitWindow {
    pub nswindow: Retained<NSWindow>,
    pub split_controller: Retained<NSSplitViewController>,
    /// One [`Pane`] per spec passed to [`open_split_window`], in
    /// the same order.
    pub panes: Vec<Pane>,
}

impl OpenedSplitWindow {
    pub fn show(&self, mtm: MainThreadMarker) {
        self.nswindow.makeKeyAndOrderFront(None);
        let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    }

    /// Collapse / expand the first **inspector** pane. Wraps
    /// `NSSplitViewController.toggleInspector:` (macOS 14+) for the
    /// system's standard animation. **No-op on macOS < 14** (the
    /// selector doesn't exist; we guard with `respondsToSelector:`
    /// rather than letting AppKit raise an exception). Also no-op
    /// if no pane has `PaneBehavior::Inspector`.
    pub fn toggle_inspector(&self) {
        self.perform_optional(objc2::sel!(toggleInspector:));
    }

    /// Collapse / expand the first **sidebar** pane. Wraps
    /// `NSSplitViewController.toggleSidebar:` (macOS 11+). No-op if
    /// no pane has `PaneBehavior::Sidebar`.
    pub fn toggle_sidebar(&self) {
        self.perform_optional(objc2::sel!(toggleSidebar:));
    }

    /// Invoke a no-arg selector on the split controller IF the
    /// controller actually responds to it. Used to call
    /// `toggleInspector:` (macOS 14+) and `toggleSidebar:` (11+)
    /// without crashing on older systems that don't have one or
    /// the other.
    fn perform_optional(&self, sel: objc2::runtime::Sel) {
        let responds: bool = unsafe {
            msg_send![&*self.split_controller, respondsToSelector: sel]
        };
        if !responds {
            return;
        }
        // `performSelector:withObject:` is declared to return `id`
        // (objc type code `@`) — bind the result to a raw pointer so
        // objc2's runtime type-check matches the actual signature.
        // For toggleSidebar: / toggleInspector: the underlying
        // selector returns void, so the id we get is always nil; we
        // intentionally discard it.
        unsafe {
            let _ret: *mut objc2::runtime::AnyObject = msg_send![
                &*self.split_controller,
                performSelector: sel,
                withObject: std::ptr::null::<NSObject>()
            ];
        }
    }

    /// Set the collapsed state of a pane by index. Animates via
    /// the pane's `collapseBehavior` (defaults to the system
    /// behavior for sidebars / inspectors).
    pub fn set_pane_collapsed(&self, index: usize, collapsed: bool) {
        if let Some(pane) = self.panes.get(index) {
            if pane.item.isCollapsed() != collapsed {
                // The animator proxy wraps the setter in an
                // NSAnimationContext so the change animates.
                unsafe {
                    let animator: Retained<NSSplitViewItem> =
                        msg_send![&*pane.item, animator];
                    animator.setCollapsed(collapsed);
                }
            }
        }
    }

    /// Read the collapsed state of a pane by index.
    pub fn is_pane_collapsed(&self, index: usize) -> bool {
        self.panes
            .get(index)
            .map(|p| p.item.isCollapsed())
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------

/// Open a window whose `contentViewController` is an
/// `NSSplitViewController` configured with `panes`. Returns
/// [`OpenedSplitWindow`] — caller mounts their view tree into each
/// `Pane`'s `root` element.
///
/// Each pane:
/// - Has its own [`TreeRef`] (independent Taffy tree).
/// - Has its FlippedView wired as the pane's `viewController.view`,
///   with `translatesAutoresizingMaskIntoConstraints = false` so
///   Auto-Layout owns the outer frame.
/// - Re-runs Taffy on every `viewDidLayout` callback.
pub fn open_split_window(
    title: &str,
    size: (f64, f64),
    vertical: bool,
    specs: Vec<PaneSpec>,
    mtm: MainThreadMarker,
) -> OpenedSplitWindow {
    // Matches `open_window` exactly. Earlier this also set
    // `FullSizeContentView`, which extends the contentView behind
    // the title bar — useful for windows with a real `NSToolbar`
    // integrated into the title bar, but we don't install one. With
    // `FullSizeContentView` and no toolbar, the first ~28pt of the
    // pane content gets hidden under the title bar; pages's toolbar
    // appeared to "render half-clipped at the top" because of it.
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::Miniaturizable;
    let content_rect = NSRect::new(
        NSPoint::new(200.0, 200.0),
        NSSize::new(size.0, size.1),
    );
    let nswindow: Retained<NSWindow> = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            content_rect,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    nswindow.setTitle(&NSString::from_str(title));

    // Build the split-view controller. NSSplitViewController is its
    // own splitView's delegate — we don't override the delegate.
    let split_controller: Retained<NSSplitViewController> = unsafe {
        let alloc = NSSplitViewController::alloc(mtm);
        msg_send![alloc, init]
    };
    split_controller.splitView().setVertical(vertical);

    // Build a pane per spec.
    let mut panes: Vec<Pane> = Vec::with_capacity(specs.len());
    for spec in &specs {
        let pane = build_pane(spec, size, vertical, mtm);
        split_controller.addSplitViewItem(&pane.item);
        panes.push(pane);
    }

    // Window-content wiring. The "obvious" approach
    // (`setContentViewController:` with the split controller)
    // makes `splitController.view` the contentView directly,
    // which means AppKit pins it to the *full* contentView area
    // — including the band the toolbar overlays in
    // FullSizeContentView / unified-toolbar mode. The toolbar
    // then visually clips the top of our content.
    //
    // Instead, install a tiny **container view controller** as
    // the window's `contentViewController`, add the split
    // controller as its child (so the responder chain still
    // routes `toggleSidebar:` to it), then add
    // `splitController.view` as a subview of the container's
    // view and pin its four edges to `window.contentLayoutGuide`
    // via Auto Layout. The layout guide automatically tracks the
    // "non-obscured" portion of the contentView, so the split
    // view follows the safe area as the toolbar toggles or the
    // window resizes — no manual layout-pass plumbing required.
    let target = NSSize::new(size.0, size.1);

    let container_view: Retained<NSView> = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::ZERO, target),
    );

    let container_controller: Retained<NSViewController> = unsafe {
        msg_send![NSViewController::alloc(mtm), init]
    };
    unsafe {
        let _: () = msg_send![&*container_controller, setView: &*container_view];
    }

    // Set preferred content size on the container so AppKit
    // resizes the window to fit at install time.
    container_controller.setPreferredContentSize(target);
    nswindow.setContentViewController(Some(container_controller.as_ref()));
    nswindow.setContentSize(target);
    // Clear preferredContentSize once initial size is locked —
    // leaving it set would pin the contentView at exactly
    // `target` forever, fighting AppKit when the window resizes.
    container_controller.setPreferredContentSize(NSSize::new(0.0, 0.0));

    // Add the split controller as a child of the container so
    // it's in the responder chain. `toggleSidebar:` (sent by
    // `<toolbar_toggle_sidebar/>`) walks the responder chain and
    // NSSplitViewController implements it natively.
    unsafe {
        let _: () = msg_send![
            &*container_controller,
            addChildViewController: &*split_controller
        ];
    }

    // Add splitController.view as a subview of the container view
    // and pin its four edges to `window.contentLayoutGuide`. The
    // guide tracks `contentLayoutRect` — the area NOT obscured by
    // a toolbar in `FullSizeContentView`-style layouts — so the
    // split view (and its panes) automatically inset for the
    // toolbar without us doing any safe-area bookkeeping.
    let split_view_root: Retained<NSView> =
        unsafe { Retained::cast_unchecked(split_controller.view()) };
    split_view_root.setTranslatesAutoresizingMaskIntoConstraints(false);
    container_view.addSubview(&split_view_root);

    if let Some(guide_any) = nswindow.contentLayoutGuide() {
        let guide: Retained<NSLayoutGuide> =
            unsafe { Retained::cast_unchecked(guide_any) };
        let constraints = [
            split_view_root
                .topAnchor()
                .constraintEqualToAnchor(&guide.topAnchor()),
            split_view_root
                .leadingAnchor()
                .constraintEqualToAnchor(&guide.leadingAnchor()),
            split_view_root
                .trailingAnchor()
                .constraintEqualToAnchor(&guide.trailingAnchor()),
            split_view_root
                .bottomAnchor()
                .constraintEqualToAnchor(&guide.bottomAnchor()),
        ];
        let refs: Vec<&NSLayoutConstraint> =
            constraints.iter().map(|c| c.as_ref()).collect();
        let array = NSArray::from_slice(&refs);
        NSLayoutConstraint::activateConstraints(&array);
    } else {
        // Fall back to pinning the split view to the container's
        // bounds via autoresizing if contentLayoutGuide isn't
        // available (very old macOS).
        split_view_root.setTranslatesAutoresizingMaskIntoConstraints(true);
        split_view_root.setFrame(container_view.bounds());
        use objc2_app_kit::NSAutoresizingMaskOptions;
        split_view_root.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
    }

    OpenedSplitWindow {
        nswindow,
        split_controller,
        panes,
    }
}

/// Build one pane: FlippedView + Taffy tree + PaneViewController
/// + NSSplitViewItem with constraints from `spec`.
fn build_pane(
    spec: &PaneSpec,
    window_size: (f64, f64),
    vertical: bool,
    mtm: MainThreadMarker,
) -> Pane {
    // Pane root: a FlippedView with its own Taffy tree.
    let tree = layout::new_tree();
    let root = Element::create_with(&tree, "view", mtm);
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Column);
    layout::register_in_tree(root.as_node(), &tree);

    // **Keep** `translatesAutoresizingMaskIntoConstraints = true`
    // (the default). NSSplitViewController sets each pane's frame
    // directly during layout passes; converting that frame into
    // constraints automatically is exactly what we want.
    // Disabling it forces us to define width / height / position
    // constraints ourselves, which we can't do generically — the
    // split view's internal constraint system already handles
    // sizing via `preferredThicknessFraction` / `minimumThickness`
    // / `maximumThickness` / `holdingPriority` on the `NSSplitViewItem`.

    // Controller wrapping our root view.
    let controller = PaneViewController::new(
        root.as_node().clone(),
        tree.clone(),
        mtm,
    );
    controller.loadViewIfNeeded();

    // Split-view item. Pick the constructor that matches the
    // requested behavior — each preset installs its own default
    // constraints, animations, and (for inspector / sidebar)
    // visual material.
    let item: Retained<NSSplitViewItem> = match spec.behavior {
        PaneBehavior::Default => {
            NSSplitViewItem::splitViewItemWithViewController(
                controller.as_ref(),
            )
        }
        PaneBehavior::Sidebar => {
            NSSplitViewItem::sidebarWithViewController(
                controller.as_ref(),
            )
        }
        PaneBehavior::ContentList => {
            NSSplitViewItem::contentListWithViewController(
                controller.as_ref(),
            )
        }
        PaneBehavior::Inspector => {
            NSSplitViewItem::inspectorWithViewController(
                controller.as_ref(),
            )
        }
    };

    // Apply spec.
    let total = if vertical { window_size.0 } else { window_size.1 };
    if let Some(p) = spec.preferred_thickness {
        if total > 0.0 {
            item.setPreferredThicknessFraction(p / total);
        }
    }
    if let Some(m) = spec.minimum_thickness {
        item.setMinimumThickness(m);
    }
    if let Some(m) = spec.maximum_thickness {
        item.setMaximumThickness(m);
    }
    if let Some(c) = spec.can_collapse {
        item.setCanCollapse(c);
    }
    if let Some(hp) = spec.holding_priority {
        // NSLayoutPriority is a typedef for `c_float`; pass the
        // raw f32 directly.
        item.setHoldingPriority(hp);
    }
    item.setCollapsed(spec.collapsed);
    if let Some(cb) = spec.collapse_behavior {
        item.setCollapseBehavior(cb.to_appkit());
    }

    // No manual constraints — `NSSplitViewItem`'s minimum /
    // maximum / preferred thickness install constraints
    // internally, and `translatesAutoresizingMaskIntoConstraints`
    // (left at default `true`) converts the pane's frame into
    // additional constraints each layout pass. Adding our own
    // here would overconstrain the system.

    Pane { root, tree, controller, item }
}
