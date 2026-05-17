//! Two-way binding (`bind:value=signal`, `bind:checked=signal`) for
//! UIControl-derived elements. Direct port of [`crate::cocoa::bind`].
//!
//! The `view!{}` macro emits `.bind(::leptos::attr::Value, signal)`
//! for `bind:value=signal` (and similarly for `bind:checked`). We
//! provide:
//!   - an [`IntoSignal<T>`] trait abstracting over the kinds of
//!     signal-like values users might pass (`RwSignal<T>`, `(Get,
//!     Set)` tuples, etc.) — yielding a getter + setter pair
//!   - a [`BindAttribute<Key, Sig>`] trait that elements implement
//!     to accept the `.bind(key, signal)` call
//!   - per-control `BindAttribute` impls that wire up the right
//!     UIKit observer on the way in and an [`Effect`](reactive_graph::effect::Effect)
//!     on the way out.

#![allow(missing_docs)]

use crate::ios::element::{
    ColorWell, DatePicker, Label, PopUpButton, SegmentedControl, Slider,
    Stepper, Switch, TextField, TextView,
};
use ios_dom::{BoolAttr, Element as IosElement, StringAttr};
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

/// `.bind(Key, Sig)` — invoked by `view!{}` for `bind:foo=…`.
pub trait BindAttribute<Key, Sig> {
    fn bind(self, key: Key, signal: Sig) -> Self;
}

// ---------------------------------------------------------------------
// Bound* — the wiring-instructions a builder stashes between
// `bind(...)` and `Render::build`.
// ---------------------------------------------------------------------

pub(crate) struct BoundValue {
    pub(crate) getter: Box<dyn Fn() -> String + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(String) + Send + 'static>,
}

pub(crate) struct BoundFloat {
    pub(crate) getter: Box<dyn Fn() -> f64 + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(f64) + Send + 'static>,
}

pub(crate) struct BoundChecked {
    pub(crate) getter: Box<dyn Fn() -> bool + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(bool) + Send + 'static>,
}

pub(crate) struct BoundDate {
    pub(crate) getter:
        Box<dyn Fn() -> ios_dom::Date + Send + 'static>,
    pub(crate) setter:
        Box<dyn FnMut(ios_dom::Date) + Send + 'static>,
}

pub(crate) struct BoundIndex {
    pub(crate) getter: Box<dyn Fn() -> usize + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(usize) + Send + 'static>,
}

// ---------------------------------------------------------------------
// TextField — bind:value=String-ish signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::keys::Value, Sig> for TextField
where
    Sig: IntoSignal<String>,
{
    fn bind(
        mut self,
        _key: crate::keys::Value,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_value(BoundValue { getter, setter });
        self
    }
}

pub(crate) fn install_text_field_value_bind(
    el: &IosElement,
    bound: BoundValue,
) -> RenderEffect<()> {
    // Outgoing: editingChanged (every keystroke / paste / clear).
    let mut setter = bound.setter;
    el.on_text_change(move |new_value| setter(new_value));

    // Incoming: signal change → set field text. set_string_attribute
    // diffs internally so a no-op write doesn't trigger
    // controlTextDidChange ping-pong with the outgoing leg.
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_string_attribute(StringAttr::Value, &v);
    })
}

// ---------------------------------------------------------------------
// Switch — bind:checked=bool signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::keys::Checked, Sig> for Switch
where
    Sig: IntoSignal<bool>,
{
    fn bind(
        mut self,
        _key: crate::keys::Checked,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_checked(BoundChecked { getter, setter });
        self
    }
}

pub(crate) fn install_switch_checked_bind(
    el: &IosElement,
    bound: BoundChecked,
) -> RenderEffect<()> {
    // Outgoing: user toggles → read switch state → push to signal.
    // UIControlEventValueChanged fires AFTER UISwitch.isOn updates,
    // so `el.checked()` returns the new state.
    use objc2_ui_kit::UISwitch;
    let mut setter = bound.setter;
    // Capture Retained<UIView> rather than IosElement — capturing
    // the Element here would form an Rc cycle: closure →
    // IosElement → IosNodeHandlersBundle → ActionTarget → ivars
    // → closure. See `ios_dom::event` module doc.
    let view_for_action = el.ui_view_retained();
    el.on_click(move || {
        let any: &objc2::runtime::AnyObject = (&*view_for_action).as_ref();
        if let Some(s) = any.downcast_ref::<UISwitch>() {
            setter(s.isOn());
        }
    });

    // Incoming: signal → setOn:animated:. set_bool_attribute diffs
    // internally to avoid pinging back through the outgoing leg
    // when the Effect re-fires after setting the value the switch
    // already shows.
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_bool_attribute(BoolAttr::Checked, v);
    })
}

// ---------------------------------------------------------------------
// Slider — bind:value=f64 signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::keys::Value, Sig> for Slider
where
    Sig: IntoSignal<f64>,
{
    fn bind(
        mut self,
        _key: crate::keys::Value,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_value(BoundFloat { getter, setter });
        self
    }
}

pub(crate) fn install_slider_value_bind(
    el: &IosElement,
    bound: BoundFloat,
) -> RenderEffect<()> {
    // Outgoing: UISlider fires UIControlEventValueChanged on every
    // drag step (continuous = true at create time).
    use objc2_ui_kit::UISlider;
    let mut setter = bound.setter;
    // Retained<UIView> capture; see `install_switch_checked_bind`.
    let view_for_action = el.ui_view_retained();
    el.on_click(move || {
        let any: &objc2::runtime::AnyObject = (&*view_for_action).as_ref();
        if let Some(s) = any.downcast_ref::<UISlider>() {
            setter(s.value() as f64);
        }
    });

    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_double_value(v);
    })
}

// ---------------------------------------------------------------------
// Stepper — bind:value=f64 signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::keys::Value, Sig> for Stepper
where
    Sig: IntoSignal<f64>,
{
    fn bind(
        mut self,
        _key: crate::keys::Value,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_value(BoundFloat { getter, setter });
        self
    }
}

pub(crate) fn install_stepper_value_bind(
    el: &IosElement,
    bound: BoundFloat,
) -> RenderEffect<()> {
    use objc2_ui_kit::UIStepper;
    let mut setter = bound.setter;
    // Retained<UIView> capture; see `install_switch_checked_bind`.
    let view_for_action = el.ui_view_retained();
    el.on_click(move || {
        let any: &objc2::runtime::AnyObject = (&*view_for_action).as_ref();
        if let Some(s) = any.downcast_ref::<UIStepper>() {
            setter(s.value());
        }
    });
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_stepper_value(v);
    })
}

// ---------------------------------------------------------------------
// DatePicker — bind:value=Date signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::keys::Value, Sig> for DatePicker
where
    Sig: IntoSignal<ios_dom::Date>,
{
    fn bind(
        mut self,
        _key: crate::keys::Value,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_date(BoundDate { getter, setter });
        self
    }
}

pub(crate) fn install_date_picker_bind(
    el: &IosElement,
    bound: BoundDate,
) -> RenderEffect<()> {
    use objc2_ui_kit::UIDatePicker;
    let mut setter = bound.setter;
    // Retained<UIView> capture; see `install_switch_checked_bind`.
    let view_for_action = el.ui_view_retained();
    el.on_click(move || {
        let any: &objc2::runtime::AnyObject = (&*view_for_action).as_ref();
        if let Some(p) = any.downcast_ref::<UIDatePicker>() {
            let d = p.date();
            setter(ios_dom::Date::from_nsdate(&d));
        }
    });
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let d = getter();
        el_for_set.set_date_picker_value(d);
    })
}

// ---------------------------------------------------------------------
// SegmentedControl — bind:selection=usize signal
// ---------------------------------------------------------------------

/// Custom `selection` AttributeKey — not an HTML attribute, only
/// used to drive `bind:selection=` from the macro. Mirrors the
/// cocoa port's `Selection` key.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Selection;

impl crate::keys::AttributeKey for Selection {
    const KEY: &'static str = "selection";
}

impl<Sig> BindAttribute<Selection, Sig> for SegmentedControl
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

pub(crate) fn install_segmented_selection_bind(
    el: &IosElement,
    bound: BoundIndex,
) -> RenderEffect<()> {
    use objc2_ui_kit::UISegmentedControl;
    let mut setter = bound.setter;
    // Retained<UIView> capture; see `install_switch_checked_bind`.
    let view_for_action = el.ui_view_retained();
    el.on_click(move || {
        let any: &objc2::runtime::AnyObject = (&*view_for_action).as_ref();
        if let Some(s) = any.downcast_ref::<UISegmentedControl>() {
            let idx = s.selectedSegmentIndex();
            if idx >= 0 {
                setter(idx as usize);
            }
        }
    });
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_segmented_selection(v as isize);
    })
}

// ---------------------------------------------------------------------
// PopUpButton — bind:value=usize signal (matches Cocoa naming).
// Outgoing-edge wiring (menu-item tap → signal setter) is handled
// inside the PopUpButton builder's `build()` via `set_popup_items`'s
// on_select callback. We only need to hand the setter over.
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::keys::Value, Sig> for PopUpButton
where
    Sig: IntoSignal<usize>,
{
    fn bind(mut self, _key: crate::keys::Value, signal: Sig) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_selection(BoundIndex { getter, setter });
        self
    }
}

// ---------------------------------------------------------------------
// ColorWell — bind:value=Color signal
// ---------------------------------------------------------------------

pub(crate) struct BoundColor {
    pub(crate) getter: Box<dyn Fn() -> ios_dom::Color + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(ios_dom::Color) + Send + 'static>,
}

impl<Sig> BindAttribute<crate::keys::Value, Sig> for ColorWell
where
    Sig: IntoSignal<ios_dom::Color>,
{
    fn bind(mut self, _key: crate::keys::Value, signal: Sig) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_value(BoundColor { getter, setter });
        self
    }
}

pub(crate) fn install_color_well_value_bind(
    el: &IosElement,
    bound: BoundColor,
) -> RenderEffect<()> {
    let mut setter = bound.setter;
    el.on_color_change(move |c| setter(c));

    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_color_well_value(v);
    })
}

// ---------------------------------------------------------------------
// TextView — bind:value=String-ish signal
//
// UITextView is a UIScrollView subclass, NOT a UIControl, so the
// outgoing leg uses the UITextViewDelegate's `textViewDidChange:`
// fan-out (see `ios_dom::event::on_text_view_change`).
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::keys::Value, Sig> for TextView
where
    Sig: IntoSignal<String>,
{
    fn bind(
        mut self,
        _key: crate::keys::Value,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_value(BoundValue { getter, setter });
        self
    }
}

pub(crate) fn install_text_view_value_bind(
    el: &IosElement,
    bound: BoundValue,
) -> RenderEffect<()> {
    let mut setter = bound.setter;
    el.on_text_view_change(move |new_value| setter(new_value));

    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let s = getter();
        el_for_set.set_string_attribute(StringAttr::Value, &s);
    })
}

// ---------------------------------------------------------------------
// Label — bind:value=String-ish signal (read-only sink, but useful
// for symmetry with the cocoa port).
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::keys::Value, Sig> for Label
where
    Sig: IntoSignal<String>,
{
    fn bind(
        mut self,
        _key: crate::keys::Value,
        signal: Sig,
    ) -> Self {
        // Only the read direction is meaningful for a label; reuse
        // the same `.text(closure)` plumbing.
        let getter = signal.into_get();
        self.set_pending_bind_text(getter);
        self
    }
}
