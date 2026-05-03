//! Macro-facing facade for `tachys::html::event` on macOS.
//!
//! The `view!{}` macro emits paths like
//! `::leptos::tachys::html::event::on(::leptos::tachys::html::event::click, handler)`
//! to wire up event listeners. On macOS we map this to our Cocoa
//! event-handler infrastructure (target/action for buttons,
//! NSTextFieldDelegate for text input).
//!
//! The macro emits `.on(event, handler)` directly on the element
//! builder for inline `on:event=handler` syntax. The free-standing
//! `on(event, handler) -> OnAttribute` entry point exists for the
//! spread case (`view!{ <button {..attr}/> }` where `attr = on(...)`).
//! Both paths converge on [`PendingHandler`], a typed enum the
//! element's `Render::build` drains and installs.

#![allow(non_upper_case_globals, missing_docs)]

// ---------------------------------------------------------------------
// Event marker types and descriptors
// ---------------------------------------------------------------------

/// Marker type for the click event (NSButton target/action).
pub struct ClickEvent;
pub const click: ClickEvent = ClickEvent;

/// Marker type for the input event — fires on every text change as
/// the user types in a text field. AppKit equivalent:
/// `controlTextDidChange:`.
pub struct InputEvent;
pub const input: InputEvent = InputEvent;

/// Marker type for the change event — fires when text editing
/// commits (return key, focus loss). AppKit equivalent:
/// `controlTextDidEndEditing:`.
pub struct ChangeEvent;
pub const change: ChangeEvent = ChangeEvent;

/// Each event descriptor knows its payload type ([`EventType`]) and
/// how to package a user-supplied handler into a [`PendingHandler`]
/// the element can install in `Render::build`.
pub trait EventDescriptor {
    /// Payload passed to the handler closure. `()` for events with
    /// no meaningful payload (e.g. click); `String` for text events.
    type EventType;

    /// Box the handler into the right [`PendingHandler`] variant.
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

// ---------------------------------------------------------------------
// PendingHandler — typed wrapper passed from the builder into build()
// ---------------------------------------------------------------------

/// A user-supplied event handler, tagged with which AppKit hook to
/// install it via. Element builders accumulate a `Vec<PendingHandler>`
/// during construction; `Render::build` drains it and installs each
/// handler against the constructed `cocoa_dom::Element`.
///
/// Routing per variant (in `apply_to`):
///   * `Click`  — `Element::on_click`           (NSButton target/action)
///   * `Input`  — `Element::on_text_change`     (controlTextDidChange:)
///   * `Change` — `Element::on_text_end_editing` (controlTextDidEndEditing:)
///
/// If the element type doesn't match (e.g. `Click` on a text field),
/// the underlying cocoa_dom call no-ops — matches the web's
/// `addEventListener` shape.
pub enum PendingHandler {
    Click(Box<dyn FnMut() + Send + 'static>),
    Input(Box<dyn FnMut(String) + Send + 'static>),
    Change(Box<dyn FnMut(String) + Send + 'static>),
}

impl PendingHandler {
    /// Install this handler against `el`. No-ops if the underlying
    /// AppKit view doesn't support the event (the cocoa_dom hooks
    /// downcast and silently drop on mismatch).
    pub fn apply_to(self, el: &cocoa_dom::Element) {
        match self {
            PendingHandler::Click(cb) => el.on_click(cb),
            PendingHandler::Input(cb) => el.on_text_change(cb),
            PendingHandler::Change(cb) => el.on_text_end_editing(cb),
        }
    }
}

// ---------------------------------------------------------------------
// Free-standing `on()` for the spread-attribute path
// ---------------------------------------------------------------------

/// `on(event, handler)` — wraps a handler closure into an attribute
/// the macro can apply to an element via `.add_any_attr(...)`.
///
/// Used for the spread-syntax case (`view!{ <button {..attr}/> }`
/// where `attr = on(click, ...)`). Inline `on:event=handler` goes
/// directly through the element's `on()` method without constructing
/// an `OnAttribute`.
pub fn on<E, F>(_event: E, handler: F) -> OnAttribute
where
    E: EventDescriptor,
    F: FnMut(E::EventType) + Send + 'static,
{
    OnAttribute {
        handler: Some(E::into_pending(handler)),
    }
}

/// Attribute produced by [`on`]. Applied to elements via
/// `.add_any_attr(...)`. Each element's `add_any_attr` impl simply
/// calls `attr.apply(&self.el_or_pending_handlers)`.
pub struct OnAttribute {
    pub handler: Option<PendingHandler>,
}

impl OnAttribute {
    /// Apply this attribute to a built cocoa Element (used by
    /// elements whose builder has already constructed the underlying
    /// view).
    pub fn apply(mut self, el: &cocoa_dom::Element) {
        if let Some(h) = self.handler.take() {
            h.apply_to(el);
        }
    }

    /// Take the inner [`PendingHandler`] so the builder can stash it
    /// alongside other pre-build handlers and install at build time.
    pub fn take_pending(mut self) -> Option<PendingHandler> {
        self.handler.take()
    }
}
