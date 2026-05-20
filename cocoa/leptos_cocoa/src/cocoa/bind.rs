//! Two-way binding (`bind:value=signal`) for NSControl-derived
//! elements.
//!
//! The `view!{}` macro emits `.bind(::leptos_native::attr::Value, signal)`
//! for `bind:value=signal`. We provide:
//!   - an `IntoSignal<T>` trait abstracting over the kinds of
//!     signal-like values users might pass (`RwSignal<T>`, `(Get,
//!     Set)` tuples, etc.) — yielding a getter + setter pair
//!   - a `BindAttribute<Key, Sig>` trait that elements implement to
//!     accept the `.bind(key, signal)` call
//!   - per-control `BindAttribute` impls that wire up the right
//!     AppKit observer on the way in and an Effect on the way out.

#![allow(missing_docs)]

use crate::cocoa::element::{
    Checkbox, ColorWell, DatePicker, PopUpButton, SegmentedControl,
    Slider, Stack, Stepper, TextField, TextView,
};
use cocoa_dom::Element as CocoaElement;
use objc2::rc::Retained;
use reactive_graph::{
    effect::RenderEffect,
    signal::RwSignal,
    traits::{Get, Set},
};

/// Downcast the element's NSView to a specific subclass at install
/// time. Returns `Retained<T>` so the install closures can hold a
/// typed handle without re-downcasting on every Effect / action
/// invocation. Per `MEMORY_POLICY.md` §3 and §7, install closures
/// must capture a typed `Retained<NSSubclass>` rather than a
/// `CocoaElement` clone — that keeps the closure outside the
/// `Node → bundle → handler` Rc graph.
///
/// Panics if the underlying view isn't a `T` — this only happens
/// when an `install_*` function is called on the wrong element
/// type, which is a programming error at the framework level (the
/// builders are typed and route to the right install).
fn typed_view<T>(el: &CocoaElement, ctx: &'static str) -> Retained<T>
where
    T: objc2::Message + objc2::DowncastTarget,
{
    el
        .ns_view_retained()
        .downcast::<T>()
        .unwrap_or_else(|_| {
            panic!(
                "{ctx}: NSView is not a {}",
                std::any::type_name::<T>(),
            )
        })
}

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

impl<Sig> BindAttribute<crate::keys::Value, Sig> for TextField
where
    Sig: IntoSignal<String>,
{
    fn bind(
        mut self,
        _key: crate::keys::Value,
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

    // Incoming: signal change → set field's stringValue. Routed
    // through `Element::set_string_attribute` (rather than a typed
    // `Retained<NSTextField>` capture) for two reasons:
    //   1. `set_string_attribute` also calls `schedule_relayout`
    //      on content change — NSTextField with
    //      intrinsic-width-from-content needs a layout pass after
    //      the new text width settles.
    //   2. The closure stays cycle-safe per `MEMORY_POLICY.md` §3
    //      because this `RenderEffect` lives on
    //      `ElementState::_effects` (drops with the state) — it
    //      isn't installed into the Node's handler bundle, so
    //      capturing `CocoaElement` here doesn't form an Rc cycle.
    // Incoming: signal change → set field's stringValue. Routed
    // through `Element::set_string_attribute` (rather than a typed
    // `Retained<NSTextField>` capture) for two reasons:
    //   1. `set_string_attribute` also calls `schedule_relayout`
    //      on content change — NSTextField with
    //      intrinsic-width-from-content needs a layout pass after
    //      the new text width settles.
    //   2. The closure stays cycle-safe per `MEMORY_POLICY.md` §3
    //      because this `RenderEffect` lives on
    //      `ElementState::_effects` (drops with the state) — it
    //      isn't installed into the Node's handler bundle, so
    //      capturing `CocoaElement` here doesn't form an Rc cycle.
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

pub(crate) struct BoundFloat {
    pub(crate) getter: Box<dyn Fn() -> f64 + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(f64) + Send + 'static>,
}

// TextView reuses the existing BoundValue — same `String` shape
// as TextField, just routed through NSTextView's delegate
// (`textDidChange:`) instead of NSControlTextEditingDelegate.
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
    el: &CocoaElement,
    bound: BoundValue,
) -> RenderEffect<()> {
    // Outgoing: user types → push to signal.
    let mut setter = bound.setter;
    el.on_text_view_change(move |new_value| setter(new_value));

    // Incoming: signal change → set NSTextView's string. Routed
    // through `Element::set_string_attribute` rather than a
    // typed `Retained<NSTextView>` capture so the incoming write
    // ALSO calls `schedule_relayout` (NSTextView's
    // `intrinsicContentSize` changes with content; without a
    // relayout pass, the Taffy frame can drift). Capturing the
    // bare `Retained<NSTextView>` inline would skip that —
    // observed empirically as a leak / hang in the fuzzer.
    //
    // The closure stays cycle-safe per `MEMORY_POLICY.md` §3
    // because this `RenderEffect` lives on
    // `ElementState::_effects` (drops with the state) — it isn't
    // installed into the Node's handler bundle, so capturing
    // `CocoaElement` here doesn't form an Rc cycle.
    let getter = bound.getter;
    let el_for_set = el.clone();
    RenderEffect::new(move |_prev| {
        let s = getter();
        el_for_set.set_value(&s);
    })
}

// Stepper reuses the existing BoundFloat — the value is f64 and
// the wiring shape (signal ↔ doubleValue + target/action) is
// identical to slider's.
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
    el: &CocoaElement,
    bound: BoundFloat,
) -> RenderEffect<()> {
    use objc2_app_kit::NSStepper;
    let stepper = typed_view::<NSStepper>(el, "install_stepper_value_bind");

    // Outgoing: AppKit fires target/action; read doubleValue → push.
    let mut setter = bound.setter;
    let stepper_out = stepper.clone();
    el.on_action(move || setter(stepper_out.doubleValue()));

    // Incoming: signal → setDoubleValue, with diff guard.
    let getter = bound.getter;
    RenderEffect::new(move |_prev| {
        let v = getter();
        if (stepper.doubleValue() - v).abs() > f64::EPSILON {
            stepper.setDoubleValue(v);
        }
    })
}

// ---------------------------------------------------------------------
// DatePicker — bind:value=Date signal
// ---------------------------------------------------------------------

pub(crate) struct BoundDate {
    pub(crate) getter: Box<dyn Fn() -> cocoa_dom::Date + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(cocoa_dom::Date) + Send + 'static>,
}

impl<Sig> BindAttribute<crate::keys::Value, Sig> for DatePicker
where
    Sig: IntoSignal<cocoa_dom::Date>,
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
    el: &CocoaElement,
    bound: BoundDate,
) -> RenderEffect<()> {
    use objc2_app_kit::NSDatePicker;
    let picker =
        typed_view::<NSDatePicker>(el, "install_date_picker_bind");

    // Outgoing.
    let mut setter = bound.setter;
    let picker_out = picker.clone();
    el.on_action(move || {
        let d = picker_out.dateValue();
        setter(cocoa_dom::Date::from_nsdate(&d));
    });

    // Incoming: signal → setDateValue. Construct an NSDate from
    // our cocoa_dom::Date and compare via NSDate equality before
    // mutating.
    let getter = bound.getter;
    RenderEffect::new(move |_prev| {
        let d = getter();
        let nsd = d.to_nsdate();
        let current = picker.dateValue();
        if !current.isEqualToDate(&nsd) {
            picker.setDateValue(&nsd);
        }
    })
}

pub(crate) fn install_slider_value_bind(
    el: &CocoaElement,
    bound: BoundFloat,
) -> RenderEffect<()> {
    // NSSlider extends NSControl directly; capture the typed
    // Retained<NSSlider> for both outgoing (target/action fires on
    // every drag step thanks to setContinuous(true) at create
    // time) and incoming (setDoubleValue).
    use objc2_app_kit::NSSlider;
    let slider = typed_view::<NSSlider>(el, "install_slider_value_bind");

    let mut setter = bound.setter;
    let slider_out = slider.clone();
    el.on_action(move || setter(slider_out.doubleValue()));

    let getter = bound.getter;
    RenderEffect::new(move |_prev| {
        let v = getter();
        if (slider.doubleValue() - v).abs() > f64::EPSILON {
            slider.setDoubleValue(v);
        }
    })
}

// ---------------------------------------------------------------------
// PopUpButton / SegmentedControl — bind:value=usize signal
// (selected index)
// ---------------------------------------------------------------------
//
// Earlier revisions exposed a custom `bind:selection=` key for these
// two controls. P1 of API_REVIEW.md folded them under `bind:value=`:
// the type of the signal (`usize`) is enough to disambiguate from
// the other `bind:value=` impls.

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

impl<Sig> BindAttribute<crate::keys::Value, Sig> for SegmentedControl
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

pub(crate) struct BoundIndex {
    pub(crate) getter: Box<dyn Fn() -> usize + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(usize) + Send + 'static>,
}

pub(crate) fn install_popup_selection_bind(
    el: &CocoaElement,
    bound: BoundIndex,
) -> RenderEffect<()> {
    use objc2_app_kit::NSPopUpButton;
    // NSPopUpButton IS-A NSButton, so on_click hooks the same
    // target/action wiring as a button.
    let popup =
        typed_view::<NSPopUpButton>(el, "install_popup_selection_bind");

    let mut setter = bound.setter;
    let popup_out = popup.clone();
    el.on_click(move || {
        let idx = popup_out.indexOfSelectedItem();
        if idx >= 0 {
            setter(idx as usize);
        }
    });

    let getter = bound.getter;
    RenderEffect::new(move |_prev| {
        let v = getter() as isize;
        if popup.indexOfSelectedItem() != v {
            popup.selectItemAtIndex(v);
        }
    })
}

// ---------------------------------------------------------------------
// ColorWell — bind:value=Color signal
// ---------------------------------------------------------------------

pub(crate) struct BoundColor {
    pub(crate) getter: Box<dyn Fn() -> cocoa_dom::Color + Send + 'static>,
    pub(crate) setter: Box<dyn FnMut(cocoa_dom::Color) + Send + 'static>,
}

impl<Sig> BindAttribute<crate::keys::Value, Sig> for ColorWell
where
    Sig: IntoSignal<cocoa_dom::Color>,
{
    fn bind(
        mut self,
        _key: crate::keys::Value,
        signal: Sig,
    ) -> Self {
        let getter = signal.into_get();
        let setter = signal.into_set();
        self.set_pending_bind_color(BoundColor { getter, setter });
        self
    }
}

pub(crate) fn install_color_well_bind(
    el: &CocoaElement,
    bound: BoundColor,
) -> RenderEffect<()> {
    use objc2_app_kit::NSColorWell;
    use objc2_foundation::NSObjectProtocol;
    let well = typed_view::<NSColorWell>(el, "install_color_well_bind");

    let mut setter = bound.setter;
    let well_out = well.clone();
    el.on_action(move || {
        let c = well_out.color();
        if let Some(parsed) = cocoa_dom::Color::from_nscolor(&c) {
            setter(parsed);
        }
    });

    let getter = bound.getter;
    RenderEffect::new(move |_prev| {
        let c = getter();
        let nscolor = c.to_nscolor();
        // NSColor equality via isEqual (handles colorspace
        // conversion); diff to skip redundant writes.
        if !well.color().isEqual(Some(&nscolor)) {
            well.setColor(&nscolor);
        }
    })
}

pub(crate) fn install_segmented_selection_bind(
    el: &CocoaElement,
    bound: BoundIndex,
) -> RenderEffect<()> {
    use objc2_app_kit::NSSegmentedControl;
    let seg = typed_view::<NSSegmentedControl>(
        el,
        "install_segmented_selection_bind",
    );

    let mut setter = bound.setter;
    let seg_out = seg.clone();
    el.on_action(move || {
        let idx = seg_out.selectedSegment();
        if idx >= 0 {
            setter(idx as usize);
        }
    });

    let getter = bound.getter;
    RenderEffect::new(move |_prev| {
        let v = getter() as isize;
        if seg.selectedSegment() != v {
            seg.setSelectedSegment(v);
        }
    })
}

// ---------------------------------------------------------------------
// Checkbox — bind:checked=bool signal
// ---------------------------------------------------------------------

impl<Sig> BindAttribute<crate::keys::Checked, Sig> for Checkbox
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
    use objc2_app_kit::{
        NSButton, NSControlStateValueOff, NSControlStateValueOn,
    };
    // Checkboxes are NSButtons configured with .switch type.
    let button = typed_view::<NSButton>(el, "install_checkbox_checked_bind");

    // Outgoing: user clicks → read button state → push to signal.
    // NSButton's target/action fires AFTER the state has been
    // updated by AppKit's click handling, so `state()` returns the
    // new value.
    let mut setter = bound.setter;
    let button_out = button.clone();
    el.on_click(move || {
        setter(button_out.state() == NSControlStateValueOn);
    });

    // Incoming: signal change → set button state. Diff before
    // writing — the Effect can re-run after the click → setter
    // cycle wrote back the same value, and a redundant
    // setState() would trigger an unwanted redraw.
    let getter = bound.getter;
    RenderEffect::new(move |_prev| {
        let v = getter();
        let new_state = if v {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        };
        if button.state() != new_state {
            button.setState(new_state);
        }
    })
}

// Label deliberately does NOT have a `bind:value=` impl. A label is a
// read-only sink — the `bind:` syntax implies two-way binding, which
// doesn't apply. Users that want signal-driven label text should write
// `<label>{move || sig.get()}</label>` directly.

// ---------------------------------------------------------------------
// Stack — bind:mouse_hover=signal (one-way: framework → app)
// ---------------------------------------------------------------------
//
// Cocoa-specific: hover is OS-driven, not app-driven, so we only
// install the setter side. The signal is `set(true)` on
// mouseEntered: and `set(false)` on mouseExited:; writes from the
// app into the signal don't propagate back to AppKit (there's no
// "synthesise hover" path).

impl<Ch, Sig> BindAttribute<crate::keys::MouseHover, Sig> for Stack<Ch>
where
    Sig: IntoSignal<bool>,
{
    fn bind(
        mut self,
        _key: crate::keys::MouseHover,
        signal: Sig,
    ) -> Self {
        let setter = signal.into_set();
        self.set_pending_bind_mouse_hover(setter);
        self
    }
}
