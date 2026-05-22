//! `use:directive=param` plumbing — minimal port of
//! `tachys::html::directive`. Each port re-exports the trait with its
//! own `Element` type baked in.
//!
//! `use:foo` (no param) → handler is `Fn(Element)`.
//! `use:foo=param`     → handler is `Fn(Element, P)`.
//!
//! `T` distinguishes the two signatures so the trait can be
//! implemented for both shapes simultaneously.

/// A directive: a `Fn(Element[, P])` callable that runs once after
/// the underlying element is built.
pub trait IntoDirective<E, T: ?Sized, P> {
    /// Run the directive against a live element.
    fn run(self, el: E, param: P);
}

// 0-param: `use:foo` — handler is `Fn(E)`.
impl<E, F> IntoDirective<E, (E,), ()> for F
where
    E: 'static,
    F: FnOnce(E) + Send + 'static,
{
    fn run(self, el: E, _: ()) {
        self(el)
    }
}

// 1-param: `use:foo=param` — handler is `Fn(E, P)`.
impl<E, F, P> IntoDirective<E, ((E, P),), P> for F
where
    E: 'static,
    F: FnOnce(E, P) + Send + 'static,
    P: 'static,
{
    fn run(self, el: E, param: P) {
        self(el, param)
    }
}

/// Pack a `(handler, param)` directive into the `FnOnce(&E)` boxed
/// shape that builders' `directives: Vec<...>` field stores. Each
/// builder's `Render::build` later drains the Vec via [`run_all`]
/// after constructing the underlying element.
pub fn pack<E, D, T, P>(
    handler: D,
    param: P,
) -> Box<dyn FnOnce(E) + Send + 'static>
where
    E: Copy + Send + 'static,
    D: IntoDirective<E, T, P> + Send + 'static,
    P: Send + 'static,
    T: ?Sized + 'static,
{
    Box::new(move |el: E| {
        handler.run(el, param);
    })
}

/// Drain a builder's directives Vec, calling each closure with `&el`.
pub fn run_all<E>(
    directives: Vec<Box<dyn FnOnce(E) + Send + 'static>>,
    el: E,
) where E: Copy {
    for d in directives {
        d(el);
    }
}
