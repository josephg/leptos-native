# macOS / Cocoa

The macOS port is the most mature of the three. It targets AppKit
directly — `<button>` becomes `NSButton`, `<text_field>` becomes
`NSTextField`, `<window>` builds an `NSWindow`. Layout is driven by
Taffy.

## Prerequisites

- **macOS 11.0+**. Earlier versions are untested.
- **Xcode** (from the App Store). This provides the AppKit toolchain
  and the macOS SDK. Verify with `xcodebuild -version`.
- **Rust**, installed via [rustup](https://rustup.rs/). The
  toolchain version is pinned in `rust-toolchain.toml`; rustup
  fetches it automatically on first build.

That's it — no `cargo-bundle`, no Xcode project required. Examples
build with plain `cargo run`.

## Your first app

Create a new binary crate:

```sh
cargo new my_app
cd my_app
```

Edit `Cargo.toml`:

```toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2021"

[dependencies]
leptos = { package = "leptos_cocoa", path = "../leptos-mac/cocoa/leptos_cocoa" }
```

> Until `leptos_cocoa` is published on crates.io, depend on the
> fork by path or git URL.

Replace `src/main.rs`:

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
    mount_to_window("Counter", (320.0, 200.0), || {
        view! { <Counter initial=0 /> }
    })
    .run();
}
```

Then:

```sh
cargo run
```

A 320×200 window with the counter UI appears. The app quits when
you close the window (the bundled `AppDelegate` returns `true`
from `applicationShouldTerminateAfterLastWindowClosed:`).

## Running the bundled examples

The fork itself ships a wide range of examples under
`cocoa/examples/`. They're all workspace members, so:

```sh
cargo run -p counter_cocoa
cargo run -p counters_cocoa     # <For> keyed iteration
cargo run -p greeter_cocoa      # bind:value text field
cargo run -p login_form_cocoa   # Memo-driven button enable
cargo run -p error_boundary_cocoa
cargo run -p grid_cocoa         # Grid layout
cargo run -p menu_demo_cocoa    # native menu bar
cargo run -p toolbar_demo_cocoa # NSToolbar
cargo run -p pages_cocoa        # split view + toolbar
cargo run -p two_windows_cocoa  # multi-window
cargo run -p showcase_cocoa     # most controls in one app
```

## Type-checking without running

Compiling AppKit binaries can take a moment. To iterate just on
the lowest layer:

```sh
cargo check -p cocoa_dom        # the NSView façade
cargo check -p leptos_cocoa     # element builders + macro shims
```

## Entry points

Two top-level entry points are exposed by `leptos_cocoa`:

- `mount_to_window(title, size, view_fn)` — opens one window with
  your view mounted inside. The convenient default for simple
  apps.
- `run(view_fn)` — more general; the closure returns any
  `Render<Dom>`. Use this when your top-level view contains
  multiple `<window>`s or a `<menu_bar>` sibling, or when you
  need to mount into an `<split_view>` via the dedicated
  `mount_to_split_window` entry point. See
  [Windows](../platform/cocoa/windows.md).

Both return an `AppHandle`; calling `.run()` on it enters the
AppKit run loop and tears the app state down cleanly when the
loop returns. See [Cocoa Overview](../platform/cocoa/README.md#apphandle-and-the-run-chain).

## Where to go next

- [A Basic Component](../view/01_basic_component.md) — read this
  first if Leptos is new to you.
- [Layout / Flexbox](../layout/flexbox.md) — how `vstack` /
  `hstack` / `view` actually lay things out.
- [macOS Platform Features](../platform/cocoa/README.md) — menus,
  toolbar, split view, SF Symbols.
