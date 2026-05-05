//! DOM-shaped façade over UIKit, used by tachys/leptos to render
//! native iOS UIs.
//!
//! This crate is the lowest layer of the iOS port. It provides
//! [`Node`], [`Element`], [`Text`], and [`Placeholder`] types that loosely
//! mirror their web-sys equivalents in shape, but are backed directly by
//! `UIView` (and subclasses such as `UIButton`, `UITextField`).
//!
//! Higher layers (`tachys`, `leptos`) target this façade rather than
//! `web_sys` when building for iOS.
//!
//! # Threading
//!
//! All public APIs in this crate must be called on the main thread (the
//! UIKit thread). Construction will panic if called off-main; later
//! method calls panic via the `SendWrapper` runtime check. This mirrors
//! the single-threaded model that `web_sys` uses in the browser.

#![cfg(target_os = "ios")]

pub mod app;
pub mod color;
pub mod date;
pub mod event;
pub mod interval;
pub mod key_event;
pub mod layout;
pub mod node;
pub mod renderer;
pub mod spawner;
pub mod storage;

pub use color::Color;
pub use date::Date;
pub use interval::{
    set_interval, set_interval_with_handle, IntervalError, IntervalHandle,
};
pub use key_event::KeyEvent;
pub use node::{
    BoolAttr, Element, Node, NodeKind, Placeholder, StringAttr, Text,
};
pub use storage::{local_storage, Storage, StorageError};
pub use renderer::{ClassList, CssStyleDeclaration, Event, Renderer, TemplateElement};

// Re-export the most common objc2 / objc2_ui_kit types so
// downstream crates don't have to take a direct objc2 dependency
// just to interact with our UIKit façade.
pub use objc2::{rc::Retained, MainThreadMarker};
pub use objc2_ui_kit::{
    UIView, NSTextAlignment,
};
