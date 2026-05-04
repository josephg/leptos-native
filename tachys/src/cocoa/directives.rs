//! `use:directive=param` macro plumbing for cocoa builders.
//!
//! The upstream `tachys::html::directive::IntoDirective` trait
//! and its blanket impls already cover function pointers with
//! one or two parameters where the first is
//! `crate::renderer::types::Element` (which on macOS is
//! `cocoa_dom::Element`). Our job is just to call `handler.run(el,
//! param)` at the right time.
//!
//! Each cocoa element builder has an inherent `.directive(handler,
//! param)` method that boxes the call as a `FnOnce(&Element)` and
//! pushes it onto a directives Vec. `Render::build` drains the Vec
//! after constructing the underlying NSView.

use cocoa_dom::Element;

use crate::html::directive::IntoDirective;

/// Pack a `(handler, param)` directive into the `FnOnce(&Element)`
/// boxed shape that builders' `directives: Vec<...>` field
/// stores.
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
        // `IntoDirective::run(&self, el, param)` — handler is
        // borrowed, param is owned. Cloning the Element passes a
        // shared retain to the directive body.
        handler.run(el.clone(), param);
    })
}

/// Drain a builder's directives Vec, calling each closure with
/// `&el`. Called from each builder's `Render::build` after
/// constructing the underlying element.
pub(crate) fn run_all(
    directives: Vec<Box<dyn FnOnce(&Element) + Send + 'static>>,
    el: &Element,
) {
    for d in directives {
        d(el);
    }
}
