//! Attribute-value plumbing for Cocoa elements.
//!
//! Builder methods like `.title(...)` accept anything that implements
//! [`IntoMaybeReactive<T>`]. The two impls of interest:
//!
//! - **`T` itself** — a static value. Wrapped as `MaybeReactive::Static`.
//! - **`F: Fn() -> T`** — a closure. Wrapped as `MaybeReactive::Reactive`.
//!   At build time we register a [`RenderEffect`] that re-runs the
//!   closure whenever any signal it reads changes, and updates the
//!   underlying NSView property each time.
//!
//! The `RenderEffect` is owned by the element's `State` so it lives
//! exactly as long as the element is mounted.

use reactive_graph::effect::RenderEffect;

/// Either a static value or a closure that produces one reactively.
///
/// The closure is `Send` so that `MaybeReactive<T>` itself is `Send`,
/// which is required by leptos's `IntoView` blanket impl. Most user
/// closures are Send already (reactive_graph signals are Send).
///
/// `Fn` (not `FnMut`): we only ever READ the value through this
/// closure — `RenderEffect` re-runs the closure on each signal
/// change to fetch a fresh value, never mutates closure state.
pub enum MaybeReactive<T: 'static> {
    Static(T),
    Reactive(Box<dyn Fn() -> T + Send + 'static>),
}

/// Conversion trait so attribute setters can take either form
/// transparently.
pub trait IntoMaybeReactive<T: 'static> {
    fn into_maybe_reactive(self) -> MaybeReactive<T>;
}

/// A dimension value for sizing (`width`, `height`, `min_width`,
/// `max_width`, `min_height`, `max_height`). Mirrors Taffy's
/// `Dimension` but with a more compact constructor surface.
///
/// - `Px(v)` — fixed length in points.
/// - `Pct(v)` — fraction of the parent's content width/height,
///   `0.0..=1.0`. (`Pct(1.0)` = 100%.)
/// - `Auto` — let the layout engine decide (Taffy's default).
///
/// `From<f32>` constructs a `Px` so existing call sites that pass
/// raw floats keep working — `width(520.0)` and `width(Dim::pct(0.5))`
/// are both valid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dim {
    Px(f32),
    Pct(f32),
    Auto,
}

impl Dim {
    pub const fn px(v: f32) -> Self {
        Self::Px(v)
    }
    pub const fn pct(v: f32) -> Self {
        Self::Pct(v)
    }
    pub const AUTO: Self = Self::Auto;

    pub fn to_dimension(self) -> cocoa_dom::layout::Dimension {
        use cocoa_dom::layout::Dimension as D;
        match self {
            Self::Px(v) => D::length(v),
            Self::Pct(v) => D::percent(v),
            Self::Auto => D::auto(),
        }
    }
}

impl From<f32> for Dim {
    fn from(v: f32) -> Self {
        Self::Px(v)
    }
}

// Static-value impls. `&str` and `String` have explicit impls so
// callers can pass them without `.to_string()`.
impl IntoMaybeReactive<String> for String {
    fn into_maybe_reactive(self) -> MaybeReactive<String> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<String> for &str {
    fn into_maybe_reactive(self) -> MaybeReactive<String> {
        MaybeReactive::Static(self.to_owned())
    }
}

impl IntoMaybeReactive<bool> for bool {
    fn into_maybe_reactive(self) -> MaybeReactive<bool> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<i32> for i32 {
    fn into_maybe_reactive(self) -> MaybeReactive<i32> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<f64> for f64 {
    fn into_maybe_reactive(self) -> MaybeReactive<f64> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<f32> for f32 {
    fn into_maybe_reactive(self) -> MaybeReactive<f32> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<Dim> for Dim {
    fn into_maybe_reactive(self) -> MaybeReactive<Dim> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<Dim> for f32 {
    fn into_maybe_reactive(self) -> MaybeReactive<Dim> {
        MaybeReactive::Static(Dim::Px(self))
    }
}

impl<F> IntoMaybeReactive<Dim> for F
where
    F: Fn() -> Dim + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<Dim> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl IntoMaybeReactive<usize> for usize {
    fn into_maybe_reactive(self) -> MaybeReactive<usize> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<cocoa_dom::layout::FlexDirection>
    for cocoa_dom::layout::FlexDirection
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::layout::FlexDirection> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<cocoa_dom::layout::JustifyContent>
    for cocoa_dom::layout::JustifyContent
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::layout::JustifyContent> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<cocoa_dom::layout::AlignItems>
    for cocoa_dom::layout::AlignItems
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::layout::AlignItems> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<cocoa_dom::layout::FlexWrap>
    for cocoa_dom::layout::FlexWrap
{
    fn into_maybe_reactive(self) -> MaybeReactive<cocoa_dom::layout::FlexWrap> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<cocoa_dom::Color> for cocoa_dom::Color {
    fn into_maybe_reactive(self) -> MaybeReactive<cocoa_dom::Color> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<cocoa_dom::Date> for cocoa_dom::Date {
    fn into_maybe_reactive(self) -> MaybeReactive<cocoa_dom::Date> {
        MaybeReactive::Static(self)
    }
}

// AppKit enum types — Copy enums, used for `alignment`,
// `segment_style`, `date_picker_style` builder attrs.
impl IntoMaybeReactive<cocoa_dom::NSTextAlignment>
    for cocoa_dom::NSTextAlignment
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::NSTextAlignment> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<cocoa_dom::NSSegmentStyle>
    for cocoa_dom::NSSegmentStyle
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::NSSegmentStyle> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<cocoa_dom::NSDatePickerStyle>
    for cocoa_dom::NSDatePickerStyle
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::NSDatePickerStyle> {
        MaybeReactive::Static(self)
    }
}

// Closure impl. We avoid `impl<T, F> IntoMaybeReactive<T> for F` (that
// would conflict with the static impls above) by writing one closure
// impl per concrete output type. This is enough for the small set of
// attribute types we currently support.
impl<F> IntoMaybeReactive<String> for F
where
    F: Fn() -> String + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<String> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<bool> for F
where
    F: Fn() -> bool + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<bool> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<f64> for F
where
    F: Fn() -> f64 + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<f64> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<f32> for F
where
    F: Fn() -> f32 + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<f32> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<cocoa_dom::layout::FlexDirection> for F
where
    F: Fn() -> cocoa_dom::layout::FlexDirection + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::layout::FlexDirection> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<cocoa_dom::layout::JustifyContent> for F
where
    F: Fn() -> cocoa_dom::layout::JustifyContent + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::layout::JustifyContent> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<cocoa_dom::layout::AlignItems> for F
where
    F: Fn() -> cocoa_dom::layout::AlignItems + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::layout::AlignItems> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<cocoa_dom::layout::FlexWrap> for F
where
    F: Fn() -> cocoa_dom::layout::FlexWrap + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<cocoa_dom::layout::FlexWrap> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<usize> for F
where
    F: Fn() -> usize + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<usize> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<cocoa_dom::Color> for F
where
    F: Fn() -> cocoa_dom::Color + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<cocoa_dom::Color> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<cocoa_dom::Date> for F
where
    F: Fn() -> cocoa_dom::Date + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<cocoa_dom::Date> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<cocoa_dom::NSTextAlignment> for F
where
    F: Fn() -> cocoa_dom::NSTextAlignment + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::NSTextAlignment> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<cocoa_dom::NSSegmentStyle> for F
where
    F: Fn() -> cocoa_dom::NSSegmentStyle + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::NSSegmentStyle> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<cocoa_dom::NSDatePickerStyle> for F
where
    F: Fn() -> cocoa_dom::NSDatePickerStyle + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<cocoa_dom::NSDatePickerStyle> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

/// Drives `apply` whenever the underlying signal(s) change.
///
/// For `Static`, calls `apply(value)` once and returns `None`.
/// For `Reactive`, builds a [`RenderEffect`] that calls
/// `apply(closure())` on every reactive run. The effect's internal
/// constructor runs the closure synchronously inside the reactive
/// observer, so the initial value is set before this returns.
pub fn install<T: 'static>(
    value: MaybeReactive<T>,
    mut apply: impl FnMut(T) + 'static,
) -> Option<RenderEffect<()>> {
    match value {
        MaybeReactive::Static(v) => {
            apply(v);
            None
        }
        MaybeReactive::Reactive(f) => {
            let effect = RenderEffect::new(move |_prev| {
                let v = f();
                apply(v);
            });
            Some(effect)
        }
    }
}
