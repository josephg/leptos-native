//! DOM-shaped façade over Cocoa/AppKit, used by tachys/leptos to render
//! native macOS UIs.
//!
//! This crate is the lowest layer of the macOS port. It provides
//! [`Node`], [`Element`], [`Text`], and [`Placeholder`] types that loosely
//! mirror their web-sys equivalents in shape, but are backed directly by
//! `NSView` (and subclasses such as `NSButton`, `NSTextField`).
//!
//! Higher layers (`tachys`, `leptos`) target this façade rather than
//! `web_sys` when building for macOS.
//!
//! # Threading
//!
//! All public APIs in this crate must be called on the main thread (the
//! AppKit thread). Construction will panic if called off-main; later
//! method calls panic via the `SendWrapper` runtime check. This mirrors
//! the single-threaded model that `web_sys` uses in the browser.

#![cfg(target_os = "macos")]

pub mod app;
pub mod event;
pub mod flipped_view;
pub mod layout;
pub mod node;
pub mod renderer;
pub mod spawner;
pub mod window;

pub use node::{
    BoolAttr, Element, Node, NodeKind, Placeholder, StringAttr, Text,
};
pub use renderer::{ClassList, CssStyleDeclaration, Event, Renderer, TemplateElement};

// Re-export the most common objc2 / objc2_app_kit types so
// downstream crates don't have to take a direct objc2 dependency
// just to interact with our Cocoa façade.
pub use objc2::{rc::Retained, MainThreadMarker};
pub use objc2_app_kit::NSView;
