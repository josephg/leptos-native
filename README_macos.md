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

The web Leptos crates still build and work as before — the macOS path
is gated by `cfg(target_os = "macos")` and swaps the renderer.

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
cocoa_dom/    — DOM-shaped façade over AppKit (NSView, NSButton,
                NSTextField, …). Owns the Taffy layout integration,
                NSWindow / NSApplication setup, target/action wiring,
                main-thread spawner.
tachys/src/cocoa/   — Bridges cocoa_dom to tachys' Render/Mountable
                     traits. Where you'll find the element builders
                     (button(), label(), slider(), …) and the bind:
                     plumbing.
leptos/src/mount_macos.rs — `mount_to_window` / `run` entry points.
examples_cocoa/<name>/  — Each example is its own Cargo crate (not a
                          workspace member, so each has its own
                          target/ dir).
xcuitests/    — Swift package: end-to-end UI tests that drive built
                .app bundles via the Accessibility framework.
implementation_log.md — Design-decision journal, newest first.
                        Read this before changing the layout cascade,
                        eventing, or window lifecycle.
tests.md      — Comprehensive test plan; tracked items have ■, open
                items have □.
CLAUDE.md     — Onboarding doc for AI assistants.
```

## Running the examples

The included examples in `examples_cocoa/`:

| Example              | Demonstrates                                      |
|----------------------|---------------------------------------------------|
| `counter`      | view! macro + #[component] + reactive label       |
| `counters`     | `<For>` dynamic children, per-row signals         |
| `greeter`      | `bind:value` two-way bind on a text field         |
| `checkbox`     | `bind:checked` + `on:input` + `on:change` coexist |
| `settings`     | slider, popup, mute-gates-slider reactive enable  |
| `login_form`   | secure_text_field, button.enabled=Memo, submit    |

To run any of them:

```sh
cargo run --manifest-path examples_cocoa/login_form/Cargo.toml
```

(Or `cd` into the example dir and `cargo run`.)

The window appears, you interact with it, and the app exits when you
close the window.

## Writing your own app

Add a new crate under `examples_cocoa/` (or anywhere else) with a
Cargo.toml like:

```toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "my_app"
path = "src/main.rs"

[dependencies]
leptos = { path = "../../leptos", features = ["native-ui"] }
```

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
`leptos/src/mount_macos.rs`):

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

There are two tiers, run independently.

### 1. cocoa_dom unit tests (~103 tests)

These run `cargo test` in the cocoa_dom crate. Tests build NSView trees,
exercise attribute setters, run Taffy `compute_layout`, fire NSControl
actions via the ObjC runtime, and assert against the resulting
state — all without opening a window.

```sh
cargo test --manifest-path cocoa_dom/Cargo.toml
```

These tests use a **custom main-thread harness** (see
`cocoa_dom/tests/common/mod.rs`) — AppKit APIs require the main
thread, but Cargo's default test harness spawns a worker per test.
Each test binary uses `harness = false` and runs the test bodies on
the binary's actual main thread.

To run a single test binary:

```sh
cargo test --manifest-path cocoa_dom/Cargo.toml --test layout
cargo test --manifest-path cocoa_dom/Cargo.toml --test attributes
# Available: element_creation, attributes, events, text_and_placeholder,
# tree_mutation, layout, app_menu, builders.
```

### 2. End-to-end UI tests (24 tests)

These live in `xcuitests/`, a Swift Package. They build a target
example as a `.app` bundle, launch it via `NSWorkspace`, and drive
its UI through the Accessibility framework (`AXUIElement`) — clicking
real buttons, typing real keystrokes via `CGEvent`, and asserting
against the live AX tree.

The test runner is `xctest`, which needs **Accessibility permission**
the first time you run it. The first run will print a remediation
message; follow these steps once:

1. Run `./xcuitests/run_tests.sh` once. It'll fail with a clear
   "permission not granted" error.
2. Open System Settings → Privacy & Security → Accessibility.
3. Click `+`, press ⌘⇧G, paste:
   ```
   /Applications/Xcode.app/Contents/Developer/usr/bin/xctest
   ```
4. Click Open → Add. Toggle the new entry on.

(Granting to your terminal/IDE doesn't cascade to xctest — it has its
own signed identity.)

After granting:

```sh
./xcuitests/run_tests.sh
```

This builds all three example apps as `.app` bundles, sets the
`LEPTOS_MAC_<NAME>_PATH` env vars, and runs `swift test`.

To run only one suite:

```sh
./xcuitests/run_tests.sh --filter LoginFormUITests.LoginFormUITests
./xcuitests/run_tests.sh --filter SettingsUITests.SettingsUITests
./xcuitests/run_tests.sh --filter CountersUITests.CountersUITests
```

To list available tests:

```sh
cd xcuitests && swift test list
```

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
  structure (cocoa_dom / tachys::cocoa / *_macos facades) and the
  conventions / gotchas.
- **[tests.md](tests.md)** — comprehensive test checklist. ■ items
  have coverage; □ items don't.
- **[xcuitests/Sources/AppDriver/](xcuitests/Sources/AppDriver)** —
  the Swift helpers wrapping `AXUIElement`. The starting point for
  writing new UI tests against new examples.

## Known limitations

- **Single platform**: macOS only. iOS / iPadOS would be a separate
  port (UIKit, not AppKit).
- **No SSR / hydration**: native apps don't have a server side. The
  hydration stubs exist purely so `IntoView`'s trait bounds compile.
- **`mount_to_window` leaks the Owner**: the reactive root is
  intentionally leaked for the app's lifetime. When the run loop
  exits, the OS reclaims everything anyway. Per-window cleanup
  (`windowWillClose:` → `Mountable::unmount`) does fire correctly for
  multi-window apps.
- **No XCUIAutomation literal**: the `.xcuitests/` tier uses
  `AXUIElement` directly rather than `XCUIApplication`, because Swift
  Package Manager doesn't support UI testing bundles. Same end-to-end
  fidelity, no Xcode project required.
