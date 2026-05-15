# iOS / UIKit

The iOS port targets UIKit. It mirrors the macOS layering closely
(same Taffy bridge, same target/action handler-store pattern). The
visible differences are:

- **No user-facing windows.** An iOS app is a single fullscreen
  scene, so there's no `window()` builder and no
  `mount_to_window`. Only `run(view_fn)` exists.
- **Safe-area and keyboard avoidance** are handled automatically
  by a `RootViewController` that pads the content root with the
  current safe-area insets and the keyboard layout guide. See
  [Safe Area and Keyboard](../platform/ios/safe_area.md).
- **No menu bar.** No `<menu_bar>` / `<menu>` builders.
- **A few control deltas.** `<switch>` replaces `<checkbox>`,
  there's no `<pop_up_button>` or `<color_well>`. See
  [Differences from macOS](../platform/ios/deltas.md).

## Prerequisites

- **macOS host with Xcode**. UIKit only builds on macOS; you also
  need the iOS Simulator that ships with Xcode.
- **Rust targets:**
  ```sh
  rustup target add aarch64-apple-ios-sim    # Apple Silicon sim
  rustup target add x86_64-apple-ios         # Intel sim (optional)
  rustup target add aarch64-apple-ios        # real devices (optional)
  ```

No Xcode project is required. iOS examples ship a `run_ios.sh`
shell script that hand-rolls a `.app` bundle and installs it via
`xcrun simctl`.

## Your first app

iOS example crates are **out-of-workspace** — Cargo doesn't
support target-conditional workspace members, and iOS builds need
`--target aarch64-apple-ios-sim`. Create your app outside the main
workspace:

```toml
# Cargo.toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2021"

[dependencies]
leptos = { package = "leptos_uikit", path = "../leptos-mac/uikit/leptos_uikit" }
```

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
    leptos::mount_ios::run(|| view! { <Counter initial=0 /> });
}
```

Copy `uikit/examples/counter/run_ios.sh` next to your `Cargo.toml`
and run it:

```sh
./run_ios.sh
```

The script will:

1. `cargo build --target aarch64-apple-ios-sim`
2. Assemble a `.app` bundle with an Info.plist + the built
   binary.
3. Boot (or reuse) an iPhone simulator.
4. `xcrun simctl install` the app, terminate any prior copy, then
   launch it.
5. Stream stdout via `--console`.

## The `-t` flag for non-interactive runs

Without `-t`, the script blocks streaming app output. From CI or
an agent loop:

```sh
./run_ios.sh -t 3
```

`-t SECONDS` terminates the app after that many seconds and
returns. Three seconds is plenty to verify the app launched and
didn't crash.

## Running the bundled examples

```sh
cd uikit/examples/counter   && ./run_ios.sh -t 3
cd uikit/examples/counters  && ./run_ios.sh -t 3
cd uikit/examples/greeter   && ./run_ios.sh -t 3
cd uikit/examples/grid      && ./run_ios.sh -t 3
cd uikit/examples/controls  && ./run_ios.sh -t 3
cd uikit/examples/switch_demo && ./run_ios.sh -t 3
cd uikit/examples/todomvc   && ./run_ios.sh -t 3
```

## Type-checking without the full build

```sh
cargo check -p ios_dom        --target aarch64-apple-ios-sim
cargo check -p leptos_uikit   --target aarch64-apple-ios-sim
```

## Building outside `run_ios.sh`

Set `CARGO_TARGET_DIR` so the build lands in the shared workspace
`target/` rather than a per-example dir:

```sh
CARGO_TARGET_DIR=$(pwd)/target cargo build \
  --manifest-path uikit/examples/counter/Cargo.toml \
  --target aarch64-apple-ios-sim
```

## Where to go next

- [A Basic Component](../view/01_basic_component.md)
- [iOS Platform Features](../platform/ios/README.md) — app
  lifecycle, safe area, build flow, macOS deltas.
