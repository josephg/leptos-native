//! DOM-shaped façade over Cocoa/AppKit. The lowest layer of the macOS
//! port — `leptos_cocoa` builds on top of this. User code talks to
//! `leptos_cocoa::prelude::*` rather than to this crate directly.
//!
//! Provides [`CocoaElem`] and [`Element`] types that mirror the shape of
//! their `web_sys` equivalents but are backed directly by `NSView`
//! (and subclasses like `NSButton`, `NSTextField`). The renderer's
//! "text node" and "placeholder" varieties are just Element
//! constructors ([`Element::create_text`], [`Element::create_placeholder`])
//! — there's no distinct wrapper type for them. Plus the AppKit lifecycle glue (`init_app`,
//! `run_loop`, `open_window`), the Taffy layout bridge (per-window
//! `TaffyTree` + `compute_layout` + `intrinsicContentSize` measure
//! callback), event-handler installation (`on_click`, `on_text_change`,
//! …), the `dispatch2`-backed spawner, `set_interval` / `local_storage`
//! analogues, and the `Color` / `Date` / `KeyEvent` value types.
//!
//! `leptos_cocoa::Dom` is the [`renderer::Renderer`] impl that drives
//! this façade from a Render tree; this crate itself doesn't know
//! about Render or any of the upper abstractions.
//!
//! # Threading
//!
//! All public APIs in this crate must be called on the main thread (the
//! AppKit thread). Construction will panic if called off-main; later
//! method calls panic via the `SendWrapper` runtime check. This mirrors
//! the single-threaded model that `web_sys` uses in the browser.

#[cfg(feature = "animation")]
pub mod animation;
pub mod app;
pub mod color;
#[cfg(feature = "debug-overlay")]
pub mod debug_overlay;
pub mod date;
pub mod event;
pub mod flipped_view;
pub mod icon;
pub mod interval;
pub mod key_event;
pub mod layout;
mod make_view;
pub mod menu;
pub mod node;
pub mod objc_enums;
pub mod renderer;
pub mod spawner;
pub mod split_window;
pub mod storage;
pub mod toolbar;
pub mod window;

pub use color::Color;
pub use date::Date;
pub use icon::Icon;
pub use objc_enums::{DatePickerStyle, LineBreak, SegmentStyle, TextAlignment};
pub use interval::{
    set_interval, set_interval_with_handle, IntervalError, IntervalHandle,
};
pub use key_event::KeyEvent;
pub use node::CocoaElem;
pub use storage::{local_storage, Storage, StorageError};
pub use renderer::{ClassList, CssStyleDeclaration, Event, TemplateElement};

// Re-export the most common objc2 / objc2_app_kit types so
// downstream crates don't have to take a direct objc2 dependency
// just to interact with our Cocoa façade. Visual-style enums live
// in `objc_enums` as newtype wrappers — see `TextAlignment`,
// `SegmentStyle`, `DatePickerStyle` above.
pub use objc2::{rc::Retained, MainThreadMarker};
pub use objc2_app_kit::NSView;
