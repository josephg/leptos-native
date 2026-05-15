# Leptos for macOS — getting started

This fork ports the [Leptos](https://leptos.dev) reactive framework to
native macOS. The same `view!` macro, `#[component]` attribute, and
signals you use on the web drive an AppKit UI instead of a DOM:
`<button>` becomes an `NSButton`, `<text_field>` becomes an
`NSTextField`, `bind:value` two-way binds a signal to a control's
state. Layout is via [Taffy](https://github.com/DioxusLabs/taffy)
flexbox.

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
    mount_to_window("Counter", (320.0, 200.0), || {
        view! { <Counter initial=0 /> }
    });
}
```

This is a **native-only fork** of Leptos. The web / SSR crates
have been removed; the same `view!{}` macro and reactive
primitives drive AppKit directly. See [`CLAUDE.md`](./CLAUDE.md)
for the full picture.

## Prerequisites

- **macOS 11.0+** — earlier versions are untested.
- **Xcode** — installed from the App Store. Provides the AppKit
  toolchain, signed `xctest` binary used for UI tests, and the macOS
  SDK. Verify with `xcodebuild -version`.
- **Rust** — install via [rustup](https://rustup.rs/). The toolchain
  version comes from `rust-toolchain.toml`; rustup will fetch it
  automatically on first build.

That's it — no extra build tools, no `cargo-bundle`, no Xcode
project required. The macOS examples build via plain `cargo run`.

## Crate layout

```
cocoa/dom/                       — DOM-shaped façade over AppKit
                                   (NSView, NSButton, NSTextField,
                                   …). Owns the Taffy layout
                                   integration, NSWindow /
                                   NSApplication setup,
                                   target/action wiring, main-thread
                                   spawner.
cocoa/leptos_cocoa/src/cocoa/    — Bridges cocoa_dom to renderer's
                                   Render/Mountable traits. Where
                                   you'll find the element builders
                                   (button(), label(), slider(), …)
                                   and the bind: plumbing.
cocoa/leptos_cocoa/src/mount.rs  — `mount_to_window` / `run` /
                                   `mount_to_split_window` entry
                                   points.
cocoa/examples/<name>/           — Workspace members; each example
                                   is a small `cargo run`-able crate.
implementation_log.md            — Design-decision journal, newest
                                   first. Read this before changing
                                   the layout cascade, eventing, or
                                   window lifecycle.
tests_macos.md                   — Comprehensive test plan; tracked
                                   items have ■, open items have □.
CLAUDE.md                        — Onboarding doc for AI assistants
                                   (and humans).
```

## Running the examples

The cocoa examples live under `cocoa/examples/` — each is a
workspace member, so plain `cargo run -p <name>` works from
the repo root. A sample:

| Example              | Demonstrates                                      |
|----------------------|---------------------------------------------------|
| `counter_cocoa`      | `view!` macro + `#[component]` + reactive label   |
| `counters_cocoa`     | `<For>` dynamic children, per-row signals         |
| `greeter_cocoa`      | `bind:value` two-way bind on a text field         |
| `checkbox_cocoa`     | `bind:checked` + `on:input` + `on:change` coexist |
| `settings_cocoa`     | slider, popup, mute-gates-slider reactive enable  |
| `login_form_cocoa`   | secure_text_field, button.enabled=Memo, submit    |
| `menu_demo_cocoa`    | `<menu_bar>` / `<menu>` / `<menu_item>` + shortcuts |
| `toolbar_demo_cocoa` | `<toolbar>` + every toolbar-item variant          |
| `spotify_cocoa`      | Full Spotify-style desktop UI stress test          |
| `pages_cocoa`        | Apple-Pages-style toolbar + split-view inspector   |

```sh
cargo run -p login_form_cocoa
cargo run -p toolbar_demo_cocoa
# …etc.
```

The window appears, you interact with it, and the app exits when you
close the window.

## Writing your own app

Add a new crate (under `cocoa/examples/` for the workspace, or
anywhere else) with a Cargo.toml like:

```toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2021"

[dependencies]
leptos = { package = "leptos_cocoa", path = "../../leptos_cocoa" }
```

There is no `native-ui` feature flag — each example picks its
port by aliasing `leptos = { package = "leptos_<port>" }`.

Then in `src/main.rs`:

```rust
use leptos::prelude::*;

#[component]
fn App() -> impl IntoView {
    view! { <label>"Hello, AppKit"</label> }
}

fn main() {
    mount_to_window("My App", (320.0, 200.0), || {
        view! { <App /> }
    });
}
```

Multi-window apps use `run` directly (see
`cocoa/leptos_cocoa/src/mount.rs`):

```rust
fn main() {
    leptos::mount::run(|| (
        window().title("Main").size(640.0, 480.0).child(view! { <Main/> }),
        window().title("Inspector").size(280.0, 600.0).child(view! { <Inspector/> }),
    ));
}
```

## Available controls and attributes

Element tags are snake_case (deliberate convention — see
[CLAUDE.md](CLAUDE.md)):

| Tag                     | AppKit class            | Notes                          |
|-------------------------|-------------------------|--------------------------------|
| `<view>`                | `FlippedView`           | generic flexbox container      |
| `<vstack>` / `<hstack>` | `FlippedView`           | preset flex_direction          |
| `<button>`              | `NSButton` (push)       |                                |
| `<checkbox>`            | `NSButton` (switch)     |                                |
| `<label>`               | `NSTextField` (label)   | non-editable                   |
| `<text_field>`          | `NSTextField`           | editable                       |
| `<secure_text_field>`   | `NSSecureTextField`     | password input                 |
| `<slider>`              | `NSSlider`              | continuous (drag fires)        |
| `<pop_up_button>`       | `NSPopUpButton`         |                                |

Attributes:

- **Layout**: `padding=N`, `gap=N`, `flex_grow=N`, `flex_direction=…`
- **Static**: `title=…`, `value=…`, `placeholder=…`, `min_value=N`,
  `max_value=N`, `items=vec![…]`, `selection=N`
- **Bool**: `enabled=true|closure`, `checked=true|closure`
- **Events**: `on:click`, `on:input` (text), `on:change` (text)
- **Two-way**: `bind:value=signal` (text + slider), `bind:checked=signal`
  (checkbox), `bind:selection=signal` (popup)

The `on:event` list is constrained at compile time per builder type via
the `SupportsEvent<E>` trait — `<button on:input=…>` won't compile.

## Running the tests

```sh
# Dom-layer unit tests — build NSView trees, exercise attribute
# setters, run Taffy `compute_layout`, fire NSControl actions
# via the ObjC runtime, assert against the resulting state. All
# without opening a window.
cargo test -p cocoa_dom

# High-level leptos_cocoa integration tests — exercise the
# builder / Mountable / event plumbing against real AppKit.
cargo test -p leptos_cocoa
```

Both use a **custom main-thread harness** (`tests/common/mod.rs`)
— AppKit APIs require the main thread, but Cargo's default test
harness spawns a worker per test. Each test binary uses
`harness = false` and runs the test bodies on the binary's actual
main thread.

End-to-end UI tests (XCUITest harness driving real `.app`
bundles via the Accessibility framework) are tracked but **not
yet implemented**. See `tests_macos.md` for the planned shape.

**Don't run these in a session you're using interactively** — they
synthesise real keyboard input via `CGEvent`, which gets sent to
whichever window has focus. The tests bring the .app forward before
typing, but if you click away mid-run keystrokes will land somewhere
else.

## Where to look next

- **[implementation_log.md](implementation_log.md)** — the running
  design-decision journal. If you're about to change layout, eventing,
  window lifecycle, or the macro plumbing, read this first. Newest
  entries at the top.
- **[CLAUDE.md](CLAUDE.md)** — architecture overview written for AI
  agents but useful for human onboarding too. Covers the three-layer
  structure (`cocoa/dom` / `cocoa/leptos_cocoa/src/cocoa` / element-macro
  facades) and the conventions / gotchas.
- **[tests_macos.md](tests_macos.md)** — comprehensive test
  checklist. ■ items have coverage; □ items don't.

## Known limitations

- **`mount_to_window` leaks the Owner**: the reactive root is
  intentionally leaked for the app's lifetime. When the run loop
  exits, the OS reclaims everything anyway. Per-window cleanup
  (`windowWillClose:` → `Mountable::unmount`) does fire correctly for
  multi-window apps.
- **XCUIAutomation harness is deferred**: an end-to-end test tier
  driving built `.app` bundles via the Accessibility framework is
  planned but not yet implemented. See `tests_macos.md`.
