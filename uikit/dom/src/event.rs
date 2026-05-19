//! Event handlers — bridging UIKit's target/action and delegate
//! patterns into Rust closures.
//!
//! Design (mirror of `cocoa_dom::event`): `IosNodeHandlers` lives
//! in the arena's `NodeData::handlers` slot, allocated eagerly
//! when the Node is created. Exactly one per node; no Rc sharing
//! between Node clones (clones share an `Rc<NodeInner>` that
//! addresses a single arena entry). When the last clone drops,
//! `NodeInner::Drop` calls `tree.decref(id)`; under the arena
//! removal rule the entry is removed, dropping
//! `NodeData::handlers` and triggering `IosNodeHandlers::Drop`.
//! Drop nils out `setDelegate` / `removeAllTargets` on the view
//! BEFORE releasing the delegate / target retains, so any lingering
//! UIKit retain can't dispatch into freed memory.
//!
//! - `ActionTarget` ObjC class holds a Rust closure as an ivar and
//!   exposes one selector (`actionFired:`) that invokes it. UIControl
//!   supports multiple target/action pairs per event, so the Node's
//!   storage is a `Vec<Retained<ActionTarget>>`.
//! - For UITextField: we use UIControl's `editingChanged`,
//!   `editingDidEnd`, and `editingDidBegin` events via target/action
//!   (simpler than UITextFieldDelegate for these common cases).
//! - For UITextView: we use `UITextViewDelegate::textViewDidChange:`
//!   (UITextView is not a UIControl subclass so target/action
//!   doesn't apply).

use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{NSObject, ProtocolObject, Sel},
    sel, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_ui_kit::{
    UIControl, UIControlEvents, UITapGestureRecognizer, UITextField,
    UITextView, UITextViewDelegate, UIView, UIScrollViewDelegate,
};
use objc2_foundation::NSObjectProtocol;
use send_wrapper::SendWrapper;
use std::{
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering},
};

// ---------------------------------------------------------------------
// Live counts for leak tests.
// ---------------------------------------------------------------------

static LIVE_ACTION_TARGETS: AtomicUsize = AtomicUsize::new(0);
static LIVE_TEXT_VIEW_DELEGATES: AtomicUsize = AtomicUsize::new(0);

/// Sentinel embedded in each handler's ivars. Drop runs as part of
/// the ObjC `dealloc` synthesised by `define_class!`, decrementing
/// the matching counter.
struct LiveTracker(&'static AtomicUsize);

impl LiveTracker {
    fn new(counter: &'static AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self(counter)
    }
}

impl Drop for LiveTracker {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------
// Associated-object helper
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// NodeHandlers — Rust-side retain for everything installed on a Node
// ---------------------------------------------------------------------

/// Per-Node handler/delegate storage. Lives in the arena's
/// `NodeData::handlers` slot, allocated eagerly at Node creation.
///
/// UIControl supports multiple target/action pairs per event mask,
/// so `action_targets` is a `Vec` rather than a single slot.
///
/// The `view` field is a back-reference to the UIView whose target
/// / delegate slots the handlers populated. Set via
/// [`Self::attach_view`]; if `None` the Drop impl is a no-op.
#[derive(Default)]
pub struct IosNodeHandlers {
    view: Option<SendWrapper<Retained<UIView>>>,
    pub(crate) action_targets: Vec<Retained<ActionTarget>>,
    /// **Released explicitly in `Drop` BEFORE `disconnect_view_handlers`
    /// runs** — same workaround as cocoa's text-view delegate; see
    /// the Drop impl for the rationale.
    pub(crate) text_view_delegate: Option<Retained<TextViewDelegate>>,
    /// Gesture-recognizer targets installed by `on_tap_gesture`.
    pub(crate) gesture_targets: Vec<Retained<ActionTarget>>,
}

impl IosNodeHandlers {
    /// Register the UIView whose target/delegate slots this struct
    /// populated. Drop will nil those slots before releasing the
    /// retain fields. Idempotent: a second call is a no-op.
    pub fn attach_view(&mut self, view: Retained<UIView>) {
        if self.view.is_none() {
            self.view = Some(SendWrapper::new(view));
        }
    }
}

impl Drop for IosNodeHandlers {
    fn drop(&mut self) {
        // Drop the UITextView delegate explicitly BEFORE
        // disconnect_view_handlers calls setDelegate(None). See the
        // cocoa equivalent (cocoa/dom/src/event.rs NodeHandlers::drop)
        // for the long-form rationale — empirically the delegate
        // ends up at retainCount=1 if released after the disconnect
        // call clears the view's slot.
        let _tv = self.text_view_delegate.take();
        drop(_tv);

        let Some(view) = self.view.as_ref() else {
            return;
        };
        if !view.valid() {
            // Off-main drop — can't touch UIKit. Leak rather than
            // abort.
            return;
        }
        disconnect_view_handlers(view);
        // Field-drop order then releases the remaining ActionTarget
        // / gesture-target retains.
    }
}

/// Nil out `setDelegate` and clear all target/action pairs on
/// `view` so any lingering UIKit retain can't dispatch into freed
/// handler memory after the owning `IosNodeHandlers` drops.
/// Idempotent.
pub fn disconnect_view_handlers(view: &UIView) {
    let any: &objc2::runtime::AnyObject = view.as_ref();
    if let Some(control) = any.downcast_ref::<UIControl>() {
        // Pass nil target with ALL events to remove every installed
        // target/action pair this control has.
        unsafe {
            control.removeTarget_action_forControlEvents(
                None,
                None,
                UIControlEvents::all(),
            );
        }
    }
    if let Some(tv) = any.downcast_ref::<UITextView>() {
        unsafe { tv.setDelegate(None) };
    }
    // Gesture recognizers: UIView retains its recognizer list. We
    // could iterate and `removeGestureRecognizer` each, but the
    // recognizers hold their targets weakly and the view itself
    // dies shortly after this bundle drops — leaving them attached
    // is harmless.
}

// ---------------------------------------------------------------------
// ActionTarget — shared ObjC class for UIControl target/action
// ---------------------------------------------------------------------

type Callback = RefCell<Box<dyn FnMut() + 'static>>;

/// Bundle of ivars stored on each ActionTarget: the closure plus a
/// LiveTracker for leak tests.
pub struct ActionIvars {
    callback: Callback,
    _live: LiveTracker,
}

define_class!(
    /// ObjC class that holds one Rust closure and exposes one
    /// selector, `actionFired:`, that invokes it. Used as the
    /// target of UIControl target/action wiring.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ActionIvars]
    pub struct ActionTarget;

    impl ActionTarget {
        #[unsafe(method(actionFired:))]
        fn action_fired(&self, _sender: *mut NSObject) {
            let mut cb = match self.ivars().callback.try_borrow_mut() {
                Ok(cb) => cb,
                Err(_) => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[ios_dom] reentrant action handler call skipped"
                    );
                    return;
                }
            };
            (cb)();
        }
    }
);

impl ActionTarget {
    pub fn new(
        cb: impl FnMut() + 'static,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let alloc = Self::alloc(mtm);
        let this = alloc.set_ivars(ActionIvars {
            callback: RefCell::new(Box::new(cb)),
            _live: LiveTracker::new(&LIVE_ACTION_TARGETS),
        });
        unsafe { msg_send![super(this), init] }
    }
}

pub fn action_fired_sel() -> Sel {
    sel!(actionFired:)
}


// ---------------------------------------------------------------------
// Test-only introspection (mirror cocoa's *_for_test functions).
// ---------------------------------------------------------------------

/// Test-only: live ActionTargets (created minus dropped).
#[doc(hidden)]
pub fn handler_store_size_for_test() -> usize {
    LIVE_ACTION_TARGETS.load(Ordering::Relaxed)
}

/// Test-only: live TextViewDelegates.
#[doc(hidden)]
pub fn text_view_store_size_for_test() -> usize {
    LIVE_TEXT_VIEW_DELEGATES.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------
// On-control-action: UIControl target/action
// ---------------------------------------------------------------------

/// Variant of [`on_control_action`] that lets the caller specify
/// which UIControlEvents to listen for. The handler always takes
/// `()`. If you need the control's value, read it inside the closure.
fn on_control_action_with_events(
    node: &crate::Node,
    events: objc2_ui_kit::UIControlEvents,
    cb: impl FnMut() + 'static,
) {
    let view = node.ui_view();
    let Some(control) = crate::node::downcast::<UIControl>(view) else {
        return;
    };
    let mtm = MainThreadMarker::new()
        .expect("on_control_action must run on the main thread");

    let target = ActionTarget::new(cb, mtm);
    let target_obj: &NSObject = &target;
    unsafe {
        control.addTarget_action_forControlEvents(
            Some(target_obj),
            action_fired_sel(),
            events,
        );
    }
    let view_retained = node.ui_view_retained();
    node.with_handlers_mut(|h| {
        h.attach_view(view_retained);
        h.action_targets.push(target);
    });
}

/// Wire the given closure to fire when a UIControl's primary action
/// fires — `TouchUpInside` for UIButton,
/// `ValueChanged` for UISlider/UISwitch/UISegmentedControl/
/// UIDatePicker/UIStepper.
///
/// We detect the control class and pick the appropriate event mask.
/// **Single handler per control per event** — duplicates on the
/// same control+event pair are allowed (UIKit supports multiple
/// targets), but the same restriction as the macOS port applies:
/// consecutive calls to `on_control_action` will each add a
/// separate handler. This is intentional — unlike macOS NSControl's
/// single target/action slot, UIControl supports multiple target/
/// action pairs.
pub fn on_control_action(
    node: &crate::Node,
    cb: impl FnMut() + 'static,
) {
    let view = node.ui_view();
    let any: &objc2::runtime::AnyObject = view.as_ref();
    // No-op if not a UIControl — keeps callers from having to check
    // first.
    if any.downcast_ref::<UIControl>().is_none() {
        return;
    }
    let events = if any.downcast_ref::<objc2_ui_kit::UIButton>().is_some() {
        objc2_ui_kit::UIControlEvents::TouchUpInside
    } else if any.downcast_ref::<objc2_ui_kit::UISlider>().is_some()
        || any.downcast_ref::<objc2_ui_kit::UISwitch>().is_some()
        || any.downcast_ref::<objc2_ui_kit::UISegmentedControl>().is_some()
        || any.downcast_ref::<objc2_ui_kit::UIDatePicker>().is_some()
        || any.downcast_ref::<objc2_ui_kit::UIStepper>().is_some()
        || any.downcast_ref::<objc2_ui_kit::UIColorWell>().is_some()
    {
        objc2_ui_kit::UIControlEvents::ValueChanged
    } else {
        // Default: ValueChanged + TouchUpInside (generic controls)
        objc2_ui_kit::UIControlEvents::ValueChanged
    };
    on_control_action_with_events(node, events, cb);
}

// ---------------------------------------------------------------------
// Tap-gesture path: lets `<view>` / `<label>` / `<image_view>` / etc.
// react to taps even though they aren't UIControls. Used by
// `Element::on_click` as a fallback when the view isn't a UIControl.
// ---------------------------------------------------------------------

/// Install a `UITapGestureRecognizer` on `view` that calls `cb` on
/// each recognised tap. The recognizer is retained by the view (via
/// `addGestureRecognizer:`), and the `ActionTarget` is stashed in
/// the per-view handler store so it lives until `teardown`.
///
/// `userInteractionEnabled` is forced to `true` because
/// `UILabel` and `UIImageView` default to `NO` — a gesture
/// recognizer attached to either silently never fires unless
/// user-interaction is explicitly turned on.
pub fn on_tap_gesture(node: &crate::Node, cb: impl FnMut() + 'static) {
    let view = node.ui_view();
    let mtm = MainThreadMarker::new()
        .expect("on_tap_gesture must run on the main thread");
    if !view.isUserInteractionEnabled() {
        view.setUserInteractionEnabled(true);
    }
    let target = ActionTarget::new(cb, mtm);
    let target_obj: &NSObject = &target;
    let recognizer = unsafe {
        UITapGestureRecognizer::initWithTarget_action(
            UITapGestureRecognizer::alloc(mtm),
            Some(target_obj),
            Some(action_fired_sel()),
        )
    };
    view.addGestureRecognizer(&recognizer);
    // The recognizer holds its target weakly; keep the
    // ActionTarget alive via the node's handler storage. Drop of
    // the node's bundle clears these via field drop order; the
    // recognizer itself dies with the view shortly after.
    let view_retained = node.ui_view_retained();
    node.with_handlers_mut(|h| {
        h.attach_view(view_retained);
        h.gesture_targets.push(target);
    });
}

// ---------------------------------------------------------------------
// UITextField events via UIControl editing events
// ---------------------------------------------------------------------

/// Append an input observer on a UITextField (fires on every
/// keystroke / paste). Uses `UIControlEventEditingChanged`.
/// Captures the field via `Retained<UITextField>` (no Element
/// capture — avoids the cycle described in the module docs).
/// No-op if `node` isn't a UITextField.
pub fn on_text_field_change(
    node: &crate::Node,
    mut cb: impl FnMut(String) + 'static,
) {
    let Some(field) =
        crate::node::downcast::<UITextField>(node.ui_view())
    else { return };
    let field_clone: Retained<UITextField> = field.into();
    on_control_action_with_events(
        node,
        objc2_ui_kit::UIControlEvents::EditingChanged,
        move || {
            let value: String = field_clone.text()
                .map(|s| s.to_string())
                .unwrap_or_default();
            cb(value);
        },
    );
}

/// Append a commit observer on a UITextField (fires on Return key /
/// focus loss). Uses `UIControlEventEditingDidEnd`.
pub fn on_text_field_end_editing(
    node: &crate::Node,
    mut cb: impl FnMut(String) + 'static,
) {
    let Some(field) =
        crate::node::downcast::<UITextField>(node.ui_view())
    else { return };
    let field_clone: Retained<UITextField> = field.into();
    on_control_action_with_events(
        node,
        objc2_ui_kit::UIControlEvents::EditingDidEnd,
        move || {
            let value: String = field_clone.text()
                .map(|s| s.to_string())
                .unwrap_or_default();
            cb(value);
        },
    );
}

/// Append a focus observer on a UITextField (fires on
/// `UIControlEventEditingDidBegin`).
pub fn on_text_field_focus(
    node: &crate::Node,
    cb: impl FnMut() + 'static,
) {
    if crate::node::downcast::<UITextField>(node.ui_view()).is_none() {
        return;
    }
    on_control_action_with_events(
        node,
        objc2_ui_kit::UIControlEvents::EditingDidBegin,
        cb,
    );
}

/// Append a blur observer on a UITextField (fires on
/// `UIControlEventEditingDidEnd` — same as end editing but without
/// the value payload).
pub fn on_text_field_blur(
    node: &crate::Node,
    cb: impl FnMut() + 'static,
) {
    if crate::node::downcast::<UITextField>(node.ui_view()).is_none() {
        return;
    }
    on_control_action_with_events(
        node,
        objc2_ui_kit::UIControlEvents::EditingDidEnd,
        cb,
    );
}

// ---------------------------------------------------------------------
// UITextView delegate (multi-line text — `bind:value`)
// ---------------------------------------------------------------------

#[derive(Default)]
pub struct TextViewHandlers {
    on_change: Vec<Box<dyn FnMut(String) + 'static>>,
}

type SharedTextViewHandlers = std::rc::Rc<RefCell<TextViewHandlers>>;

/// Bundle of ivars on each TextViewDelegate: the shared handler
/// state plus a LiveTracker for leak tests.
pub struct TextViewIvars {
    handlers: SharedTextViewHandlers,
    _live: LiveTracker,
}

define_class!(
    /// UITextView delegate that fans `textViewDidChange:` notifications
    /// out to all installed callbacks.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TextViewIvars]
    pub struct TextViewDelegate;

    unsafe impl NSObjectProtocol for TextViewDelegate {}

    unsafe impl UIScrollViewDelegate for TextViewDelegate {}

    unsafe impl UITextViewDelegate for TextViewDelegate {
        #[unsafe(method(textViewDidChange:))]
        fn text_view_did_change(&self, text_view: &UITextView) {
            let value: String = text_view.text().to_string();
            let mut handlers = match self.ivars().handlers.try_borrow_mut() {
                Ok(h) => h,
                Err(_) => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[ios_dom] reentrant textViewDidChange skipped"
                    );
                    return;
                }
            };
            for cb in handlers.on_change.iter_mut() {
                cb(value.clone());
            }
        }
    }
);

impl TextViewDelegate {
    fn new(
        handlers: SharedTextViewHandlers,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let alloc = Self::alloc(mtm);
        let this = alloc.set_ivars(TextViewIvars {
            handlers,
            _live: LiveTracker::new(&LIVE_TEXT_VIEW_DELEGATES),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// Look up (or lazily create) the per-text-view handler state on
/// the node's [`IosNodeHandlers`]. Repeated installs reuse the
/// existing delegate's `SharedHandlers`.
fn ensure_text_view_entry(
    node: &crate::Node,
) -> SharedTextViewHandlers {
    let tv = crate::node::downcast::<UITextView>(node.ui_view())
        .expect("ensure_text_view_entry: node is not a UITextView");
    let mtm = MainThreadMarker::new()
        .expect("text-view event installs must run on the main thread");

    // Fast path: reuse existing delegate if installed.
    if let Some(existing) = node.with_handlers_mut(|h| {
        h.text_view_delegate
            .as_ref()
            .map(|d| d.ivars().handlers.clone())
    }) {
        return existing;
    }
    let handlers: SharedTextViewHandlers = Default::default();
    // Wrap delegate creation + setDelegate in a tight
    // autoreleasepool. `UIText…setDelegate:` (and the
    // sibling AppKit path on cocoa) internally autoreleases an
    // extra retain on the delegate the first time it sets up the
    // text-system shared state for a process scope. Without an
    // immediate drain, the first-ever text_view's delegate stays
    // alive past `IosNodeHandlers::Drop` for the lifetime of
    // the outer autoreleasepool — surfacing as a deferred leak in
    // unit tests / fuzzers even though every Rust-side reference
    // has been dropped. See the matching block in
    // `cocoa/dom/src/event.rs::ensure_text_view_entry` for the
    // full history.
    let delegate = objc2::rc::autoreleasepool(|_| {
        let d = TextViewDelegate::new(handlers.clone(), mtm);
        let proto: &ProtocolObject<dyn UITextViewDelegate> =
            ProtocolObject::from_ref(&*d);
        unsafe { tv.setDelegate(Some(proto)) };
        d
    });
    let view_retained = node.ui_view_retained();
    node.with_handlers_mut(|h| {
        h.attach_view(view_retained);
        h.text_view_delegate = Some(delegate);
    });
    handlers
}

/// Append a change observer on a UITextView — fires on every
/// keystroke. No-op if `node` isn't a UITextView.
pub fn on_text_view_change(
    node: &crate::Node,
    cb: impl FnMut(String) + 'static,
) {
    if crate::node::downcast::<UITextView>(node.ui_view()).is_none() {
        return;
    }
    let handlers = ensure_text_view_entry(node);
    handlers.borrow_mut().on_change.push(Box::new(cb));
}
