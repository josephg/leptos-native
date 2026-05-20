//! Macro-facing facade for `tachys::html::event` on Linux.
//!
//! Mirrors `leptos_cocoa::event_macos`. The `view!{}` macro emits
//! paths like
//! `::leptos_native::tachys::html::event::on(::leptos_native::tachys::html::event::click, handler)`.
//! On GTK we map this to our gtk_dom event-handler infrastructure
//! (signal connections on `Button::clicked`, `Entry::changed`, etc.).

#![allow(non_upper_case_globals, missing_docs)]

// ---------------------------------------------------------------------
// Event marker types and descriptors
// ---------------------------------------------------------------------

/// Marker type for the click event (Button "clicked" signal).
pub struct ClickEvent;
pub const click: ClickEvent = ClickEvent;

/// Marker type for the input event — fires on every text change as
/// the user types in a text field. GTK signal: `Entry::changed`.
pub struct InputEvent;
pub const input: InputEvent = InputEvent;

/// Marker type for the change event — fires whenever a control's
/// value changes. Universal across controls; payload is `()`
/// because the new value lives in the bound signal.
///
/// For text fields: same as `on:input`, with `()` payload. For
/// "fires on Return / blur" semantics see [`CommitEvent`].
pub struct ChangeEvent;
pub const change: ChangeEvent = ChangeEvent;

/// Marker type for the commit event — text-field specific. Fires
/// when text editing commits (Return key, focus loss). GTK signal:
/// `Entry::activate`. Payload is the committed text.
pub struct CommitEvent;
pub const commit: CommitEvent = CommitEvent;

/// Marker type for the focus event.
pub struct FocusEvent;
pub const focus: FocusEvent = FocusEvent;

/// Marker type for the blur event.
pub struct BlurEvent;
pub const blur: BlurEvent = BlurEvent;

/// Marker type for the menu-item `action` event — fires when the
/// user activates a `<menu_item>` (click, keyboard shortcut,
/// accessibility). GTK: `gio::SimpleAction::activate`.
///
/// Only `<menu_item>` accepts this event. The
/// `PendingHandler::Action` variant is consumed by the
/// `MenuItem` builder's Render impl (it goes to
/// `gtk_dom::menu::MenuItem::set_action`), not by
/// `PendingHandler::apply_to` (which targets gtk_dom Elements).
pub struct ActionEvent;
pub const action: ActionEvent = ActionEvent;

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
    type EventType = ();
    fn into_pending<F>(mut handler: F) -> PendingHandler
    where
        F: FnMut(()) + Send + 'static,
    {
        PendingHandler::Change(Box::new(move || handler(())))
    }
}

impl EventDescriptor for CommitEvent {
    type EventType = String;
    fn into_pending<F>(handler: F) -> PendingHandler
    where
        F: FnMut(String) + Send + 'static,
    {
        PendingHandler::Commit(Box::new(handler))
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

impl EventDescriptor for ActionEvent {
    type EventType = ();
    fn into_pending<F>(mut handler: F) -> PendingHandler
    where
        F: FnMut(()) + Send + 'static,
    {
        PendingHandler::Action(Box::new(move || handler(())))
    }
}

// ---------------------------------------------------------------------
// Compile-time control/event compatibility
// ---------------------------------------------------------------------

/// Marker trait — a builder type implements `SupportsEvent<E>` for
/// each event descriptor `E` whose payload makes sense on it.
pub trait SupportsEvent<E: EventDescriptor> {}

// ---------------------------------------------------------------------
// PendingHandler — typed wrapper passed from the builder into build()
// ---------------------------------------------------------------------

pub enum PendingHandler {
    Click(Box<dyn FnMut() + Send + 'static>),
    Change(Box<dyn FnMut() + Send + 'static>),
    Input(Box<dyn FnMut(String) + Send + 'static>),
    Commit(Box<dyn FnMut(String) + Send + 'static>),
    Focus(Box<dyn FnMut() + Send + 'static>),
    Blur(Box<dyn FnMut() + Send + 'static>),
    /// Menu-item activation — see [`ActionEvent`]. Consumed by the
    /// menu_item builder rather than by `apply_to`.
    Action(Box<dyn FnMut() + Send + 'static>),
}

impl PendingHandler {
    /// Install this handler against `el`. No-ops if the underlying
    /// GTK widget doesn't support the event.
    pub fn apply_to(self, el: &gtk_dom::Node) {
        match self {
            PendingHandler::Click(cb) => el.on_click(cb),
            PendingHandler::Change(cb) => el.on_value_change(cb),
            PendingHandler::Input(cb) => el.on_text_change(cb),
            PendingHandler::Commit(cb) => el.on_text_end_editing(cb),
            PendingHandler::Focus(cb) => el.on_text_focus(cb),
            PendingHandler::Blur(cb) => el.on_text_blur(cb),
            PendingHandler::Action(_) => panic!(
                "on:action handler reached PendingHandler::apply_to. \
                 on:action is only valid on <menu_item>, and the \
                 menu_item builder consumes the handler directly."
            ),
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
    pub fn apply(mut self, el: &gtk_dom::Node) {
        if let Some(h) = self.handler.take() {
            h.apply_to(el);
        }
    }

    pub fn take_pending(mut self) -> Option<PendingHandler> {
        self.handler.take()
    }
}

impl renderer::view::ApplyAttr<crate::Dom> for OnAttribute {
    fn apply_to(self, el: &gtk_dom::Node) {
        OnAttribute::apply(self, el)
    }
}
