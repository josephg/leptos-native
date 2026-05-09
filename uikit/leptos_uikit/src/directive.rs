//! Directive plumbing — minimal port of `tachys::html::directive` for
//! UIKit. `use:directive=param` lets users attach closures that
//! receive the live `ios_dom::Element` after build, with optional param.

use ios_dom::Element;

pub trait IntoDirective<T: ?Sized, P> {
    fn run(self, el: Element, param: P);
}

impl<F> IntoDirective<(Element,), ()> for F
where
    F: FnOnce(Element) + Send + 'static,
{
    fn run(self, el: Element, _: ()) {
        self(el)
    }
}

impl<F, P> IntoDirective<((Element, P),), P> for F
where
    F: FnOnce(Element, P) + Send + 'static,
    P: 'static,
{
    fn run(self, el: Element, param: P) {
        self(el, param)
    }
}
