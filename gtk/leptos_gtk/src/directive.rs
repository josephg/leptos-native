//! `use:directive=param` plumbing — re-exports the shared
//! [`renderer::directive::IntoDirective`] trait. Each gtk builder
//! spells the trait `IntoDirective<Element, T, P>` to bind the
//! generic `E` to `gtk_dom::Element`.

pub use renderer::directive::IntoDirective;
