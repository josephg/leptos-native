//! `Icon` — a single attribute carrying either an SF Symbol name
//! or a filesystem image path.
//!
//! Replaces the mutually-exclusive `sf_symbol=` / `image=` pair
//! that was duplicated across `<toolbar_item>` and `<menu_item>`.
//! One enum, one setter, one reactive slot.
//!
//! Lives in `cocoa_dom` for now. The variants cover the macOS /
//! iOS shape (SF Symbols + file paths); when the GTK port grows
//! a comparable need it can either depend on this enum
//! (treating SF-symbol names as freedesktop icon names) or
//! introduce a parallel one.

/// Source for an icon image.
///
/// Construct via the [`Self::sf_symbol`] / [`Self::image`] helpers
/// for ergonomic call sites; the bare variants are public so
/// reactive closures can return them directly. Mutually exclusive
/// by construction — one icon source per item.
///
/// ```ignore
/// use leptos_native::prelude::*;
///
/// // Static SF symbol.
/// <toolbar_item label="Add" icon=Icon::sf_symbol("plus.circle"/>
///
/// // Reactive — pick the symbol based on app state.
/// let pinned = RwSignal::new(false);
/// <toolbar_item
///     label="Pin"
///     icon=move || if pinned.get() {
///         Icon::sf_symbol("pin.fill")
///     } else {
///         Icon::sf_symbol("pin")
///     }
/// />
///
/// // File-based image.
/// <toolbar_item label="Logo" icon=Icon::image("/path/to/icon.png")/>
/// ```
///
/// Marked `#[non_exhaustive]` so future variants (in-memory
/// `Vec<u8>`, remote URL, ...) can be added without breaking
/// downstream `match` arms. Construct via the named helpers,
/// destructure with a wildcard fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Icon {
    /// SF Symbol name (`"plus.circle"`, `"sidebar.left"`, ...).
    /// Looked up in the system symbol library. Supported on
    /// macOS 11+ and iOS 13+; an unrecognised name falls back to
    /// a missing-symbol placeholder rather than panicking.
    SfSymbol(String),
    /// Filesystem path to an image file (PNG, JPEG, PDF, ...).
    /// Empty string clears the slot.
    Image(String),
}

impl Icon {
    /// Construct an SF Symbol icon by name.
    pub fn sf_symbol(name: impl Into<String>) -> Self {
        Self::SfSymbol(name.into())
    }

    /// Construct a file-based icon from a filesystem path.
    pub fn image(path: impl Into<String>) -> Self {
        Self::Image(path.into())
    }
}
