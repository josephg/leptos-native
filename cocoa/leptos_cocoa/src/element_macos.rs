//! Macro-facing facade for `tachys::html::element` on macOS.
//!
//! Re-exposes element constructors at the path the `view!{}` macro
//! expects (`::leptos::tachys::html::element::<tag>()`), backed by
//! the Cocoa builders in `tachys::cocoa::element`.
//!
//! This is "Option 2" from the Stage-5-part-3 design: the macro
//! emission stays unchanged; the path it emits resolves on macOS to
//! a Cocoa-flavoured implementation that quacks the way the macro
//! expects (chained `.child(...)` / `.on(event, handler)`).

#![allow(missing_docs)]

// Direct re-exports for tags that map 1:1 to a Cocoa builder.
pub use crate::cocoa::element::{
    button, checkbox, color_well, date_picker, grid, hstack, image_view, label,
    pop_up_button, progress_indicator, scroll_view, secure_text_field,
    segmented_control, slider, stack, stepper, text_field, text_view, toggle,
    vstack,
};

// Native NSToolbar-backed `<toolbar>` + items. Lives in its own
// module since NSToolbar is not a view — attaches to the
// containing NSWindow at mount time.
pub use crate::cocoa::toolbar::{
    toolbar, toolbar_flexible_space, toolbar_item, toolbar_print,
    toolbar_search_item, toolbar_sidebar_tracking_separator, toolbar_space,
    toolbar_toggle_sidebar,
};

// Split-view: `<split_view>` + `<split_pane>` — native
// `NSSplitViewController` panes, each with their own Taffy tree.
// Must be used with `mount_to_split_window` (not `mount_to_window`).
pub use crate::cocoa::split::{split_pane, split_view};

// Menus: `<menu_bar>` + `<menu>` + `<menu_item>` + `<menu_separator>`.
// `<menu_bar>` sits as a top-level sibling of `<window>` in the
// `run()` closure; the others only make sense nested inside.
pub use crate::cocoa::menu::{menu, menu_bar, menu_item, menu_separator};

// `<window>` — top-level NSWindow builder. Until menu support landed,
// examples used `window()` directly (imported from
// `leptos::tachys::cocoa::window`); the `<window>` macro path was
// unused. Re-exposing it here lets a single `view!{}` invocation host
// both `<menu_bar>` and `<window>` siblings.
pub use crate::cocoa::window::window;
