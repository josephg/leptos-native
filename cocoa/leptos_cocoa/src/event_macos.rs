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

/// Marker type for the focus event — fires when a control gains
/// keyboard focus. For text fields this is
/// `controlTextDidBeginEditing:`.
pub struct FocusEvent;
pub const focus: FocusEvent = FocusEvent;

/// Marker type for the blur event — fires when a control loses
/// focus. For text fields this is `controlTextDidEndEditing:` (so
/// blur fires alongside change, just without a value payload).
pub struct BlurEvent;
pub const blur: BlurEvent = BlurEvent;

/// Marker type for keydown — fires on recognized "command keys"
/// in a text field's editor (Enter, Escape, Tab, arrows). AppKit
/// routes these through `control:textView:doCommandBySelector:`;
/// see [`cocoa_dom::KeyEvent`] for the supported key set. AppKit
/// doesn't separate keydown from keyup at this layer; both fire
/// on the same notification.
pub struct KeyDownEvent;
pub const keydown: KeyDownEvent = KeyDownEvent;

/// Marker type for keyup — see [`KeyDownEvent`] for coverage and
/// the keydown/keyup overlap.
pub struct KeyUpEvent;
pub const keyup: KeyUpEvent = KeyUpEvent;

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
    type EventType = cocoa_dom::KeyEvent;
    fn into_pending<F>(handler: F) -> PendingHandler
    where
        F: FnMut(cocoa_dom::KeyEvent) + Send + 'static,
    {
        PendingHandler::KeyDown(Box::new(handler))
    }
}

impl EventDescriptor for KeyUpEvent {
    type EventType = cocoa_dom::KeyEvent;
    fn into_pending<F>(handler: F) -> PendingHandler
    where
        F: FnMut(cocoa_dom::KeyEvent) + Send + 'static,
    {
        PendingHandler::KeyUp(Box::new(handler))
    }
}

// ---------------------------------------------------------------------
// Compile-time control/event compatibility
// ---------------------------------------------------------------------

/// Marker trait — a builder type implements `SupportsEvent<E>` for
/// each event descriptor `E` whose payload makes sense on it.
///
/// Each builder's `.on()` method requires `Self: SupportsEvent<E>`,
/// so `<text_field on:click=...>` (Click on TextField) and
/// `<button on:input=...>` (Input on Button) fail at compile time
/// rather than silently no-oping at runtime.
///
/// Add new pairings as builders gain new events. The default is
/// "no events accepted" — controls without explicit
/// `SupportsEvent<E>` impls reject all `.on()` calls.
pub trait SupportsEvent<E: EventDescriptor> {}

// ---------------------------------------------------------------------
// PendingHandler — typed wrapper passed from the builder into build()
// ---------------------------------------------------------------------

/// A user-supplied event handler, tagged with which AppKit hook to
/// install it via. Element builders accumulate a `Vec<PendingHandler>`
/// during construction; `Render::build` drains it and installs each
/// handler against the constructed `cocoa_dom::Element`.
///
/// Routing per variant (in `apply_to`):
///   * `Click`  — `Element::on_click`            (NSButton target/action)
///   * `Input`  — `Element::on_text_change`      (controlTextDidChange:)
///   * `Change` — `Element::on_text_end_editing` (controlTextDidEndEditing:)
///
/// Builder-side compile-time checks via [`SupportsEvent`] should
/// prevent mismatched (handler, control) pairs from reaching here
/// in the inline `.on()` path. The free-standing `on(...)` →
/// `OnAttribute` → `add_any_attr` spread path is type-erased and
/// unchecked; mismatches there hit the underlying cocoa_dom
/// downcast and silently no-op.
pub enum PendingHandler {
    Click(Box<dyn FnMut() + Send + 'static>),
    Input(Box<dyn FnMut(String) + Send + 'static>),
    Change(Box<dyn FnMut(String) + Send + 'static>),
    Focus(Box<dyn FnMut() + Send + 'static>),
    Blur(Box<dyn FnMut() + Send + 'static>),
    KeyDown(Box<dyn FnMut(cocoa_dom::KeyEvent) + Send + 'static>),
    KeyUp(Box<dyn FnMut(cocoa_dom::KeyEvent) + Send + 'static>),
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

// Phase 8: dropped the upstream Attribute/NextAttribute/ToTemplate
// impls (SSR machinery). Phase 9: revived AddAnyAttr / ApplyAttr in
// minimal form so `<MyComponent on:click=...>` works — see
// `renderer::view::add_any_attr` for the trait shape.

impl renderer::view::ApplyAttr<crate::Dom> for OnAttribute {
    fn apply_to(self, el: &cocoa_dom::Element) {
        OnAttribute::apply(self, el)
    }
}
