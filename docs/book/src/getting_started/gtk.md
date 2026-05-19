# Linux / GTK4

The GTK port targets GTK4 on Linux. Layout is driven by Taffy
(through a custom `gtk::LayoutManager`), the same engine used on the
other ports — `vstack` / `hstack` / `grid` behave consistently
across all three.

The widget set is smaller than Cocoa or iOS: `button`, `label`,
`text_field` / `secure_text_field`, `checkbox`, `slider`,
`pop_up_button`, plus the layout containers and menus. Color
wells, date pickers, scroll views, segmented controls, steppers,
text views, toolbars, and split views are macOS/iOS-only for now.

## Prerequisites

You need the GTK4 development headers and `pkg-config`:

```sh
# Debian / Ubuntu
sudo apt install libgtk-4-dev pkg-config

# Fedora
sudo dnf install gtk4-devel pkgconf-pkg-config

# Arch
sudo pacman -S gtk4 pkgconf
```

Plus Rust via [rustup](https://rustup.rs/).

## Your first app

```toml
# Cargo.toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2021"

[dependencies]
leptos = { package = "leptos_gtk", path = "../leptos-mac/gtk/leptos_gtk" }
```

```rust
use leptos_native::prelude::*;

#[component]
fn Counter(initial: i32) -> impl IntoView {
    let count = RwSignal::new(initial);
    view! {
        <vstack padding=16.0 gap=12.0>
            <label>{move || format!("Count: {}", count.get())}</label>
            <hstack gap=8.0>
                <button on:click=move |_| count.update(|n| *n -= 1)>"-1"</button>
                <button on:click=move |_| count.set(0)>"Reset"</button>
                <button on:click=move |_| count.update(|n| *n += 1)>"+1"</button>
            </hstack>
        </vstack>
    }
}

fn main() {
    mount_to_window(
        None,                   // application_id — see note below
        "Counter",
        (320, 200),
        || view! { <Counter initial=0 /> },
    );
}
```

Pass `None` for `application_id` and the framework picks
`local.cargo.<crate-name>` for you. For production apps with a
real domain, pass a stable reverse-DNS string instead — GTK
uses it for single-instance behaviour, `gio::Settings` paths,
and desktop integration:

```rust
mount_to_window(
    "org.example.counter",
    "Counter",
    (320, 200),
    || view! { <Counter initial=0 /> },
);
```

Then:

```sh
cargo run
```

Note two differences from the Cocoa entry point:

- The first argument is a **GApplication ID** in reverse-DNS form
  (`org.example.counter`). GTK uses this for single-instance
  behavior and desktop integration.
- The window size is `(i32, i32)` in pixels (not `f64` points
  like Cocoa).

## Running the bundled examples

```sh
cargo run -p counter_gtk
cargo run -p counters_gtk
cargo run -p greeter_gtk
cargo run -p checkbox_gtk
cargo run -p grid_gtk
cargo run -p login_form_gtk
cargo run -p menu_demo_gtk
cargo run -p settings_gtk
```

GTK examples are workspace members but excluded from
`default-members` — they need GTK linked at build time, so
`cargo build --workspace` would fail on a machine without the
development headers.

## Type-checking

```sh
cargo check -p gtk_dom                              # façade only
cargo check -p leptos_gtk                           # full port
cargo check -p leptos_gtk --no-default-features     # no gtk linking
```

`--no-default-features` is for contributors who want to typecheck
`leptos_gtk` from a machine without gtk4 headers installed. End
users don't need it.

## Entry points

- `mount_to_window(app_id, title, size, view_fn)` — convenience
  for a single window.
- `run(app_id, view_fn)` — more general; the closure receives a
  `&gtk4::Application` and can build multi-window or
  menu-bar+window layouts.

## What's different from Cocoa

- **Menus** are not part of the widget tree. They're a `gio::Menu`
  data model rendered by the desktop shell (GNOME's hamburger,
  Cinnamon's title-bar menu, etc.). The `<menu_bar>` / `<menu>` /
  `<menu_item>` API is the same as Cocoa's, but the menu node
  is a sibling of the window in your `run()` closure rather than
  a view-tree child.
- **No `keydown` / `keyup` events** on text fields. Use `change`
  for commit-on-Enter behavior.
- **No `<scroll_view>`, `<segmented_control>`, `<stepper>`,
  `<color_well>`, `<date_picker>`, `<progress_indicator>`,
  `<image_view>`, `<text_view>`, `<toolbar>`, or
  `<split_view>`** in this port yet.

## Where to go next

- [A Basic Component](../view/01_basic_component.md)
- [GTK Platform Features](../platform/gtk/README.md) — menus,
  GSettings integration, theming.
