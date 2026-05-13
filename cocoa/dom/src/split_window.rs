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
    NSBackingStoreType, NSLayoutAttribute, NSLayoutConstraint, NSSplitViewController,
    NSSplitViewItem, NSSplitViewItemBehavior, NSViewController, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

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

impl PaneBehavior {
    /// Map to the AppKit enum. `init(<behavior>WithViewController:)`
    /// constructors set this implicitly; the conversion exists so
    /// callers reading `NSSplitViewItem.behavior` can compare
    /// against our enum.
    #[allow(dead_code)]
    pub(crate) fn to_appkit(self) -> NSSplitViewItemBehavior {
        match self {
            Self::Default     => NSSplitViewItemBehavior::Default,
            Self::Sidebar     => NSSplitViewItemBehavior::Sidebar,
            Self::ContentList => NSSplitViewItemBehavior::ContentList,
            Self::Inspector   => NSSplitViewItemBehavior::Inspector,
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
    /// first when the split view resizes — Apple recommends the
    /// content pane be `defaultLow - 1` (199) and sidebars be
    /// `defaultLow` (200) for the "fixed sidebar, fluid content"
    /// shape.
    pub holding_priority: Option<f32>,
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
        }
    }
}

// ---------------------------------------------------------------------
// PaneViewController — per-pane NSViewController
// ---------------------------------------------------------------------

pub struct PaneState {
    pub root: Node,
    pub tree: TreeRef,
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
        #[unsafe(method(viewDidLayout))]
        fn view_did_layout(&self) {
            unsafe {
                let _: () = msg_send![super(self), viewDidLayout];
            }
            let state = self.ivars();
            let size = state.root.ns_view().frame().size;
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
    /// system's standard animation. No-op if no pane has
    /// `PaneBehavior::Inspector`.
    pub fn toggle_inspector(&self) {
        let sel = objc2::sel!(toggleInspector:);
        let _: () = unsafe {
            msg_send![&*self.split_controller, performSelector: sel, withObject: std::ptr::null::<NSObject>()]
        };
    }

    /// Collapse / expand the first **sidebar** pane. Wraps
    /// `NSSplitViewController.toggleSidebar:`. No-op if no pane has
    /// `PaneBehavior::Sidebar`.
    pub fn toggle_sidebar(&self) {
        let sel = objc2::sel!(toggleSidebar:);
        let _: () = unsafe {
            msg_send![&*self.split_controller, performSelector: sel, withObject: std::ptr::null::<NSObject>()]
        };
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
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::FullSizeContentView;
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

    // `setContentViewController:` makes the window track the
    // controller's fitting size. With no intrinsic content yet, the
    // window collapses to (0, 0). Set a preferred size on the
    // controller (matches our requested window size) so the
    // initial layout pass has a definite target — once content is
    // mounted the user can resize freely.
    split_controller.setPreferredContentSize(NSSize::new(size.0, size.1));
    nswindow.setContentViewController(Some(
        split_controller.as_ref(),
    ));
    // Re-impose the requested window size; setContentViewController
    // sometimes shrinks the window to the controller's fitting size
    // before the preferred-content-size constraint can take effect.
    nswindow.setContentSize(NSSize::new(size.0, size.1));

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
    let root = Element::create_with("view", mtm);
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Column);
    let tree = layout::new_tree();
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

    // No manual constraints — `NSSplitViewItem`'s minimum /
    // maximum / preferred thickness install constraints
    // internally, and `translatesAutoresizingMaskIntoConstraints`
    // (left at default `true`) converts the pane's frame into
    // additional constraints each layout pass. Adding our own
    // here would overconstrain the system.

    Pane { root, tree, controller, item }
}
