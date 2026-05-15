# Settings and Theming

## Persisting state with `gio::Settings`

GTK applications integrate with `gio::Settings` for persistent
preferences. The framework doesn't ship a custom abstraction —
use the `gio` crate directly and bridge to signals via `Effect`.

```rust
use leptos::prelude::*;
use gio::prelude::*;

fn main() {
    mount_to_window("org.example.settings", "Settings", (380, 340), || {
        let settings = gio::Settings::new("org.example.settings");

        // Load initial values.
        let volume = RwSignal::new(settings.double("volume"));
        let mute   = RwSignal::new(settings.boolean("mute"));

        // Persist on change.
        Effect::new({
            let settings = settings.clone();
            move |_| {
                let _ = settings.set_double("volume", volume.get());
            }
        });

        Effect::new({
            let settings = settings.clone();
            move |_| {
                let _ = settings.set_boolean("mute", mute.get());
            }
        });

        view! {
            <vstack padding=16.0 gap=12.0>
                <slider bind:value=volume min_value=0.0 max_value=100.0
                        enabled=move || !mute.get() />
                <checkbox bind:checked=mute>"Mute"</checkbox>
            </vstack>
        }
    });
}
```

You need a GSettings schema (`.gschema.xml`) installed under
`/usr/share/glib-2.0/schemas/` (or set via `XDG_DATA_DIRS` for
dev). See the `gio` documentation for the schema format and
`glib-compile-schemas`.

## Theming via GTK CSS

GTK4 widgets are themed by the system. To override styles for
your app, register CSS at app startup:

```rust
use gtk4::{CssProvider, gdk, prelude::*};

let provider = CssProvider::new();
provider.load_from_data(r#"
    button { background-image: linear-gradient(to bottom, #4a90e2, #357abd); color: white; }
    label.title { font-weight: bold; font-size: 18pt; }
"#);

gtk4::style_context_add_provider_for_display(
    &gdk::Display::default().unwrap(),
    &provider,
    gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
);
```

Apply CSS classes to elements via — currently you'd use `NodeRef`
to get to the underlying `gtk::Widget` and call
`.add_css_class("title")`. A higher-level `class=` attribute is
not yet wired up.

## What's not styleable via attributes

In the Cocoa port, `background_color`, `corner_radius`, etc.
work via NSView's CALayer. The GTK port doesn't currently apply
those attributes — they're accepted by the type system but are
no-ops or partially implemented. The expected styling path on
GTK is via CSS, not inline attributes.

## See also

- `gtk/examples/settings/src/main.rs` — a settings panel demo
  that uses `bind:value` on the slider, checkbox, and popup.
  Persistence to GSettings is left as an exercise.
