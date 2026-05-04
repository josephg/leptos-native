//! Two-way binding (`bind:value=signal`) for NSControl-derived
//! elements.
//!
//! The `view!{}` macro emits `.bind(::leptos::attr::Value, signal)`
//! for `bind:value=signal`. We provide:
//!   - an `IntoSignal<T>` trait abstracting over the kinds of
//!     signal-like values users might pass (`RwSignal<T>`, `(Get,
//!     Set)` tuples, etc.) — yielding a getter + setter pair
//!   - a `BindAttribute<Key, Sig>` trait that elements implement to
//!     accept the `.bind(key, signal)` call
//!   - per-control `BindAttribute` impls that wire up the right
//!     AppKit observer on the way in and an Effect on the way out.

#![allow(missing_docs)]

use crate::cocoa::element::{Checkbox, Label, PopUpButton, Slider, TextField};
use cocoa_dom::{BoolAttr, Element as CocoaElement, StringAttr};
use reactive_graph::{
    effect::RenderEffect,
    signal::RwSignal,
    traits::{Get, Set},
};

/// Conversion trait: turn whatever the user passed to `bind:` into a
/// `(getter, setter)` pair we can wire up.
///
/// Provided impls:
///  - `RwSignal<T>` — most common case
///  - `(impl Get<Value = T>, impl Set<Value = T>)` — split signals
pub trait IntoSignal<T: Send + Sync + 'static>: 'static {
    /// Returns a getter that reads the current value (subscribes to
    /// changes when called inside an Effect).
    fn into_get(&self) -> Box<dyn Fn() -> T + Send + 'static>;

    /// Returns a setter that updates the underlying signal.
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

// `(getter_fn, setter_fn)` tuple — handy for split signals or
// derived-state controllers where you don't have an `RwSignal`.
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
        // Stash the wiring instructions on the builder; actual
        // installation happens in `TextField::build` (where we have
        // the NSView and can attach the delegate + Effect).
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_value(BoundValue { getter, setter });
        self
    }
}

/// Internal: the bind state held by a TextField builder until build().
pub(crate) struct BoundValue {
    pub(crate) getter: Box<dyn Fn() -> String + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(String) + Send + 'static>,
}

/// Install a TextField bind-value at build time. Returns the
/// `RenderEffect` that pushes signal → view; caller stashes it in the
/// element's State so it lives as long as the mount.
pub(crate) fn install_text_field_value_bind(
    el: &CocoaElement,
    bound: BoundValue,
) -> RenderEffect<()> {
    // Outgoing: user types → push to signal.
    let mut setter = bound.setter;
    el.on_text_change(move |new_value| {
        setter(new_value);
    });

    // Incoming: signal change → set field's stringValue.
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_string_attribute(StringAttr::Value, &v);
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
    el: &CocoaElement,
    bound: BoundFloat,
) -> RenderEffect<()> {
    // Outgoing: AppKit fires target/action on every drag step
    // (setContinuous(true) at create time). Read doubleValue → push.
    // Use `on_action` (NSControl-based) rather than `on_click`
    // (NSButton-based) — NSSlider extends NSControl directly, not
    // NSButton, so the NSButton downcast in on_click would silently
    // drop the wiring.
    let mut setter = bound.setter;
    let el_for_action = el.clone();
    el.on_action(move || {
        setter(el_for_action.double_value());
    });

    // Incoming: signal change → set slider value. set_double_value
    // diffs internally so a no-op write doesn't redraw.
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
    el: &CocoaElement,
    bound: BoundIndex,
) -> RenderEffect<()> {
    // Outgoing: NSPopUpButton fires target/action when the user picks
    // an item. on_click hooks into the same target/action wiring we
    // use for buttons (NSPopUpButton IS-A NSButton).
    let mut setter = bound.setter;
    let el_for_click = el.clone();
    el.on_click(move || {
        let idx = el_for_click.popup_selection();
        if idx >= 0 {
            setter(idx as usize);
        }
    });

    // Incoming: signal → selectItemAtIndex: (with a diff guard inside
    // set_popup_selection).
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_popup_selection(v as isize);
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

/// Internal: the bind state held by a Checkbox builder until build().
pub(crate) struct BoundChecked {
    pub(crate) getter: Box<dyn Fn() -> bool + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(bool) + Send + 'static>,
}

/// Install a checkbox bind-checked at build time. Returns the
/// `RenderEffect` that pushes signal → button state; caller stashes
/// it in the element's State so it lives as long as the mount.
pub(crate) fn install_checkbox_checked_bind(
    el: &CocoaElement,
    bound: BoundChecked,
) -> RenderEffect<()> {
    // Outgoing: user clicks → read button state → push to signal.
    // NSButton's action target/action fires AFTER the state has been
    // updated by AppKit's click handling, so `el.checked()` returns
    // the new state.
    let mut setter = bound.setter;
    let el_for_click = el.clone();
    el.on_click(move || {
        setter(el_for_click.checked());
    });

    // Incoming: signal change → set button state.
    // set_bool_attribute diffs internally so a no-op write (e.g. the
    // Effect re-running after the click → setter cycle wrote back the
    // value the button already shows) doesn't cause a redraw.
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let v = getter();
        el_for_set.set_bool_attribute(BoolAttr::Checked, v);
    })
}

// ---------------------------------------------------------------------
// Label — bind:value=String-ish signal (read-only sink, but useful
// for symmetry).
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
        // For a label, only the get-direction is meaningful.
        // Reuse `.text(closure)` plumbing.
        let getter = signal.into_get();
        self.set_pending_bind_text(getter);
        self
    }
}
