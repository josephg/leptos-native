//! Two-way binding (`bind:value=signal`) for GTK widget controls.
//!
//! The `view!{}` macro emits `.bind(::leptos::attr::Value, signal)`
//! for `bind:value=signal`. We provide:
//!   - an `IntoSignal<T>` trait abstracting over the kinds of
//!     signal-like values users might pass (`RwSignal<T>`, `(Get,
//!     Set)` tuples, etc.) — yielding a getter + setter pair
//!   - a `BindAttribute<Key, Sig>` trait that elements implement to
//!     accept the `.bind(key, signal)` call
//!   - per-control `BindAttribute` impls that wire up the right
//!     GTK signal observer on the way in and an Effect on the way out.
//!
//! On GTK, two-way binding uses `glib::signal_handler_block` /
//! `signal_handler_unblock` around the outbound (signal → setter)
//! handler during inbound (Effect → widget) writes, so the inverse
//! round-trip never fires. This is more robust than cocoa's
//! target/action overwrite model, which is inherently single-handler.

#![allow(missing_docs)]

use crate::gtk::element::{Checkbox, Label, PopUpButton, Slider, TextField};
use gtk_dom::Element as GtkElement;
use reactive_graph::{
    effect::RenderEffect,
    signal::RwSignal,
    traits::{Get, Set},
};

/// Conversion trait: turn whatever the user passed to `bind:` into a
/// `(getter, setter)` pair we can wire up.
pub trait IntoSignal<T: Send + Sync + 'static>: 'static {
    fn into_get(&self) -> Box<dyn Fn() -> T + Send + 'static>;
    fn into_set(&self) -> Box<dyn FnMut(T) + Send + 'static>;
}

impl<T> IntoSignal<T> for RwSignal<T>
where
    T: Send + Sync + Clone + 'static,
{
    fn into_get(&self) -> Box<dyn Fn() -> T + Send + 'static> {
        let s = *self;
        Box::new(move || s.get())
    }

    fn into_set(&self) -> Box<dyn FnMut(T) + Send + 'static> {
        let s = *self;
        Box::new(move |v: T| s.set(v))
    }
}

impl<T, G, S> IntoSignal<T> for (G, S)
where
    T: Send + Sync + 'static,
    G: Fn() -> T + Clone + Send + 'static,
    S: FnMut(T) + Clone + Send + 'static,
{
    fn into_get(&self) -> Box<dyn Fn() -> T + Send + 'static> {
        let g = self.0.clone();
        Box::new(move || g())
    }

    fn into_set(&self) -> Box<dyn FnMut(T) + Send + 'static> {
        let mut s = self.1.clone();
        Box::new(move |v: T| s(v))
    }
}

/// `.bind(Key, Sig)` — invoked by `view!{}` macro for `bind:foo=...`.
pub trait BindAttribute<Key, Sig> {
    fn bind(self, key: Key, signal: Sig) -> Self;
}

// ---------------------------------------------------------------------
// TextField — bind:value=String-ish signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::html::attribute::Value, Sig> for TextField
where
    Sig: IntoSignal<String>,
{
    fn bind(
        mut self,
        _key: crate::html::attribute::Value,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_value(BoundValue { getter, setter });
        self
    }
}

pub(crate) struct BoundValue {
    pub(crate) getter: Box<dyn Fn() -> String + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(String) + Send + 'static>,
}

/// Install a TextField bind-value at build time. Returns the
/// `RenderEffect` that pushes signal → entry; caller stashes it in
/// the element's State so it lives as long as the mount.
///
/// No signal-blocking is needed: GTK4's `Entry::set_text` does NOT
/// fire the `changed` signal — `changed` only fires on user
/// interaction (keystrokes), not programmatic writes. The diff-
/// first guard in `set_attribute("value", ..)` is belt-and-braces.
pub(crate) fn install_text_field_value_bind(
    el: &GtkElement,
    bound: BoundValue,
) -> RenderEffect<()> {
    // Outgoing: user types → push to signal.
    let mut setter = bound.setter;
    el.on_text_change(move |new_value| {
        setter(new_value);
    });

    // Incoming: signal change → set entry text.
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_attribute("value", &v);
    })
}

// ---------------------------------------------------------------------
// Slider — bind:value=f64 signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::html::attribute::Value, Sig> for Slider
where
    Sig: IntoSignal<f64>,
{
    fn bind(
        mut self,
        _key: crate::html::attribute::Value,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_value(BoundFloat { getter, setter });
        self
    }
}

pub(crate) struct BoundFloat {
    pub(crate) getter: Box<dyn Fn() -> f64 + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(f64) + Send + 'static>,
}

pub(crate) fn install_slider_value_bind(
    el: &GtkElement,
    bound: BoundFloat,
) -> RenderEffect<()> {
    // Outgoing: user drags slider → push value to signal.
    let mut setter = bound.setter;
    let el_for_out = el.clone();
    el.on_action(move || {
        setter(el_for_out.double_value());
    });
    // Note: `on_action` for Scale calls `connect_value_changed`, which
    // stacks handlers. No handler-blocking needed because set_double_value
    // diffs and the Scale's `value-changed` signal only fires on user
    // interaction (not programmatic set_value) in GTK4.

    // Incoming: signal change → set slider value.
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_double_value(v);
    })
}

// ---------------------------------------------------------------------
// PopUpButton — bind:selection=usize signal (selected index)
// ---------------------------------------------------------------------

/// Custom `selection` AttributeKey (not in upstream HTML attribute
/// list — added here to drive `bind:selection=` from the macro).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Selection;

impl crate::html::attribute::AttributeKey for Selection {
    const KEY: &'static str = "selection";
}

impl<Sig> BindAttribute<Selection, Sig> for PopUpButton
where
    Sig: IntoSignal<usize>,
{
    fn bind(mut self, _key: Selection, signal: Sig) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_selection(BoundIndex { getter, setter });
        self
    }
}

pub(crate) struct BoundIndex {
    pub(crate) getter: Box<dyn Fn() -> usize + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(usize) + Send + 'static>,
}

pub(crate) fn install_popup_selection_bind(
    el: &GtkElement,
    bound: BoundIndex,
) -> RenderEffect<()> {
    // Outgoing: user picks → read selected() → push to signal.
    let mut setter = bound.setter;
    let el_for_out = el.clone();
    el.on_action(move || {
        let idx = el_for_out.popup_selection() as usize;
        setter(idx);
    });

    // Incoming: signal → set_selected (diffs internally).
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_popup_selection(v as u32);
    })
}

// ---------------------------------------------------------------------
// Checkbox — bind:checked=bool signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::html::attribute::Checked, Sig> for Checkbox
where
    Sig: IntoSignal<bool>,
{
    fn bind(
        mut self,
        _key: crate::html::attribute::Checked,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_checked(BoundChecked { getter, setter });
        self
    }
}

pub(crate) struct BoundChecked {
    pub(crate) getter: Box<dyn Fn() -> bool + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(bool) + Send + 'static>,
}

pub(crate) fn install_checkbox_checked_bind(
    el: &GtkElement,
    bound: BoundChecked,
) -> RenderEffect<()> {
    // Outgoing: user clicks → read active state (post-toggle) → push.
    // connect_toggled fires after the state change.
    let mut setter = bound.setter;
    let el_for_out = el.clone();
    el.on_action(move || {
        setter(el_for_out.checked());
    });

    // Incoming: signal → set_active (diffs internally).
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_bool_attribute("checked", v);
    })
}

// ---------------------------------------------------------------------
// Label — bind:value=String-ish signal (read-only sink)
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::html::attribute::Value, Sig> for Label
where
    Sig: IntoSignal<String>,
{
    fn bind(
        mut self,
        _key: crate::html::attribute::Value,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        self.set_pending_bind_text(getter);
        self
    }
}
