//! DOM-shaped façade over UIKit. The lowest layer of the iOS port —
//! `leptos_uikit` builds on top of this. User code talks to
//! `leptos_uikit::prelude::*` rather than to this crate directly.
//!
//! Provides [`Node`], [`Element`], [`Text`], and [`Placeholder`] types
//! that loosely mirror their `web_sys` equivalents in shape but are
//! backed directly by `UIView` (and subclasses like `UIButton`,
//! `UITextField`, `UISwitch`, `UISlider`). Plus the UIKit lifecycle
//! glue (`AppDelegate`, `SceneDelegate`, `RootViewController`,
//! `uiapplication_main`), a per-scene Taffy layout bridge with the
//! same `intrinsicContentSize`-driven measure callback as the cocoa
//! port, event-handler installation via UIControl target/action,
//! `dispatch2`-backed spawner, NSUserDefaults-backed `local_storage`,
//! NSTimer-backed `set_interval`, and the `Color` / `Date` /
//! `KeyEvent` value types.
//!
//! `leptos_uikit::Dom` is the [`renderer::Renderer`] impl that drives
//! this façade from a Render tree; this crate itself doesn't know
//! about Render or the upper abstractions.
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
pub mod objc_enums;
pub mod renderer;
pub mod spawner;
pub mod storage;

pub use color::{Color, SystemColor};
pub use date::Date;
pub use objc_enums::{DatePickerStyle, TextAlignment};
pub use interval::{
    set_interval, set_interval_with_handle, IntervalError, IntervalHandle,
};
pub use key_event::KeyEvent;
pub use node::{
    BoolAttr, Element, Node, NodeKind, Placeholder, StringAttr, Text,
    WeakElement, WeakNode, WeakPlaceholder, WeakText,
};
pub use storage::{local_storage, Storage, StorageError};
pub use renderer::{ClassList, CssStyleDeclaration, Event, Renderer, TemplateElement};

// Re-export the most common objc2 / objc2_ui_kit types so
// downstream crates don't have to take a direct objc2 dependency
// just to interact with our UIKit façade. Visual-style enums live
// in `objc_enums` as newtype wrappers — see `TextAlignment`,
// `DatePickerStyle` above.
pub use objc2::{rc::Retained, MainThreadMarker};
pub use objc2_ui_kit::UIView;
