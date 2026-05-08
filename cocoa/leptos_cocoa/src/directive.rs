//! Directive plumbing — minimal port of `tachys::html::directive` for
//! cocoa. `use:directive=param` lets users attach closures that receive
//! the live `cocoa_dom::Element` after build, with an optional param.
//!
//! Phase 8: pared back from upstream — dropped the `into_cloneable` /
//! `Cloneable` associated type. Inline `.directive(handler, param)`
//! is the only path examples use, and it doesn't need cloneability.

use cocoa_dom::Element;

/// A directive: a `Fn(Element[, P])` callable that runs once after the
/// underlying element is built. `T` distinguishes 0-arg from 1-arg
/// signatures so the trait can be implemented for both.
pub trait IntoDirective<T: ?Sized, P> {
    /// Run the directive against a live element.
    fn run(self, el: Element, param: P);
}

// 0-param: `use:foo` — handler is `Fn(Element)`.
impl<F> IntoDirective<(Element,), ()> for F
where
    F: FnOnce(Element) + Send + 'static,
{
    fn run(self, el: Element, _: ()) {
        self(el)
    }
}

// 1-param: `use:foo=param` — handler is `Fn(Element, P)`.
impl<F, P> IntoDirective<((Element, P),), P> for F
where
    F: FnOnce(Element, P) + Send + 'static,
    P: 'static,
{
    fn run(self, el: Element, param: P) {
        self(el, param)
    }
}
