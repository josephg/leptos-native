//! Event handlers — bridging AppKit's target/action and delegate
//! patterns into Rust closures.
//!
//! Design: a small `ActionTarget` ObjC class holds a Rust closure as an
//! ivar and exposes one selector (`actionFired:`) that invokes it. We
//! create one of these per registered handler, set the AppKit control's
//! target/action to point at it, and stash the `Retained<ActionTarget>`
//! in a thread-local registry keyed by the NSView's pointer so it
//! outlives the registration.
//!
//! When the element is removed from the tree, [`drop_handlers_for`]
//! is called from [`crate::node::Node::teardown`] to clean up the
//! registry entry. Teardown is driven by the `Mountable::unmount`
//! cascade — for top-level windows that means
//! `WindowDelegate::windowWillClose:` runs the cleanup closure
//! that unmounts the window's children.

use crate::KeyEvent;
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{Bool, NSObject, ProtocolObject, Sel},
    sel, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSControl, NSControlTextEditingDelegate, NSTextDelegate, NSTextField,
    NSTextFieldDelegate, NSTextView, NSTextViewDelegate, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol};
use std::{cell::RefCell, collections::HashMap};

/// The closure carried by [`ActionTarget`]. One per NSControl —
/// see `on_control_action`'s docstring for why we panic on
/// duplicate installs rather than fan out.
type Callback = RefCell<Box<dyn FnMut() + 'static>>;

define_class!(
    /// ObjC class that holds one Rust closure and exposes one
    /// selector, `actionFired:`, that invokes it. Used as the
    /// target of NSControl target/action wiring.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Callback]
    pub struct ActionTarget;

    impl ActionTarget {
        #[unsafe(method(actionFired:))]
        fn action_fired(&self, _sender: *mut NSObject) {
            let mut cb = match self.ivars().try_borrow_mut() {
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
        let this = alloc.set_ivars(RefCell::new(Box::new(cb)));
        unsafe { msg_send![super(this), init] }
    }
}

/// Selector matching `ActionTarget::action_fired`. Cached for cheap
/// lookups when wiring up controls.
pub fn action_fired_sel() -> Sel {
    sel!(actionFired:)
}

// ---------------------------------------------------------------------
// Storage: keep ActionTargets alive as long as the views they're
// attached to. Stage-3 implementation is a thread-local hashmap;
// entries leak on view drop. See module docs.
// ---------------------------------------------------------------------

thread_local! {
    static HANDLER_STORE: RefCell<
        HashMap<usize, Vec<Retained<ActionTarget>>>
    > = RefCell::new(HashMap::new());
}

fn view_key(view: &NSView) -> usize {
    let ptr: *const NSView = view;
    ptr as usize
}

/// Retain `target` for the lifetime of `view`. Stage-3 implementation
/// just stashes it in a thread-local map; entries leak on view drop.
pub fn keep_target_alive(view: &NSView, target: Retained<ActionTarget>) {
    keep_target_alive_for_key(view_key(view), target);
}

/// Pointer-key variant — used by code that owns a different ObjC
/// kind (e.g. `NSMenuItem`) and wants to share the NSControl handler
/// store. The key is whatever the caller's matching `drop_handlers`
/// passes; pointer-as-`usize` is fine. Note that the menu and view
/// keyspaces share a HashMap, so collisions are technically possible
/// but vanishingly unlikely — ObjC objects are aligned and live in
/// distinct zones.
pub fn keep_target_alive_for_key(key: usize, target: Retained<ActionTarget>) {
    HANDLER_STORE.with_borrow_mut(|store| {
        store.entry(key).or_default().push(target);
    });
}

/// Drop all retained handlers attached to `view`. Called from
/// [`crate::node::Node::teardown`], which is invoked via the
/// `Mountable::unmount` chain (e.g. on `windowWillClose:` for a
/// window's content tree).
pub fn drop_handlers_for(view: &NSView) {
    let key = view_key(view);
    drop_handlers_for_view_key(key);
}

/// Drop only the HANDLER_STORE entry for `key` (no TEXT_FIELD_STORE
/// / TEXT_VIEW_STORE touch). Used by non-NSView consumers like
/// `cocoa_dom::menu::MenuItem` where the key is an NSMenuItem
/// pointer that has no overlap with text-field-only storage.
pub fn drop_action_target_for_key(key: usize) {
    HANDLER_STORE.with_borrow_mut(|store| {
        store.remove(&key);
    });
}

/// Test-only: count the retained `ActionTarget`s currently held
/// for `key` in the shared `HANDLER_STORE`. Returns 0 when the
/// entry has been removed (e.g. after a `drop_action_target_for_key`).
#[doc(hidden)]
pub fn handler_count_for_test_key(key: usize) -> usize {
    HANDLER_STORE.with_borrow(|store| {
        store.get(&key).map(|v| v.len()).unwrap_or(0)
    })
}

fn drop_handlers_for_view_key(key: usize) {
    HANDLER_STORE.with_borrow_mut(|store| {
        store.remove(&key);
    });
    TEXT_FIELD_STORE.with_borrow_mut(|store| {
        store.remove(&key);
    });
    TEXT_VIEW_STORE.with_borrow_mut(|store| {
        store.remove(&key);
    });
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

define_class!(
    /// ObjC class that observes text-field input (`controlTextDidChange:`)
    /// and commit (`controlTextDidEndEditing:`), fanning each event out
    /// to all installed callbacks for the field.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SharedHandlers]
    pub struct TextFieldDelegate;

    unsafe impl NSObjectProtocol for TextFieldDelegate {}

    unsafe impl NSControlTextEditingDelegate for TextFieldDelegate {
        #[unsafe(method(controlTextDidBeginEditing:))]
        fn control_text_did_begin_editing(
            &self,
            _notification: &NSNotification,
        ) {
            let mut handlers = match self.ivars().try_borrow_mut() {
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
            let mut handlers = match self.ivars().try_borrow_mut() {
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
            let mut handlers = match self.ivars().try_borrow_mut() {
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
            let mut handlers = match self.ivars().try_borrow_mut() {
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
        let this = alloc.set_ivars(handlers);
        unsafe { msg_send![super(this), init] }
    }
}

// Per-field state lives in TEXT_FIELD_STORE. The first install on a
// field constructs the delegate (and sets it on the field); subsequent
// installs locate the existing entry by view_key and append to its
// shared handlers.
struct TextFieldEntry {
    handlers: SharedHandlers,
    // Held to keep the delegate alive (NSTextField stores delegate
    // weakly).
    _delegate: Retained<TextFieldDelegate>,
}

thread_local! {
    static TEXT_FIELD_STORE: RefCell<HashMap<usize, TextFieldEntry>> =
        RefCell::new(HashMap::new());
}

/// Look up (or lazily create) the per-field handler state, ensuring
/// the field has our `TextFieldDelegate` installed. Returns the
/// shared handler Vec the caller can append to.
///
/// Cache validity: AppKit can recycle NSTextField memory addresses
/// across allocations. A stale entry would silently misroute
/// events to a dead field's handlers. Before reusing an entry we
/// verify the field's current delegate is the one we stored; if
/// not (recycled address, or someone else swapped the delegate),
/// we evict and rebuild.
fn ensure_text_field_entry(field: &NSTextField) -> SharedHandlers {
    let mtm = MainThreadMarker::new()
        .expect("text-field event installs must run on the main thread");
    let key = view_key(field.as_ref());
    TEXT_FIELD_STORE.with_borrow_mut(|store| {
        if let Some(entry) = store.get(&key) {
            let stored_ptr: *const TextFieldDelegate = &*entry._delegate;
            let current = field.delegate();
            let still_ours = match current {
                Some(d) => {
                    let d_ptr: *const _ = &*d;
                    let d_addr = d_ptr as usize;
                    let stored_addr = stored_ptr as usize;
                    d_addr == stored_addr
                }
                None => false,
            };
            if still_ours {
                return entry.handlers.clone();
            }
            store.remove(&key);
        }
        let handlers: SharedHandlers = Default::default();
        let delegate = TextFieldDelegate::new(handlers.clone(), mtm);
        let proto: &ProtocolObject<dyn NSTextFieldDelegate> =
            ProtocolObject::from_ref(&*delegate);
        unsafe { field.setDelegate(Some(proto)) };
        store.insert(
            key,
            TextFieldEntry {
                handlers: handlers.clone(),
                _delegate: delegate,
            },
        );
        handlers
    })
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

define_class!(
    /// NSTextView delegate that fans `textDidChange:` notifications
    /// out to all installed callbacks. NSTextView's documented
    /// delegate protocol is `NSTextViewDelegate`, which inherits
    /// `NSTextDelegate` — `textDidChange:` is on the latter.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SharedTextViewHandlers]
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
            let mut handlers = match self.ivars().try_borrow_mut() {
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
        let this = alloc.set_ivars(handlers);
        unsafe { msg_send![super(this), init] }
    }
}

struct TextViewEntry {
    handlers: SharedTextViewHandlers,
    _delegate: Retained<TextViewDelegate>,
}

thread_local! {
    static TEXT_VIEW_STORE: RefCell<HashMap<usize, TextViewEntry>> =
        RefCell::new(HashMap::new());
}

/// Look up (or lazily create) the per-text-view handler state,
/// ensuring the NSTextView has our `TextViewDelegate` installed.
/// Same recycled-pointer cache-validity check as
/// `ensure_text_field_entry` — verify the field's current delegate
/// matches the one we stored before reusing the cache.
fn ensure_text_view_entry(
    tv: &NSTextView,
) -> SharedTextViewHandlers {
    let mtm = MainThreadMarker::new()
        .expect("text-view event installs must run on the main thread");
    let key = view_key(tv.as_ref());
    TEXT_VIEW_STORE.with_borrow_mut(|store| {
        if let Some(entry) = store.get(&key) {
            let stored_ptr: *const TextViewDelegate = &*entry._delegate;
            let current = tv.delegate();
            let still_ours = match current {
                Some(d) => {
                    let d_ptr: *const _ = &*d;
                    d_ptr as usize == stored_ptr as usize
                }
                None => false,
            };
            if still_ours {
                return entry.handlers.clone();
            }
            store.remove(&key);
        }
        let handlers: SharedTextViewHandlers = Default::default();
        let delegate = TextViewDelegate::new(handlers.clone(), mtm);
        let proto: &ProtocolObject<dyn NSTextViewDelegate> =
            ProtocolObject::from_ref(&*delegate);
        tv.setDelegate(Some(proto));
        store.insert(
            key,
            TextViewEntry {
                handlers: handlers.clone(),
                _delegate: delegate,
            },
        );
        handlers
    })
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
    // We look at the control's CURRENT target rather than at
    // HANDLER_STORE because HANDLER_STORE entries can be stale
    // across recycled NSView pointers (the previous owner was torn
    // down, drop_handlers_for cleared the entry, but if drop was
    // somehow missed and the pointer got reused, we don't want to
    // panic here). The control's own target reflects ground truth.
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

    keep_target_alive(control.as_ref(), target);
}

