# Leptos — GTK4 native port (Linux)

This fork ports the [Leptos](https://leptos.dev) reactive
framework to native Linux UI on **GTK4**. The same `view!{}`
macro, `#[component]` attribute, and fine-grained reactive
signals drive `gtk::Widget`s instead of a DOM: `<button>` becomes
a `gtk::Button`, `<text_field>` becomes a `gtk::Entry`,
`bind:value` two-way binds a signal to a control's state.
Layout is via [Taffy](https://github.com/DioxusLabs/taffy)
flexbox, plugged into GTK's `LayoutManager` protocol.

```rust
use leptos::prelude::*;

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
    mount_to_window("com.example.counter", "Counter", (320.0, 200.0), || {
        view! { <Counter initial=0 /> }
    });
}
```

This is a **native-only fork** of Leptos. The web / SSR crates
have been removed; the same `view!{}` macro and reactive
primitives drive GTK directly. There's a parallel macOS / AppKit
port and an iOS / UIKit port on the same branch — see
[`CLAUDE.md`](./CLAUDE.md) for the cross-port story. This README
focuses on the GTK side.

## Prerequisites

GTK4 development headers + pkg-config:

```sh
# Debian / Ubuntu
sudo apt install libgtk-4-dev pkg-config

# Fedora
sudo dnf install gtk4-devel pkg-config

# Arch
sudo pacman -S gtk4 pkgconf

# macOS (for cross-checking — gtk_dom builds, but examples won't
# run without an X / Wayland display)
brew install gtk4
```

System docs (`gtk4-doc`, `glib2-doc`) are very useful — see the
`### System documentation` section in [`CLAUDE.md`](./CLAUDE.md)
for the canonical paths to GTK / GLib / GIO reference pages.

## Crate layout

```
gtk/dom/                         — DOM-shaped façade over GTK4.
                                   `Element::create(tag)` maps tags
                                   to `gtk::Widget` subclasses
                                   (`<button>` → `gtk::Button`,
                                   `<vstack>`/`<hstack>` /
                                   `<grid>` → `gtk::Box`). Owns the
                                   Taffy layout integration (via a
                                   custom `LayoutManager`), the
                                   `gtk::Application` setup, and the
                                   `glib::MainContext`-backed
                                   spawner.
gtk/leptos_gtk/src/gtk/          — Bridges gtk_dom to renderer's
                                   Render/Mountable traits. Element
                                   builders (`button()`, `vstack()`,
                                   `slider()`, …) and the bind:
                                   plumbing.
gtk/leptos_gtk/src/mount.rs      — `mount_to_window` / `run` entry
                                   points.
gtk/examples/<name>/             — Workspace members. GTK examples
                                   are kept out of `default-members`
                                   in the root `Cargo.toml` because
                                   they link gtk4 at build time;
                                   they're discoverable via
                                   `cargo build --workspace` but
                                   need explicit `cargo run -p`.
gtk_implementation_log.md        — Design-decision journal, newest
                                   first.
tests_gtk.md                     — Per-port test plan.
```

## Running the examples

```sh
cargo run -p counter_gtk
cargo run -p counters_gtk
cargo run -p greeter_gtk
cargo run -p grid_gtk
cargo run -p login_form_gtk
cargo run -p settings_gtk
cargo run -p checkbox_gtk
cargo run -p menu_demo_gtk
```

(See `gtk/examples/` for the current set.)

The window appears, you interact with it, and the app exits when
you close the window.

## Writing your own app

Add a new crate (under `gtk/examples/` for the workspace, or
anywhere else) with a Cargo.toml like:

```toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2021"

[dependencies]
leptos = { package = "leptos_gtk", path = "../../leptos_gtk" }
```

`leptos_gtk` has `gtk` as a default feature — adding the
dependency gives you a working build with no extra flags. Each
port is selected by aliasing `leptos = { package = "leptos_<port>" }`.
There's no `native-ui` umbrella feature — every binary picks one
port directly.

For the contributor-only typecheck path (verify the renderer-
agnostic core compiles without gtk4 installed), use
`--no-default-features`.

## How it maps to GTK

The GTK port mirrors the macOS port's three-layer architecture
one-for-one. Differences (closures owned by signal connections vs.
cocoa's thread-local handler store; `gtk::Widget` is `!Send` so
nodes wrap in `SendWrapper`; `<view>` defaults to vertical
orientation by convention) are documented inline in
`gtk_implementation_log.md` and in the relevant module docs.

Tag → widget mapping (sample):

| Tag                     | GTK widget                |
|-------------------------|---------------------------|
| `<stack>` / `<vstack>` / `<hstack>` / `<grid>` | `gtk::Box`               |
| `<button>`              | `gtk::Button`             |
| `<checkbox>` / `<toggle>` | `gtk::CheckButton`      |
| `<label>`               | `gtk::Label`              |
| `<text_field>`          | `gtk::Entry`              |
| `<secure_text_field>`   | `gtk::PasswordEntry`      |
| `<slider>`              | `gtk::Scale`              |
| `<pop_up_button>`       | `gtk::DropDown`           |
| `<stack_view>`          | `gtk::Box`                |

Taffy owns the layout inside each container; GTK owns the
container's outer frame via its `LayoutManager` protocol.

## Running the tests

```sh
cargo test -p gtk_dom
cargo test -p leptos_gtk
```

See `tests_gtk.md` for the comprehensive test plan.

## Where to look next

- **[gtk_implementation_log.md](gtk_implementation_log.md)** —
  design-decision journal. Newest entries at the top.
- **[CLAUDE.md](CLAUDE.md)** — cross-port architecture overview.
- **[tests_gtk.md](tests_gtk.md)** — test plan + tracker.
