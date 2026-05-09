//! `use:directive=param` plumbing — re-exports the shared
//! [`leptos_apple_shared::directive::IntoDirective`] trait. Each
//! iOS builder spells the trait `IntoDirective<Element, T, P>` to
//! bind the generic `E` to `ios_dom::Element`.

pub use leptos_apple_shared::directive::IntoDirective;
