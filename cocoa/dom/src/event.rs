//! Event handlers — bridging AppKit's target/action and delegate
//! patterns into Rust closures.
//!
//! Design: a small `ActionTarget` ObjC class holds a Rust closure as
//! an ivar and exposes one selector (`actionFired:`) that invokes it.
//! We create one per registered handler, wire the AppKit control's
//! `target` / `action` to point at it, then attach it as an
//! **associated object** on the host (via
//! [`objc2::ffi::objc_setAssociatedObject`]) so the ObjC runtime
//! releases it automatically when the host is deallocated.
//!
//! Same pattern for `TextFieldDelegate` and `TextViewDelegate`.
//!
//! There is **no global storage** and no `drop_handlers_for`
//! plumbing — the lifecycle is driven by ObjC reference counting on
//! the host view. When `unmount` releases the last Retained on the
//! view, the view's `dealloc` runs, the runtime releases its
//! associated objects, our delegates' dealloc runs, and the Rust
//! ivars (closures, handler Vecs) drop in turn.

use crate::KeyEvent;
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObject, ProtocolObject, Sel},
    sel, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSControl, NSControlTextEditingDelegate, NSTextDelegate, NSTextField,
    NSTextFieldDelegate, NSTextView, NSTextViewDelegate,
};
use objc2_foundation::{NSNotification, NSObjectProtocol};
use std::{
    cell::RefCell,
    ffi::c_void,
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
// Associated-object helper
// ---------------------------------------------------------------------

/// Attach `value` to `host` as an ObjC associated object under
/// `key` with `OBJC_ASSOCIATION_RETAIN_NONATOMIC` policy. The
/// runtime retains `value` for the lifetime of `host`, releasing
/// it when `host` is deallocated. Repeated calls with the same
/// key replace the previous association.
fn associate(host: &AnyObject, key: *const c_void, value: Option<&AnyObject>) {
    use objc2::ffi::{objc_setAssociatedObject, OBJC_ASSOCIATION_RETAIN_NONATOMIC};
    let host_ptr = host as *const AnyObject as *mut AnyObject;
    let val_ptr = value
        .map(|v| v as *const AnyObject as *mut AnyObject)
        .unwrap_or(std::ptr::null_mut());
    unsafe {
        objc_setAssociatedObject(
            host_ptr,
            key,
            val_ptr,
            OBJC_ASSOCIATION_RETAIN_NONATOMIC,
        );
    }
}

// Unique keys (each static's address serves as a key). The byte
// value is irrelevant — `objc_setAssociatedObject` keys by pointer
// identity, not by content.
static ACTION_TARGET_KEY: u8 = 0;
static TEXT_FIELD_DELEGATE_KEY: u8 = 0;
static TEXT_VIEW_DELEGATE_KEY: u8 = 0;

/// Lift a `&'static u8` key marker to the `*const c_void` that
/// `objc_setAssociatedObject` / `objc_getAssociatedObject` expect.
fn key_of(marker: &'static u8) -> *const c_void {
    marker as *const u8 as *const c_void
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
// Public install helpers — attach the handler to its host as an
// associated object so the ObjC runtime owns the lifecycle.
// ---------------------------------------------------------------------

/// Attach `target` to `host`. The ObjC runtime keeps `target`
/// alive as long as `host` is alive; when `host` deallocates, the
/// runtime releases `target` and its Rust ivars (the closure)
/// drop. Replaces any prior ActionTarget on the same host.
///
/// `host` is any NSObject: NSControl (button, slider, checkbox, …),
/// NSMenuItem, NSToolbarItem — anything in the target/action pattern.
pub fn attach_action_target<H>(host: &H, target: Retained<ActionTarget>)
where
    H: AsRef<AnyObject>,
{
    let target_obj: &AnyObject = (&*target).as_ref();
    associate(host.as_ref(), key_of(&ACTION_TARGET_KEY), Some(target_obj));
}

/// Test-only: live ActionTargets (created minus dropped). Returns
/// to zero after every host has been deallocated.
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

/// Test-only: is there an ActionTarget currently associated with
/// `host`? Returns true after a successful install, false after the
/// host is released. Replaces the old `handler_count_for_test_key`.
#[doc(hidden)]
pub fn has_action_target_for_test<H>(host: &H) -> bool
where
    H: AsRef<AnyObject>,
{
    use objc2::ffi::objc_getAssociatedObject;
    let host_ref = host.as_ref();
    let host_ptr = host_ref as *const AnyObject as *mut AnyObject;
    !unsafe { objc_getAssociatedObject(host_ptr, key_of(&ACTION_TARGET_KEY)) }
        .is_null()
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

/// Look up (or lazily create) the per-field handler state. The
/// delegate is held alive by an associated object on the
/// NSTextField (`TEXT_FIELD_DELEGATE_KEY`), so its lifetime matches
/// the field's. Repeated installs reuse the same delegate's
/// `SharedHandlers`, so callbacks accumulate in install order.
fn ensure_text_field_entry(field: &NSTextField) -> SharedHandlers {
    use objc2::ffi::objc_getAssociatedObject;

    let mtm = MainThreadMarker::new()
        .expect("text-field event installs must run on the main thread");
    let key = key_of(&TEXT_FIELD_DELEGATE_KEY);
    let host: &AnyObject = field.as_ref();

    // Reuse the existing delegate if we've installed one on this
    // field. objc_getAssociatedObject returns nil otherwise.
    let host_ptr = host as *const AnyObject as *mut AnyObject;
    let existing_ptr = unsafe { objc_getAssociatedObject(host_ptr, key) };
    if !existing_ptr.is_null() {
        let existing: &AnyObject = unsafe { &*existing_ptr };
        if let Some(delegate) = existing.downcast_ref::<TextFieldDelegate>() {
            return delegate.ivars().handlers.clone();
        }
    }

    let handlers: SharedHandlers = Default::default();
    let delegate = TextFieldDelegate::new(handlers.clone(), mtm);
    let proto: &ProtocolObject<dyn NSTextFieldDelegate> =
        ProtocolObject::from_ref(&*delegate);
    unsafe { field.setDelegate(Some(proto)) };
    // Associated object retains the delegate; auto-released when
    // the NSTextField is deallocated.
    let delegate_obj: &AnyObject = (&*delegate).as_ref();
    associate(host, key, Some(delegate_obj));
    handlers
}

/// Append an input observer (fires on every keystroke / paste).
/// Multiple installs on the same field stack — each callback runs in
/// install order. Used by both `bind:value` (write-back leg) and
/// `on:input`.
pub fn on_text_field_change(
    field: &NSTextField,
    cb: impl FnMut(String) + 'static,
) {
    let handlers = ensure_text_field_entry(field);
    handlers.borrow_mut().on_input.push(Box::new(cb));
}

/// Append a commit observer (fires on return key / focus loss).
/// Used by `on:change`.
pub fn on_text_field_end_editing(
    field: &NSTextField,
    cb: impl FnMut(String) + 'static,
) {
    let handlers = ensure_text_field_entry(field);
    handlers.borrow_mut().on_change.push(Box::new(cb));
}

/// Append a focus observer — fires on
/// `controlTextDidBeginEditing:` (the field gained focus and the
/// user has started or is about to start editing).
pub fn on_text_field_focus(
    field: &NSTextField,
    cb: impl FnMut() + 'static,
) {
    let handlers = ensure_text_field_entry(field);
    handlers.borrow_mut().on_focus.push(Box::new(cb));
}

/// Append a blur observer — fires when editing ends (Return,
/// Tab, click-elsewhere, programmatic resignation). Coexists
/// with `on_text_field_end_editing` (which carries the value);
/// blur handlers run after change handlers from the same notif.
pub fn on_text_field_blur(
    field: &NSTextField,
    cb: impl FnMut() + 'static,
) {
    let handlers = ensure_text_field_entry(field);
    handlers.borrow_mut().on_blur.push(Box::new(cb));
}

/// Append a keydown observer — fires on recognized command keys
/// (Enter, Escape, Tab, arrows). See [`KeyEvent`] for the
/// supported keys.
pub fn on_text_field_keydown(
    field: &NSTextField,
    cb: impl FnMut(KeyEvent) + 'static,
) {
    let handlers = ensure_text_field_entry(field);
    handlers.borrow_mut().on_keydown.push(Box::new(cb));
}

/// Append a keyup observer. AppKit's field-editor command
/// pipeline doesn't separate down/up — both fire on the same
/// `doCommandBySelector:` notification. Provided for web-API
/// parity (`on:keyup=…` in upstream examples works without
/// substitution).
pub fn on_text_field_keyup(
    field: &NSTextField,
    cb: impl FnMut(KeyEvent) + 'static,
) {
    let handlers = ensure_text_field_entry(field);
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

/// Look up (or lazily create) the per-text-view handler state.
/// Same associated-object pattern as [`ensure_text_field_entry`].
fn ensure_text_view_entry(
    tv: &NSTextView,
) -> SharedTextViewHandlers {
    use objc2::ffi::objc_getAssociatedObject;

    let mtm = MainThreadMarker::new()
        .expect("text-view event installs must run on the main thread");
    let key = key_of(&TEXT_VIEW_DELEGATE_KEY);
    let host: &AnyObject = tv.as_ref();

    let host_ptr = host as *const AnyObject as *mut AnyObject;
    let existing_ptr = unsafe { objc_getAssociatedObject(host_ptr, key) };
    if !existing_ptr.is_null() {
        let existing: &AnyObject = unsafe { &*existing_ptr };
        if let Some(delegate) = existing.downcast_ref::<TextViewDelegate>() {
            return delegate.ivars().handlers.clone();
        }
    }

    let handlers: SharedTextViewHandlers = Default::default();
    let delegate = TextViewDelegate::new(handlers.clone(), mtm);
    let proto: &ProtocolObject<dyn NSTextViewDelegate> =
        ProtocolObject::from_ref(&*delegate);
    tv.setDelegate(Some(proto));
    let delegate_obj: &AnyObject = (&*delegate).as_ref();
    associate(host, key, Some(delegate_obj));
    handlers
}

/// Append a change observer on an NSTextView — fires on every
/// keystroke (it's the multi-line analog of NSTextField's
/// `controlTextDidChange:`). Stacks: multiple installs on the same
/// view all fire in install order.
pub fn on_text_view_change(
    tv: &NSTextView,
    cb: impl FnMut(String) + 'static,
) {
    let handlers = ensure_text_view_entry(tv);
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
    control: &NSControl,
    cb: impl FnMut() + 'static,
) {
    let mtm = objc2::MainThreadMarker::new()
        .expect("on_control_action must run on the main thread");

    // Detect duplicate install. A non-nil target after our prior
    // wiring means someone already installed a handler on this
    // control — panic rather than silently overwriting.
    //
    // The control's own `target` is ground truth for "is a handler
    // already installed here?" — it survives even if our internal
    // accounting drifts across recycled NSControl pointers.
    if let Some(existing) = control.target() {
        panic!(
            "on_control_action called twice on the same NSControl \
             ({:p}). NSControl has a single target/action slot — \
             fanning out would silently break the existing handler. \
             Workaround: combine your handlers into one closure, \
             or have any component that accepts on:click also \
             accept a Callback<()> prop. Existing target: {:p}",
            control, &*existing,
        );
    }

    let target = ActionTarget::new(cb, mtm);
    let target_obj: &NSObject = &target;
    unsafe {
        control.setTarget(Some(target_obj));
        control.setAction(Some(action_fired_sel()));
    }

    attach_action_target(control, target);
}

