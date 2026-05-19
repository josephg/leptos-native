//! Shared helpers for the macOS (Cocoa) and iOS (UIKit) Leptos ports.
//!
//! Just the apple-specific main-thread dispatch helper, hence the
//! `dispatch2` dep. Port-agnostic plumbing (the `IntoDirective`
//! trait, `AttributeKey` / `Value` / `Checked` markers) lives in
//! `common/renderer` (`renderer::directive`, `renderer::attr_keys`).

pub mod main_thread;

pub use main_thread::on_main;
