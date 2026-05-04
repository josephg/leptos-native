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

use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{NSObject, ProtocolObject, Sel},
    sel, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSControl, NSControlTextEditingDelegate, NSTextField,
    NSTextFieldDelegate, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol};
use std::{cell::RefCell, collections::HashMap};

/// The closure type carried by [`ActionTarget`]. `&mut` so that the
/// callback can update reactive state. Wrapped in a RefCell so that
/// the ObjC instance method (which has `&self`) can call it.
type Callback = RefCell<Box<dyn FnMut() + 'static>>;

define_class!(
    /// ObjC class that holds a single Rust closure and exposes one
    /// selector, `actionFired:`, that invokes it. Used as the target
    /// of NSControl target/action wiring.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Callback]
    pub struct ActionTarget;

    impl ActionTarget {
        #[unsafe(method(actionFired:))]
        fn action_fired(&self, _sender: *mut NSObject) {
            // Best-effort: if the callback panics, we don't want to
            // unwind into ObjC. Just log and swallow.
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
            // The callback is `Box<dyn FnMut()>`; deref then invoke.
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
    let key = view_key(view);
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
    HANDLER_STORE.with_borrow_mut(|store| {
        store.remove(&key);
    });
    TEXT_FIELD_STORE.with_borrow_mut(|store| {
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
fn ensure_text_field_entry(field: &NSTextField) -> SharedHandlers {
    let mtm = MainThreadMarker::new()
        .expect("text-field event installs must run on the main thread");
    let key = view_key(field.as_ref());
    TEXT_FIELD_STORE.with_borrow_mut(|store| {
        if let Some(entry) = store.get(&key) {
            return entry.handlers.clone();
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

/// Wire the given closure to fire when an NSControl's action fires
/// — clicks for NSButton/NSPopUpButton, value changes for NSSlider,
/// etc. (NSButton, NSSlider, NSPopUpButton, NSColorWell, ... are
/// all NSControls — target/action is the unifying mechanism.)
///
/// Multiple handlers per control are supported by retaining all of
/// them in our store; however, NSControl's target/action only stores
/// *one* target/action pair, so calling this twice replaces the
/// previous wiring (the previous handler stays in the retain-store
/// but never fires again).
pub fn on_control_action(
    control: &NSControl,
    cb: impl FnMut() + 'static,
) {
    let mtm = objc2::MainThreadMarker::new()
        .expect("on_control_action must run on the main thread");
    let target = ActionTarget::new(cb, mtm);

    let target_obj: &NSObject = &target;
    unsafe {
        control.setTarget(Some(target_obj));
        control.setAction(Some(action_fired_sel()));
    }

    keep_target_alive(control.as_ref(), target);
}

