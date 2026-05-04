//! Macro-facing facade for `tachys::html::event` on Linux/GTK.
//!
//! The `view!{}` macro emits paths like
//! `::leptos::tachys::html::event::on(::leptos::tachys::html::event::click, handler)`
//! to wire up event listeners. On Linux we map this to GTK signal
//! connections (connect_clicked for buttons, connect_changed for
//! entries, etc.).

#![allow(non_upper_case_globals, missing_docs)]

// ---------------------------------------------------------------------
// Event marker types and descriptors
// ---------------------------------------------------------------------

/// Marker type for the click event (gtk::Button "clicked" signal).
pub struct ClickEvent;
pub const click: ClickEvent = ClickEvent;

/// Marker type for the input event — fires on every text change as
/// the user types. GTK equivalent: `Entry::connect_changed`.
pub struct InputEvent;
pub const input: InputEvent = InputEvent;

/// Marker type for the change event — fires when text editing
/// commits (return key). GTK equivalent: `Entry::connect_activate`.
pub struct ChangeEvent;
pub const change: ChangeEvent = ChangeEvent;

/// Each event descriptor knows its payload type ([`EventType`]) and
/// how to package a user-supplied handler into a [`PendingHandler`]
/// the element can install in `Render::build`.
pub trait EventDescriptor {
    type EventType;

    fn into_pending<F>(handler: F) -> PendingHandler
    where
        F: FnMut(Self::EventType) + Send + 'static;
}

/// Marker trait: an element declares it supports an event by
/// implementing this.
pub trait SupportsEvent<E> {}

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

pub enum PendingHandler {
    Click(Box<dyn FnMut() + Send + 'static>),
    Input(Box<dyn FnMut(String) + Send + 'static>),
    Change(Box<dyn FnMut(String) + Send + 'static>),
}

impl PendingHandler {
    pub fn apply_to(self, el: &gtk_dom::Element) {
        match self {
            PendingHandler::Click(cb) => el.on_click(cb),
            PendingHandler::Input(cb) => el.on_text_change(cb),
            PendingHandler::Change(cb) => el.on_text_activate(cb),
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
    pub fn apply(mut self, el: &gtk_dom::Element) {
        if let Some(h) = self.handler.take() {
            h.apply_to(el);
        }
    }

    pub fn take_pending(mut self) -> Option<PendingHandler> {
        self.handler.take()
    }
}
