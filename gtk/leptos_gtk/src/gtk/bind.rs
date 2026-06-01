//! Two-way binding (`bind:value=signal`) for GTK controls. Mirrors
//! `leptos_cocoa::cocoa::bind` for the controls that exist on both
//! ports (TextField, Slider, PopUpButton, Checkbox, Label).

#![allow(missing_docs)]

use crate::gtk::element::{
    Checkbox, Label, PopUpButton, Slider, TextField,
};
use crate::dom::GtkElem as GtkElement;
use crate::dom::GtkNodeExt;
use reactive_graph::effect::RenderEffect;
use leptos_native::renderer::attr_keys;

// `IntoSignal` (the `RwSignal` / `(getter, setter)` erasure) is shared
// across ports — it lives in core. Re-exported so existing paths
// (`crate::gtk::bind::IntoSignal`) keep resolving.
pub use leptos_native::renderer::IntoSignal;

pub trait BindAttribute<Key, Sig> {
    fn bind(self, key: Key, signal: Sig) -> Self;
}

// ---------------------------------------------------------------------
// TextField — bind:value=String-ish signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<attr_keys::Value, Sig> for TextField
where
    Sig: IntoSignal<String>,
{
    fn bind(mut self, _key: attr_keys::Value, signal: Sig) -> Self {
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

pub(crate) fn install_text_field_value_bind(
    el: &GtkElement,
    bound: BoundValue,
) -> RenderEffect<()> {
    let mut setter = bound.setter;
    el.on_text_change(move |new_value| {
        setter(new_value);
    });

    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_value(&v);
    })
}

// ---------------------------------------------------------------------
// Slider — bind:value=f64 signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<attr_keys::Value, Sig> for Slider
where
    Sig: IntoSignal<f64>,
{
    fn bind(mut self, _key: attr_keys::Value, signal: Sig) -> Self {
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
    let mut setter = bound.setter;
    let el_for_action = el.clone();
    el.on_action(move || {
        setter(el_for_action.double_value());
    });

    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_double_value(v);
    })
}

// ---------------------------------------------------------------------
// PopUpButton — bind:value=usize signal (matches Cocoa naming)
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<attr_keys::Value, Sig> for PopUpButton
where
    Sig: IntoSignal<usize>,
{
    fn bind(mut self, _key: attr_keys::Value, signal: Sig) -> Self {
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
    let mut setter = bound.setter;
    let el_for_action = el.clone();
    el.on_action(move || {
        setter(el_for_action.popup_selection() as usize);
    });

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

impl<Sig> BindAttribute<attr_keys::Checked, Sig> for Checkbox
where
    Sig: IntoSignal<bool>,
{
    fn bind(mut self, _key: attr_keys::Checked, signal: Sig) -> Self {
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
    let mut setter = bound.setter;
    let el_for_action = el.clone();
    el.on_action(move || {
        setter(el_for_action.checked());
    });

    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_checked(v);
    })
}

// ---------------------------------------------------------------------
// Label — bind:value=String signal (read-only sink)
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<attr_keys::Value, Sig> for Label
where
    Sig: IntoSignal<String>,
{
    fn bind(mut self, _key: attr_keys::Value, signal: Sig) -> Self {
        let getter = signal.into_get();
        self.set_pending_bind_text(getter);
        self
    }
}
