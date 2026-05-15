//! GTK-local "decoration" attribute trait.
//!
//! Cocoa and iOS expose `background_color` / `corner_radius` /
//! `border_*` / `clip` directly on each element builder via the
//! shared `renderer::WithDecoration` trait — they translate to
//! CALayer / UIView properties at install time.
//!
//! GTK styling goes through `gtk::CssProvider` and per-widget CSS
//! classes instead; setting these attributes inline doesn't
//! match the platform idiom. The methods below exist so portable
//! view code (`<vstack background_color=red>`) compiles on GTK,
//! but each call emits a one-time warning per process so the
//! developer knows their styling won't show up.
//!
//! See `docs/book/src/platform/gtk/settings.md` for the GTK CSS
//! pattern.

use crate::gtk::IntoMaybeReactive;
use gtk_dom::Color;
use std::sync::Once;

static WARNED: Once = Once::new();

fn warn_decoration_ignored(attr: &str) {
    WARNED.call_once(|| {
        eprintln!(
            "[leptos_gtk] decoration attributes are not yet \
             implemented on the GTK port. The attribute '{}' (and \
             any others like background_color / corner_radius / \
             border_* / clip on the same widget) will compile but \
             have no visual effect. Use a `gtk::CssProvider` + \
             per-widget CSS class for styling on GTK — see \
             docs/book/src/platform/gtk/settings.md. This warning \
             prints once per process.",
            attr,
        );
    });
}

/// GTK-local shadow of `renderer::WithDecoration`. Provided so
/// `<vstack background_color=...>` etc. compile portably; emits
/// a one-time warning at install time and otherwise discards.
pub trait WithDecoration: Sized {
    fn background_color<V: IntoMaybeReactive<Color>>(self, _c: V) -> Self {
        warn_decoration_ignored("background_color");
        self
    }
    fn corner_radius<V: IntoMaybeReactive<f32>>(self, _r: V) -> Self {
        warn_decoration_ignored("corner_radius");
        self
    }
    fn border_width<V: IntoMaybeReactive<f32>>(self, _w: V) -> Self {
        warn_decoration_ignored("border_width");
        self
    }
    fn border_color<V: IntoMaybeReactive<Color>>(self, _c: V) -> Self {
        warn_decoration_ignored("border_color");
        self
    }
    fn clip<V: IntoMaybeReactive<bool>>(self, _c: V) -> Self {
        warn_decoration_ignored("clip");
        self
    }
}
