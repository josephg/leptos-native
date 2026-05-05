//! Event handlers — bridging UIKit's target/action and delegate
//! patterns into Rust closures.
//!
//! Design:
//! - `ActionTarget` ObjC class holds a Rust closure as an ivar and
//!   exposes one selector (`actionFired:`) that invokes it. Created
//!   per registered handler and stashed in a thread-local registry.
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
    UIControl, UITextField, UITextView, UITextViewDelegate, UIView,
    UIScrollViewDelegate,
};
use objc2_foundation::NSObjectProtocol;
use std::{cell::RefCell, collections::HashMap};

// ---------------------------------------------------------------------
// ActionTarget — shared ObjC class for UIControl target/action
// ---------------------------------------------------------------------

type Callback = RefCell<Box<dyn FnMut() + 'static>>;

define_class!(
    /// ObjC class that holds one Rust closure and exposes one
    /// selector, `actionFired:`, that invokes it. Used as the
    /// target of UIControl target/action wiring.
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
        let this = alloc.set_ivars(RefCell::new(Box::new(cb)));
        unsafe { msg_send![super(this), init] }
    }
}

pub fn action_fired_sel() -> Sel {
    sel!(actionFired:)
}

// ---------------------------------------------------------------------
// Handler store — keep ActionTargets alive
// ---------------------------------------------------------------------

thread_local! {
    static HANDLER_STORE: RefCell<
        HashMap<usize, Vec<Retained<ActionTarget>>>
    > = RefCell::new(HashMap::new());
}

fn view_key(view: &UIView) -> usize {
    let ptr: *const UIView = view;
    ptr as usize
}

pub fn keep_target_alive(view: &UIView, target: Retained<ActionTarget>) {
    let key = view_key(view);
    HANDLER_STORE.with_borrow_mut(|store| {
        store.entry(key).or_default().push(target);
    });
}

/// Drop all retained handlers attached to `view`. Called from
/// [`crate::node::Node::teardown`].
pub fn drop_handlers_for(view: &UIView) {
    let key = view_key(view);
    HANDLER_STORE.with_borrow_mut(|store| {
        store.remove(&key);
    });
    TEXT_VIEW_STORE.with_borrow_mut(|store| {
        store.remove(&key);
    });
}

// ---------------------------------------------------------------------
// On-control-action: UIControl target/action
// ---------------------------------------------------------------------

/// Variant of [`on_control_action`] that lets the caller specify
/// which UIControlEvents to listen for. The handler always takes
/// `()`. If you need the control's value, read it inside the closure.
fn on_control_action_with_events(
    control: &UIControl,
    events: objc2_ui_kit::UIControlEvents,
    cb: impl FnMut() + 'static,
) {
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

    keep_target_alive(control.as_ref(), target);
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
    control: &UIControl,
    cb: impl FnMut() + 'static,
) {
    // Choose the event based on control type.
    let any: &objc2::runtime::AnyObject = control.as_ref();
    let events = if any.downcast_ref::<objc2_ui_kit::UIButton>().is_some() {
        objc2_ui_kit::UIControlEvents::TouchUpInside
    } else if any.downcast_ref::<objc2_ui_kit::UISlider>().is_some()
        || any.downcast_ref::<objc2_ui_kit::UISwitch>().is_some()
        || any.downcast_ref::<objc2_ui_kit::UISegmentedControl>().is_some()
        || any.downcast_ref::<objc2_ui_kit::UIDatePicker>().is_some()
        || any.downcast_ref::<objc2_ui_kit::UIStepper>().is_some()
    {
        objc2_ui_kit::UIControlEvents::ValueChanged
    } else {
        // Default: ValueChanged + TouchUpInside (generic controls)
        objc2_ui_kit::UIControlEvents::ValueChanged
    };
    on_control_action_with_events(control, events, cb);
}

// ---------------------------------------------------------------------
// UITextField events via UIControl editing events
// ---------------------------------------------------------------------

/// Append an input observer on a UITextField (fires on every
/// keystroke / paste). Uses `UIControlEventEditingChanged`.
pub fn on_text_field_change(
    field: &UITextField,
    mut cb: impl FnMut(String) + 'static,
) {
    let field_ref: &UIControl = field;
    let field_clone: Retained<UITextField> = field.into();
    on_control_action_with_events(
        field_ref,
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
    field: &UITextField,
    mut cb: impl FnMut(String) + 'static,
) {
    let field_ref: &UIControl = field;
    let field_clone: Retained<UITextField> = field.into();
    on_control_action_with_events(
        field_ref,
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
    field: &UITextField,
    cb: impl FnMut() + 'static,
) {
    let field_ref: &UIControl = field;
    on_control_action_with_events(
        field_ref,
        objc2_ui_kit::UIControlEvents::EditingDidBegin,
        cb,
    );
}

/// Append a blur observer on a UITextField (fires on
/// `UIControlEventEditingDidEnd` — same as end editing but without
/// the value payload).
pub fn on_text_field_blur(
    field: &UITextField,
    cb: impl FnMut() + 'static,
) {
    let field_ref: &UIControl = field;
    on_control_action_with_events(
        field_ref,
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

define_class!(
    /// UITextView delegate that fans `textViewDidChange:` notifications
    /// out to all installed callbacks.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SharedTextViewHandlers]
    pub struct TextViewDelegate;

    unsafe impl NSObjectProtocol for TextViewDelegate {}

    unsafe impl UIScrollViewDelegate for TextViewDelegate {}

    unsafe impl UITextViewDelegate for TextViewDelegate {
        #[unsafe(method(textViewDidChange:))]
        fn text_view_did_change(&self, text_view: &UITextView) {
            let value: String = text_view.text().to_string();
            let mut handlers = match self.ivars().try_borrow_mut() {
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

fn ensure_text_view_entry(
    tv: &UITextView,
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
        let proto: &ProtocolObject<dyn UITextViewDelegate> =
            ProtocolObject::from_ref(&*delegate);
        unsafe { tv.setDelegate(Some(proto)) };
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

/// Append a change observer on a UITextView — fires on every
/// keystroke.
pub fn on_text_view_change(
    tv: &UITextView,
    cb: impl FnMut(String) + 'static,
) {
    let handlers = ensure_text_view_entry(tv);
    handlers.borrow_mut().on_change.push(Box::new(cb));
}
