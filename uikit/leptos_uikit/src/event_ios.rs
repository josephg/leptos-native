//! Macro-facing facade for `tachys::html::event` on iOS.
//!
//! Maps the `view!{}` macro's event emissions to UIKit event handlers
//! (UIControl target/action for buttons/switches, editing events for
//! text fields).

#![allow(non_upper_case_globals, missing_docs)]

// ---------------------------------------------------------------------
// Event marker types and descriptors
// ---------------------------------------------------------------------

pub struct ClickEvent;
pub const click: ClickEvent = ClickEvent;

pub struct InputEvent;
pub const input: InputEvent = InputEvent;

pub struct ChangeEvent;
pub const change: ChangeEvent = ChangeEvent;

pub struct FocusEvent;
pub const focus: FocusEvent = FocusEvent;

pub struct BlurEvent;
pub const blur: BlurEvent = BlurEvent;

pub struct KeyDownEvent;
pub const keydown: KeyDownEvent = KeyDownEvent;

pub struct KeyUpEvent;
pub const keyup: KeyUpEvent = KeyUpEvent;

pub trait EventDescriptor {
    type EventType;
    fn into_pending<F>(handler: F) -> PendingHandler
    where
        F: FnMut(Self::EventType) + Send + 'static;
}

impl EventDescriptor for ClickEvent {
    type EventType = ();
    fn into_pending<F>(mut handler: F) -> PendingHandler
    where
        F: FnMut(()) + Send + 'static,
    {
        PendingHandler::Click(Box::new(move || handler(())))
    }
}

impl EventDescriptor for InputEvent {
    type EventType = String;
    fn into_pending<F>(handler: F) -> PendingHandler
    where
        F: FnMut(String) + Send + 'static,
    {
        PendingHandler::Input(Box::new(handler))
    }
}

impl EventDescriptor for ChangeEvent {
    type EventType = String;
    fn into_pending<F>(handler: F) -> PendingHandler
    where
        F: FnMut(String) + Send + 'static,
    {
        PendingHandler::Change(Box::new(handler))
    }
}

impl EventDescriptor for FocusEvent {
    type EventType = ();
    fn into_pending<F>(mut handler: F) -> PendingHandler
    where
        F: FnMut(()) + Send + 'static,
    {
        PendingHandler::Focus(Box::new(move || handler(())))
    }
}

impl EventDescriptor for BlurEvent {
    type EventType = ();
    fn into_pending<F>(mut handler: F) -> PendingHandler
    where
        F: FnMut(()) + Send + 'static,
    {
        PendingHandler::Blur(Box::new(move || handler(())))
    }
}

impl EventDescriptor for KeyDownEvent {
    type EventType = ios_dom::KeyEvent;
    fn into_pending<F>(handler: F) -> PendingHandler
    where
        F: FnMut(ios_dom::KeyEvent) + Send + 'static,
    {
        PendingHandler::KeyDown(Box::new(handler))
    }
}

impl EventDescriptor for KeyUpEvent {
    type EventType = ios_dom::KeyEvent;
    fn into_pending<F>(handler: F) -> PendingHandler
    where
        F: FnMut(ios_dom::KeyEvent) + Send + 'static,
    {
        PendingHandler::KeyUp(Box::new(handler))
    }
}

// ---------------------------------------------------------------------
// SupportsEvent — compile-time control/event pairing
// ---------------------------------------------------------------------

pub trait SupportsEvent<E: EventDescriptor> {}

// ---------------------------------------------------------------------
// PendingHandler
// ---------------------------------------------------------------------

pub enum PendingHandler {
    Click(Box<dyn FnMut() + Send + 'static>),
    Input(Box<dyn FnMut(String) + Send + 'static>),
    Change(Box<dyn FnMut(String) + Send + 'static>),
    Focus(Box<dyn FnMut() + Send + 'static>),
    Blur(Box<dyn FnMut() + Send + 'static>),
    KeyDown(Box<dyn FnMut(ios_dom::KeyEvent) + Send + 'static>),
    KeyUp(Box<dyn FnMut(ios_dom::KeyEvent) + Send + 'static>),
}

impl PendingHandler {
    pub fn apply_to(self, el: &ios_dom::Element) {
        match self {
            PendingHandler::Click(cb) => el.on_click(cb),
            PendingHandler::Input(cb) => el.on_text_change(cb),
            PendingHandler::Change(cb) => el.on_text_end_editing(cb),
            PendingHandler::Focus(cb) => el.on_text_focus(cb),
            PendingHandler::Blur(cb) => el.on_text_blur(cb),
            PendingHandler::KeyDown(cb) => el.on_text_keydown(cb),
            PendingHandler::KeyUp(cb) => el.on_text_keyup(cb),
        }
    }
}

// ---------------------------------------------------------------------
// Free-standing `on()` for the spread-attribute path
// ---------------------------------------------------------------------

pub fn on<E, F>(_event: E, handler: F) -> OnAttribute
where
    E: EventDescriptor,
    F: FnMut(E::EventType) + Send + 'static,
{
    OnAttribute {
        handler: Some(E::into_pending(handler)),
    }
}

pub struct OnAttribute {
    pub handler: Option<PendingHandler>,
}

impl OnAttribute {
    pub fn apply(mut self, el: &ios_dom::Element) {
        if let Some(h) = self.handler.take() {
            h.apply_to(el);
        }
    }

    pub fn take_pending(mut self) -> Option<PendingHandler> {
        self.handler.take()
    }
}

// Phase 8: dropped the upstream Attribute / NextAttribute / ToTemplate
// impls on OnAttribute (SSR-coupled machinery from the deleted
// tachys::html::attribute::* + tachys::view::ToTemplate). Inline `.on()`
// on the element builder is the only path examples use, and it goes
// through `OnAttribute::apply` directly.
