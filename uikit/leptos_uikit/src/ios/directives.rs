//! `use:directive=param` macro plumbing for iOS builders.
//! Port from `tachys::cocoa::directives`. Currently unreferenced —
//! the iOS builders have no `.directive()` method yet (audit issue
//! 5d). Kept here so that wiring it up later is a one-side change.

#![allow(dead_code)]

use ios_dom::Element;
use crate::directive::IntoDirective;

pub(crate) fn pack<D, T, P>(
    handler: D,
    param: P,
) -> Box<dyn FnOnce(&Element) + Send + 'static>
where
    D: IntoDirective<T, P> + Send + 'static,
    P: Send + 'static,
    T: 'static,
{
    Box::new(move |el: &Element| {
        handler.run(el.clone(), param);
    })
}

pub(crate) fn run_all(
    directives: Vec<Box<dyn FnOnce(&Element) + Send + 'static>>,
    el: &Element,
) {
    for d in directives {
        d(el);
    }
}
