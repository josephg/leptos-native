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

impl IntoMaybeReactive<usize> for usize {
    fn into_maybe_reactive(self) -> MaybeReactive<usize> {
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

impl<F> IntoMaybeReactive<usize> for F
where
    F: Fn() -> usize + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<usize> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

/// Drives `apply` whenever the underlying signal(s) change.
///
/// For `Static`, calls `apply(value)` once and returns `None`.
/// For `Reactive`, builds a `RenderEffect` that calls `apply(closure())`
/// on every reactive run; returns the effect so the caller can keep it
/// alive (drop = unsubscribe). The first run happens synchronously
/// inside the constructor, so the initial value is set before this
/// returns.
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
