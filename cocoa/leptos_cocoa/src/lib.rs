//! Cocoa/AppKit native UI port for leptos-mac.
//!
//! The [`Dom`] unit type is this crate's [`renderer::Renderer`] impl.
//! The element builders (`Button`, `TextField`, `View<Ch>`, etc.),
//! attribute machinery, bind machinery, and event/mount glue live in
//! sibling modules.
//!
//! Phase 8 status: foundation in. The rest of the crate (`element.rs`,
//! `attr.rs`, `bind.rs`, `directives.rs`, `node_ref.rs`,
//! `element_macos.rs`, `event_macos.rs`, `svg_macos.rs`, `mount.rs`)
//! still references the old non-generic `tachys::view::Render` shape
//! and is *not yet wired into this lib.rs*. Each will be ported in
//! later commits as `impl Render<Dom> for X`.

#![cfg(target_os = "macos")]

pub mod renderer_cocoa;

pub use renderer_cocoa::Dom;
