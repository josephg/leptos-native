//! Cocoa/AppKit native UI port for leptos-mac.
//!
//! The [`Dom`] unit type is this crate's [`renderer::Renderer`] impl.

#![cfg(target_os = "macos")]
#![allow(missing_docs)]

pub mod cocoa;
pub mod directive;
pub mod element_macos;
pub mod event_macos;
pub mod keys;
pub mod mount;
pub mod renderer_cocoa;
pub mod svg_macos;

pub use renderer_cocoa::Dom;
