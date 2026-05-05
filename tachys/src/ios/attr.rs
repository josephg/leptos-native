//! Attribute-value plumbing for iOS elements.
//! Port of `tachys::cocoa::attr`.

use reactive_graph::effect::RenderEffect;

pub enum MaybeReactive<T: 'static> {
    Static(T),
    Reactive(Box<dyn Fn() -> T + Send + 'static>),
}

pub trait IntoMaybeReactive<T: 'static> {
    fn into_maybe_reactive(self) -> MaybeReactive<T>;
}

// Static-value impls per concrete type.
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

impl IntoMaybeReactive<usize> for usize {
    fn into_maybe_reactive(self) -> MaybeReactive<usize> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<ios_dom::Color> for ios_dom::Color {
    fn into_maybe_reactive(self) -> MaybeReactive<ios_dom::Color> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<ios_dom::Date> for ios_dom::Date {
    fn into_maybe_reactive(self) -> MaybeReactive<ios_dom::Date> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<ios_dom::NSTextAlignment>
    for ios_dom::NSTextAlignment
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<ios_dom::NSTextAlignment> {
        MaybeReactive::Static(self)
    }
}

// Closure impls — one per concrete output type.
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

impl<F> IntoMaybeReactive<usize> for F
where
    F: Fn() -> usize + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<usize> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<ios_dom::Color> for F
where
    F: Fn() -> ios_dom::Color + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<ios_dom::Color> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<ios_dom::Date> for F
where
    F: Fn() -> ios_dom::Date + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<ios_dom::Date> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<ios_dom::NSTextAlignment> for F
where
    F: Fn() -> ios_dom::NSTextAlignment + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<ios_dom::NSTextAlignment> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

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
