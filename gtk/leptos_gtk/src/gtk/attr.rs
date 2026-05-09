//! Attribute-value plumbing for GTK elements. Mirrors
//! `leptos_cocoa::cocoa::attr` minus the AppKit-specific value types.

use reactive_graph::effect::RenderEffect;

/// Either a static value or a closure that produces one reactively.
pub enum MaybeReactive<T: 'static> {
    Static(T),
    Reactive(Box<dyn Fn() -> T + Send + 'static>),
}

pub trait IntoMaybeReactive<T: 'static> {
    fn into_maybe_reactive(self) -> MaybeReactive<T>;
}

/// A dimension value for sizing. Same shape as the cocoa `Dim`.
///
/// - `Px(v)` — fixed length in points.
/// - `Pct(v)` — fraction of the parent's content size, 0.0..=1.0.
/// - `Auto` — let the layout engine decide.
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

    pub fn to_dimension(self) -> gtk_dom::layout::Dimension {
        use gtk_dom::layout::Dimension as D;
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

// Static-value impls.
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

impl IntoMaybeReactive<gtk_dom::layout::FlexDirection>
    for gtk_dom::layout::FlexDirection
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::FlexDirection> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<gtk_dom::layout::JustifyContent>
    for gtk_dom::layout::JustifyContent
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::JustifyContent> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<gtk_dom::layout::AlignItems>
    for gtk_dom::layout::AlignItems
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::AlignItems> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<gtk_dom::layout::FlexWrap>
    for gtk_dom::layout::FlexWrap
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::FlexWrap> {
        MaybeReactive::Static(self)
    }
}

// Closure impls per concrete output type. (We can't write a single
// `impl<T, F> IntoMaybeReactive<T> for F where F: Fn() -> T` because
// it would conflict with the static impls above.)

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

impl<F> IntoMaybeReactive<gtk_dom::layout::FlexDirection> for F
where
    F: Fn() -> gtk_dom::layout::FlexDirection + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::FlexDirection> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<gtk_dom::layout::JustifyContent> for F
where
    F: Fn() -> gtk_dom::layout::JustifyContent + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::JustifyContent> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<gtk_dom::layout::AlignItems> for F
where
    F: Fn() -> gtk_dom::layout::AlignItems + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::AlignItems> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<gtk_dom::layout::FlexWrap> for F
where
    F: Fn() -> gtk_dom::layout::FlexWrap + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::FlexWrap> {
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

/// Drive `apply` whenever the underlying signal(s) change.
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
