//! Event handlers — bridging AppKit's target/action and delegate
//! patterns into Rust closures.
//!
//! Design: a small `ActionTarget` ObjC class holds a Rust closure
//! as an ivar and exposes one selector (`actionFired:`) that
//! invokes it. We create one per registered handler, wire the
//! AppKit control's `target` / `action` to point at it, and store
//! the `Retained<ActionTarget>` in **`NodeHandlers`** — a small
//! Rust struct that lives as a shared field on every [`Node`]
//! ([`Node::handlers`]).
//!
//! Same pattern for `TextFieldDelegate` and `TextViewDelegate`.
//!
//! There is **no global storage** and no ObjC associated-object
//! sidetable: handler lifetime tracks Rust ownership. When the
//! last clone of the `Node` drops, `NodeHandlers` drops, every
//! retained `ActionTarget` / delegate dealloc runs, and the Rust
//! closures release. The `Node::Drop` impl additionally nils out
//! `setTarget` / `setDelegate` on the view before the handlers
//! release, so any lingering AppKit retain can't dispatch into
//! freed memory.
//!
//! Why this matters: AppKit retains views in ways outside our
//! control (autorelease pools, undo manager, focus chain, …). If
//! handler lifetime were tied to NSView lifetime (e.g. associated
//! objects, or a sidetable keyed by NSView pointer), those
//! lingering retains would keep our closures alive too —
//! observable as a slow leak across mount/unmount cycles. Tying
//! handlers to the Rust `Node` decouples them from AppKit's view
//! lifecycle entirely.

use crate::KeyEvent;
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObject, ProtocolObject, Sel},
    sel, DefinedClass, MainThreadMarker, MainThreadOnly, Message,
};
use objc2_app_kit::{
    NSControl, NSControlTextEditingDelegate, NSTextDelegate, NSTextField,
    NSTextFieldDelegate, NSTextView, NSTextViewDelegate, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol};
use send_wrapper::SendWrapper;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

// ---------------------------------------------------------------------
// Live counts for leak tests.
//
// Every ActionTarget / TextFieldDelegate / TextViewDelegate bumps its
// counter in `new()` and decrements in `Drop`. Tests can read these
// to verify that mount/unmount cycles don't leave retained handlers
// behind. Production code never reads them.
// ---------------------------------------------------------------------

static LIVE_ACTION_TARGETS: AtomicUsize = AtomicUsize::new(0);
static LIVE_TEXT_FIELD_DELEGATES: AtomicUsize = AtomicUsize::new(0);
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
// NodeHandlers — Rust-side retain for everything installed on a Node
// ---------------------------------------------------------------------

/// Per-Node handler/delegate storage. Held as
/// `SendWrapper<Rc<RefCell<NodeHandlers>>>` on [`crate::Node`] so
/// every clone of a Node sees the same handler set, and the
/// retains release when the last clone drops.
///
/// Each `Retained<...>` here keeps its ObjC object alive while
/// `NodeHandlers` is alive. The view's `setTarget` / `setDelegate`
/// slots point at these objects; `Node::Drop` clears those slots
/// (via [`disconnect_view_handlers`]) before this struct drops, so
/// any lingering AppKit retain on the view can't dispatch into
/// freed memory.
#[derive(Default)]
pub struct NodeHandlers {
    /// At most one ActionTarget — NSControl has a single
    /// target/action slot. Set by [`on_control_action`].
    pub(crate) action_target: Option<Retained<ActionTarget>>,
    /// At most one TextFieldDelegate per text-field-backed node.
    /// Created lazily on the first `on_text_field_*` install.
    pub(crate) text_field_delegate: Option<Retained<TextFieldDelegate>>,
    /// At most one TextViewDelegate per text-view-backed node.
    pub(crate) text_view_delegate: Option<Retained<TextViewDelegate>>,
    // NB: no gesture-recognizer slot yet — the cocoa port currently
    // doesn't expose tap-on-non-control. When `<vstack on:click>`
    // lands (NSClickGestureRecognizer), add a
    // `gesture_targets: Vec<Retained<ActionTarget>>` here, mirroring
    // the iOS `IosNodeHandlers` shape.
}

/// Wraps [`NodeHandlers`] together with a `Retained<NSView>`
/// back-reference. The Drop impl runs when the last clone of the
/// owning Node drops — it nils out `setTarget` / `setDelegate` on
/// the view BEFORE the handler retains release, so a lingering
/// AppKit retain on the NSView (autorelease pool, undo manager,
/// focus chain, etc.) can't dispatch into freed ActionTarget memory
/// between the field-drop and the eventual NSView dealloc.
///
/// Holding a `Retained<NSView>` here adds one extra refcount bump
/// per logical NSView — negligible — and ensures the view is still
/// alive at the moment we send `setTarget(None)`.
pub struct NodeHandlersBundle {
    view: SendWrapper<Retained<NSView>>,
    handlers: RefCell<NodeHandlers>,
}

impl NodeHandlersBundle {
    pub fn new_shared(view: Retained<NSView>) -> Rc<NodeHandlersBundle> {
        Rc::new(NodeHandlersBundle {
            view: SendWrapper::new(view),
            handlers: RefCell::new(NodeHandlers::default()),
        })
    }

    pub fn handlers(&self) -> &RefCell<NodeHandlers> {
        &self.handlers
    }
}

impl Drop for NodeHandlersBundle {
    fn drop(&mut self) {
        if !self.view.valid() {
            // Off-main drop — can't touch AppKit. Leak the handlers
            // rather than abort.
            return;
        }
        disconnect_view_handlers(&self.view);
        // The handlers RefCell drops next (field-drop order),
        // releasing every Retained<ActionTarget> / delegate. The
        // view's target / delegate slots are nil now, so no
        // dispatch can hit the freed closures even if AppKit holds
        // the NSView past this point.
    }
}

/// Nil out `setTarget` / `setDelegate` on `view` so any lingering
/// AppKit retain can't dispatch into freed handler memory after
/// the owning `NodeHandlers` drops. Idempotent; safe to call
/// multiple times.
///
/// Called from `Node::Drop` when the last clone is about to drop
/// the handlers. Cheap — the downcasts short-circuit to `None`
/// for views that aren't controls / text-fields / text-views.
pub fn disconnect_view_handlers(view: &NSView) {
    let any: &AnyObject = view.as_ref();
    if let Some(control) = any.downcast_ref::<NSControl>() {
        unsafe {
            control.setTarget(None);
            control.setAction(None);
        }
    }
    if let Some(field) = any.downcast_ref::<NSTextField>() {
        unsafe { field.setDelegate(None) };
    }
    if let Some(tv) = any.downcast_ref::<NSTextView>() {
        tv.setDelegate(None);
    }
    // `<text_view>` wraps an NSTextView inside an NSScrollView; the
    // delegate is set on the inner documentView, not on the scroll
    // wrapper. Nil it there too, otherwise an autoreleased scroll
    // view holding the documentView could dispatch into a freed
    // TextViewDelegate.
    if let Some(scroll) = any.downcast_ref::<objc2_app_kit::NSScrollView>() {
        if let Some(doc) = scroll.documentView() {
            let inner: &AnyObject = (&*doc).as_ref();
            if let Some(tv) = inner.downcast_ref::<NSTextView>() {
                tv.setDelegate(None);
            }
        }
    }
    // Gesture recognizers: NSView retains its recognizer list,
    // and each recognizer holds its target weakly. removeFromSuperview
    // (called in Node::teardown) detaches the view from the responder
    // chain, so recognizers can't fire post-teardown. No explicit
    // cleanup needed here.
}

/// The closure carried by [`ActionTarget`]. One per NSControl —
/// see `on_control_action`'s docstring for why we panic on
/// duplicate installs rather than fan out.
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
    /// target of NSControl target/action wiring.
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
                    // Re-entrance: a click handler synchronously
                    // triggered another click on the same control.
                    // Skip rather than panic.
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[cocoa_dom] reentrant click handler call \
                         skipped"
                    );
                    return;
                }
            };
            (cb)();
        }
    }
);

impl ActionTarget {
    /// Create a new ActionTarget holding the given closure.
    pub fn new(
        cb: impl FnMut() + 'static,
        mtm: objc2::MainThreadMarker,
    ) -> Retained<Self> {
        let alloc = Self::alloc(mtm);
        let this = alloc.set_ivars(ActionIvars {
            callback: RefCell::new(Box::new(cb)),
            _live: LiveTracker::new(&LIVE_ACTION_TARGETS),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// Selector matching `ActionTarget::action_fired`. Cached for cheap
/// lookups when wiring up controls.
pub fn action_fired_sel() -> Sel {
    sel!(actionFired:)
}

// ---------------------------------------------------------------------
// Test-only introspection
// ---------------------------------------------------------------------

/// Test-only: live ActionTargets (created minus dropped). Returns
/// to zero when every owning `NodeHandlers` has dropped.
#[doc(hidden)]
pub fn handler_store_size_for_test() -> usize {
    LIVE_ACTION_TARGETS.load(Ordering::Relaxed)
}

/// Test-only: live TextFieldDelegates.
#[doc(hidden)]
pub fn text_field_store_size_for_test() -> usize {
    LIVE_TEXT_FIELD_DELEGATES.load(Ordering::Relaxed)
}

/// Test-only: live TextViewDelegates.
#[doc(hidden)]
pub fn text_view_store_size_for_test() -> usize {
    LIVE_TEXT_VIEW_DELEGATES.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------
// Wiring helpers — call these from the higher layers (cocoa_dom::Element
// methods) to attach a handler to a control.
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Text-field event observer (input + change)
// ---------------------------------------------------------------------
//
// AppKit's text-field event story routes through NSTextFieldDelegate /
// NSControlTextEditingDelegate. We need to observe two things:
//   * `controlTextDidChange:`     — fires on every keystroke (web
//                                    `input` event, also drives
//                                    `bind:value` write-back)
//   * `controlTextDidEndEditing:` — fires on commit (return key,
//                                    focus loss; web `change` event)
//
// AppKit only stores one delegate pointer per NSTextField, so we use
// a single delegate class that carries Vecs of callbacks for each
// event. New handlers are appended; the delegate fans them out.
// This lets `bind:value`, `on:input`, and `on:change` coexist on the
// same field (each just adds another callback).

/// Per-field handler state, shared between the delegate (which fires
/// callbacks) and the install helpers (which append to the Vecs).
/// Wrapped in `Rc<RefCell<...>>` so install can mutate after
/// the delegate is constructed.
#[derive(Default)]
pub struct TextFieldHandlers {
    on_input: Vec<Box<dyn FnMut(String) + 'static>>,
    on_change: Vec<Box<dyn FnMut(String) + 'static>>,
    /// Fires on `controlTextDidBeginEditing:` (web `focus`).
    on_focus: Vec<Box<dyn FnMut() + 'static>>,
    /// Fires on `controlTextDidEndEditing:` alongside `on_change`.
    /// Web semantics: blur fires on focus loss (which on AppKit is
    /// synonymous with end-of-editing). The difference from `change`
    /// is just the payload — blur has none.
    on_blur: Vec<Box<dyn FnMut() + 'static>>,
    /// Fires on `control:textView:doCommandBySelector:` for
    /// recognized command keys (Enter, Escape, Tab, arrows). The
    /// AppKit pipeline doesn't distinguish keydown from keyup —
    /// both Vecs fire on the same notification.
    on_keydown: Vec<Box<dyn FnMut(KeyEvent) + 'static>>,
    on_keyup: Vec<Box<dyn FnMut(KeyEvent) + 'static>>,
}

type SharedHandlers = std::rc::Rc<RefCell<TextFieldHandlers>>;

/// Bundle of ivars on each TextFieldDelegate: the shared handler
/// state plus a LiveTracker for leak tests.
pub struct TextFieldIvars {
    handlers: SharedHandlers,
    _live: LiveTracker,
}

define_class!(
    /// ObjC class that observes text-field input (`controlTextDidChange:`)
    /// and commit (`controlTextDidEndEditing:`), fanning each event out
    /// to all installed callbacks for the field.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TextFieldIvars]
    pub struct TextFieldDelegate;

    unsafe impl NSObjectProtocol for TextFieldDelegate {}

    unsafe impl NSControlTextEditingDelegate for TextFieldDelegate {
        #[unsafe(method(controlTextDidBeginEditing:))]
        fn control_text_did_begin_editing(
            &self,
            _notification: &NSNotification,
        ) {
            let mut handlers = match self.ivars().handlers.try_borrow_mut() {
                Ok(h) => h,
                Err(_) => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[cocoa_dom] reentrant controlTextDidBeginEditing skipped"
                    );
                    return;
                }
            };
            for cb in handlers.on_focus.iter_mut() {
                cb();
            }
        }

        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, notification: &NSNotification) {
            let object = notification.object();
            let Some(object) = object else { return };
            let any: &objc2::runtime::AnyObject = &*object;
            let Some(field) = any.downcast_ref::<NSTextField>() else {
                return;
            };
            let value: String = field.stringValue().to_string();
            // RefCell guard: a callback that synchronously triggers
            // another text change on this same field would re-enter
            // here; skip rather than panic.
            let mut handlers = match self.ivars().handlers.try_borrow_mut() {
                Ok(h) => h,
                Err(_) => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[cocoa_dom] reentrant controlTextDidChange skipped"
                    );
                    return;
                }
            };
            for cb in handlers.on_input.iter_mut() {
                cb(value.clone());
            }
        }

        /// AppKit calls this when the field editor sees a "command
        /// key" (Return, Escape, Tab, arrow keys, etc.). Each key
        /// maps to a named selector — we translate that selector to
        /// a [`KeyEvent`] and fan out to both `on:keydown` and
        /// `on:keyup` handlers (AppKit doesn't separate down/up at
        /// this layer).
        ///
        /// Return `false` so AppKit performs the default action
        /// (e.g. Return commits the field, Tab moves focus). Users
        /// who want to suppress the default should return `true`
        /// from their handler — but our current shape doesn't
        /// thread a return back, since web semantics are
        /// "preventDefault is explicit". Adequate for all known
        /// callers.
        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn control_text_view_do_command(
            &self,
            _control: &NSControl,
            _text_view: &NSTextView,
            command: Sel,
        ) -> Bool {
            let Some(event) = KeyEvent::from_command_selector(command)
            else {
                return Bool::NO;
            };
            let mut handlers = match self.ivars().handlers.try_borrow_mut() {
                Ok(h) => h,
                Err(_) => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[cocoa_dom] reentrant doCommandBySelector skipped"
                    );
                    return Bool::NO;
                }
            };
            for cb in handlers.on_keydown.iter_mut() {
                cb(event.clone());
            }
            for cb in handlers.on_keyup.iter_mut() {
                cb(event.clone());
            }
            Bool::NO
        }

        #[unsafe(method(controlTextDidEndEditing:))]
        fn control_text_did_end_editing(
            &self,
            notification: &NSNotification,
        ) {
            let object = notification.object();
            let Some(object) = object else { return };
            let any: &objc2::runtime::AnyObject = &*object;
            let Some(field) = any.downcast_ref::<NSTextField>() else {
                return;
            };
            let value: String = field.stringValue().to_string();
            let mut handlers = match self.ivars().handlers.try_borrow_mut() {
                Ok(h) => h,
                Err(_) => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[cocoa_dom] reentrant controlTextDidEndEditing \
                         skipped"
                    );
                    return;
                }
            };
            for cb in handlers.on_change.iter_mut() {
                cb(value.clone());
            }
            for cb in handlers.on_blur.iter_mut() {
                cb();
            }
        }
    }

    unsafe impl NSTextFieldDelegate for TextFieldDelegate {}
);

impl TextFieldDelegate {
    fn new(handlers: SharedHandlers, mtm: MainThreadMarker) -> Retained<Self> {
        let alloc = Self::alloc(mtm);
        let this = alloc.set_ivars(TextFieldIvars {
            handlers,
            _live: LiveTracker::new(&LIVE_TEXT_FIELD_DELEGATES),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// Look up (or lazily create) the per-field handler state on the
/// node's [`NodeHandlers`]. Repeated installs on the same node
/// reuse the existing delegate's `SharedHandlers`, so callbacks
/// of all kinds (input, change, focus, blur, keydown, keyup) fan
/// out from one delegate in install order.
fn ensure_text_field_entry(node: &crate::Node) -> SharedHandlers {
    let view = node.ns_view();
    let field = crate::node::downcast::<NSTextField>(view)
        .expect("ensure_text_field_entry: node is not an NSTextField");
    let mtm = MainThreadMarker::new()
        .expect("text-field event installs must run on the main thread");

    let mut slot = node.handlers().borrow_mut();
    if let Some(d) = slot.text_field_delegate.as_ref() {
        return d.ivars().handlers.clone();
    }
    let handlers: SharedHandlers = Default::default();
    let delegate = TextFieldDelegate::new(handlers.clone(), mtm);
    let proto: &ProtocolObject<dyn NSTextFieldDelegate> =
        ProtocolObject::from_ref(&*delegate);
    unsafe { field.setDelegate(Some(proto)) };
    slot.text_field_delegate = Some(delegate);
    handlers
}

/// Append an input observer (fires on every keystroke / paste).
/// Multiple installs stack — each callback runs in install order.
/// Used by both `bind:value` (write-back leg) and `on:input`.
/// No-op if `node` isn't an NSTextField.
pub fn on_text_field_change(
    node: &crate::Node,
    cb: impl FnMut(String) + 'static,
) {
    if crate::node::downcast::<NSTextField>(node.ns_view()).is_none() {
        return;
    }
    let handlers = ensure_text_field_entry(node);
    handlers.borrow_mut().on_input.push(Box::new(cb));
}

/// Append a commit observer (fires on return key / focus loss).
/// Used by `on:change`. No-op if `node` isn't an NSTextField.
pub fn on_text_field_end_editing(
    node: &crate::Node,
    cb: impl FnMut(String) + 'static,
) {
    if crate::node::downcast::<NSTextField>(node.ns_view()).is_none() {
        return;
    }
    let handlers = ensure_text_field_entry(node);
    handlers.borrow_mut().on_change.push(Box::new(cb));
}

/// Append a focus observer — fires on `controlTextDidBeginEditing:`
/// (the field gained focus). No-op if `node` isn't an NSTextField.
pub fn on_text_field_focus(
    node: &crate::Node,
    cb: impl FnMut() + 'static,
) {
    if crate::node::downcast::<NSTextField>(node.ns_view()).is_none() {
        return;
    }
    let handlers = ensure_text_field_entry(node);
    handlers.borrow_mut().on_focus.push(Box::new(cb));
}

/// Append a blur observer — fires when editing ends (Return,
/// Tab, click-elsewhere, programmatic resignation). Coexists
/// with `on_text_field_end_editing` (which carries the value);
/// blur handlers run after change handlers from the same notif.
pub fn on_text_field_blur(
    node: &crate::Node,
    cb: impl FnMut() + 'static,
) {
    if crate::node::downcast::<NSTextField>(node.ns_view()).is_none() {
        return;
    }
    let handlers = ensure_text_field_entry(node);
    handlers.borrow_mut().on_blur.push(Box::new(cb));
}

/// Append a keydown observer — fires on recognized command keys
/// (Enter, Escape, Tab, arrows). See [`KeyEvent`] for the
/// supported keys.
pub fn on_text_field_keydown(
    node: &crate::Node,
    cb: impl FnMut(KeyEvent) + 'static,
) {
    if crate::node::downcast::<NSTextField>(node.ns_view()).is_none() {
        return;
    }
    let handlers = ensure_text_field_entry(node);
    handlers.borrow_mut().on_keydown.push(Box::new(cb));
}

/// Append a keyup observer. AppKit's field-editor command
/// pipeline doesn't separate down/up — both fire on the same
/// `doCommandBySelector:` notification. Provided for web-API
/// parity (`on:keyup=…` in upstream examples works without
/// substitution).
pub fn on_text_field_keyup(
    node: &crate::Node,
    cb: impl FnMut(KeyEvent) + 'static,
) {
    if crate::node::downcast::<NSTextField>(node.ns_view()).is_none() {
        return;
    }
    let handlers = ensure_text_field_entry(node);
    handlers.borrow_mut().on_keyup.push(Box::new(cb));
}

// ---------------------------------------------------------------------
// NSTextView delegate (multi-line text — `<text_view>` `bind:value`)
// ---------------------------------------------------------------------
//
// Same fan-out pattern as TextFieldDelegate, but routed through
// NSTextDelegate (NSTextView's protocol — separate from the
// NSControlTextEditingDelegate that NSTextField uses) and keyed by
// the NSTextView pointer. Only `textDidChange:` is wired today —
// nobody's asked for begin/end editing on the multi-line view yet.

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
    /// NSTextView delegate that fans `textDidChange:` notifications
    /// out to all installed callbacks. NSTextView's documented
    /// delegate protocol is `NSTextViewDelegate`, which inherits
    /// `NSTextDelegate` — `textDidChange:` is on the latter.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TextViewIvars]
    pub struct TextViewDelegate;

    unsafe impl NSObjectProtocol for TextViewDelegate {}

    unsafe impl NSTextDelegate for TextViewDelegate {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, notification: &NSNotification) {
            let object = notification.object();
            let Some(object) = object else { return };
            let any: &objc2::runtime::AnyObject = &*object;
            let Some(tv) = any.downcast_ref::<NSTextView>() else {
                return;
            };
            let value: String = tv.string().to_string();
            let mut handlers = match self.ivars().handlers.try_borrow_mut() {
                Ok(h) => h,
                Err(_) => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[cocoa_dom] reentrant textDidChange skipped"
                    );
                    return;
                }
            };
            for cb in handlers.on_change.iter_mut() {
                cb(value.clone());
            }
        }
    }

    unsafe impl NSTextViewDelegate for TextViewDelegate {}
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

/// Locate the NSTextView associated with this Node. For
/// `<text_view>` this is the documentView of the wrapping
/// NSScrollView; for a bare NSTextView Node it's the Node's
/// `ns_view` directly. Returns `None` if neither shape matches.
///
/// The TextViewDelegate gets stored on the OUTER Node's handlers
/// regardless — that's the Node the framework owns. The inner
/// NSTextView's delegate slot points at our delegate but the
/// `Retained` lives on the Node, so lifecycle is Rust-driven.
fn text_view_for_node(node: &crate::Node) -> Option<Retained<NSTextView>> {
    let view = node.ns_view();
    if let Some(tv) = crate::node::downcast::<NSTextView>(view) {
        return Some(tv.retain());
    }
    use objc2_app_kit::NSScrollView;
    let scroll = crate::node::downcast::<NSScrollView>(view)?;
    let doc = scroll.documentView()?;
    crate::node::downcast::<NSTextView>(&doc).map(|tv| tv.retain())
}

/// Look up (or lazily create) the per-text-view handler state on
/// the node's [`NodeHandlers`]. Used by all `on_text_view_*`
/// installers; works for both bare-NSTextView nodes and
/// `<text_view>` (NSScrollView-wrapped) nodes — see
/// [`text_view_for_node`].
fn ensure_text_view_entry(
    node: &crate::Node,
) -> Option<SharedTextViewHandlers> {
    let tv = text_view_for_node(node)?;
    let mtm = MainThreadMarker::new()
        .expect("text-view event installs must run on the main thread");

    let mut slot = node.handlers().borrow_mut();
    if let Some(d) = slot.text_view_delegate.as_ref() {
        return Some(d.ivars().handlers.clone());
    }
    let handlers: SharedTextViewHandlers = Default::default();
    // Wrap delegate creation + setDelegate in a tight
    // autoreleasepool. `NSText.setDelegate:` internally autoreleases
    // an extra retain on the delegate (verified empirically — the
    // very first text_view in a process scope shows retainCount=2
    // immediately after this call, while subsequent text_views in
    // the same scope show retainCount=1 because the autorelease pool
    // has already been drained between them). Without the tight
    // pool, the FIRST text_view's delegate stays alive past
    // `NodeHandlersBundle::Drop` for as long as the *outer*
    // autoreleasepool lives — which surfaces as a deferred
    // delegate-store leak in unit tests and the fuzzer, even though
    // every Rust-side reference has been dropped. Wrapping setDelegate
    // in its own pool drains the extra retain immediately, so when
    // we later drop our Retained, the delegate dealloc fires on
    // schedule.
    let delegate = objc2::rc::autoreleasepool(|_| {
        let d = TextViewDelegate::new(handlers.clone(), mtm);
        let proto: &ProtocolObject<dyn NSTextViewDelegate> =
            ProtocolObject::from_ref(&*d);
        tv.setDelegate(Some(proto));
        d
    });
    slot.text_view_delegate = Some(delegate);
    Some(handlers)
}

/// Append a change observer on an NSTextView — fires on every
/// keystroke (it's the multi-line analog of NSTextField's
/// `controlTextDidChange:`). Stacks: multiple installs on the same
/// view all fire in install order. No-op if `node` doesn't back
/// an NSTextView (directly or via a wrapping `<text_view>` scroll
/// view).
pub fn on_text_view_change(
    node: &crate::Node,
    cb: impl FnMut(String) + 'static,
) {
    let Some(handlers) = ensure_text_view_entry(node) else { return };
    handlers.borrow_mut().on_change.push(Box::new(cb));
}

/// Wire the given closure to fire when an NSControl's action fires
/// — clicks for NSButton/NSPopUpButton, value changes for NSSlider,
/// etc. (NSButton, NSSlider, NSPopUpButton, NSColorWell, ... are
/// all NSControls — target/action is the unifying mechanism.)
///
/// **Single handler per control.** NSControl has one target/action
/// slot; we don't fan out (the alternative would be a Vec or a
/// shared-target ObjC subclass — both add allocations for the
/// 99% case where there's only one handler). A second install on
/// the same control panics rather than silently overwriting.
///
/// This means:
///   * `<MyComponent on:click=outer>` where the inner component
///     also installs a click handler on its top-level NSControl
///     panics. Workaround: have the component accept a
///     `Callback<()>` prop and call it from the inner closure.
///   * `<checkbox bind:checked=signal on:click=cb>` panics
///     (bind:checked installs a write-back action; the user's
///     `on:click=cb` would be the second installer). Workaround:
///     wire the user logic into a single closure that also calls
///     the bind setter, or add an `Effect` that watches the
///     signal.
pub fn on_control_action(
    node: &crate::Node,
    cb: impl FnMut() + 'static,
) {
    let view = node.ns_view();
    let Some(control) = crate::node::downcast::<NSControl>(view) else {
        return;
    };
    let mtm = objc2::MainThreadMarker::new()
        .expect("on_control_action must run on the main thread");

    // Detect duplicate install. A non-nil target after our prior
    // wiring means someone already installed a handler on this
    // control — panic rather than silently overwriting.
    if let Some(existing) = control.target() {
        panic!(
            "on_control_action called twice on the same NSControl \
             ({:p}). NSControl has a single target/action slot — \
             fanning out would silently break the existing handler. \
             Workaround: combine your handlers into one closure, \
             or have any component that accepts on:click also \
             accept a Callback<()> prop. Existing target: {:p}",
            &*control, &*existing,
        );
    }

    let target = ActionTarget::new(cb, mtm);
    let target_obj: &NSObject = &target;
    unsafe {
        control.setTarget(Some(target_obj));
        control.setAction(Some(action_fired_sel()));
    }

    node.handlers().borrow_mut().action_target = Some(target);
}

