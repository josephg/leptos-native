//! Shared helpers for the macOS (Cocoa) and iOS (UIKit) Leptos ports.
//!
//! The two ports share the same target/action event model, the same
//! Taffy-bridged manual layout, and the same `bind:` / `use:` macro
//! plumbing. Most of that lives in platform-specific modules because
//! it's tightly bound to either `cocoa_dom::Element` or
//! `ios_dom::Element`. This crate is the small set of pieces that are
//! truly identical: marker types and generic traits with no Element
//! dependency.

pub mod attr_keys;
pub mod directive;
pub mod main_thread;

pub use main_thread::on_main;
