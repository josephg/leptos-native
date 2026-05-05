# Leptos for iOS — getting started

This fork ports the [Leptos](https://leptos.dev) reactive framework
to native iOS. The same `view!` macro, `#[component]` attribute, and
signals you use on the web drive a UIKit UI instead of a DOM:
`<button>` becomes a `UIButton`, `<text_field>` becomes a
`UITextField`, `bind:value` two-way binds a signal to a control's
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
    leptos::mount_ios::run(|| view! { <Counter initial=0 /> });
}
```

The web Leptos crates still build and work as before — the iOS path
is gated by `cfg(target_os = "ios")` and swaps the renderer.

## Prerequisites

- **Xcode** — installed from the App Store. Provides the iOS SDK,
  the simulators, and the `xcrun simctl` CLI used by the example
  run scripts. Verify with `xcodebuild -version`.
- **iOS simulator** — pre-bundled with Xcode. The example scripts
  default to "any booted iPhone simulator"; if none is booted they
  create or boot one automatically.
- **Rust + iOS targets** — install via [rustup](https://rustup.rs/):
  ```sh
  rustup target add aarch64-apple-ios-sim   # for simulator on Apple Silicon
  rustup target add x86_64-apple-ios        # for simulator on Intel Macs
  rustup target add aarch64-apple-ios       # for real device builds
  ```

That's it — no Xcode project required for the included examples.
The simulator launch scripts build with `cargo`, hand-roll a
`.app` bundle, and `xcrun simctl install` it.

## Crate layout

```
ios_dom/      — DOM-shaped façade over UIKit (UIView, UIButton,
                UITextField, UISwitch, …). Owns the Taffy layout
                integration, UIWindow / UIApplication setup, target/
                action wiring, main-thread spawner, keyboard-avoidance
                via UIKeyboardLayoutGuide.
tachys/src/ios/   — Bridges ios_dom to tachys' Render/Mountable
                    traits. Element builders (button(), text_field(),
                    switch_(), slider(), …) and the bind: plumbing.
leptos/src/mount_ios.rs — `run` entry point.
examples_ios/<name>/    — Each example is its own Cargo crate with
                          a `run_ios.sh` script that builds, bundles,
                          installs, and launches in the simulator.
implementation_ios.md   — Design-decision journal, newest first.
audit_ios.md            — Original audit + ongoing status.
TODO_ios.md             — Priority-ordered outstanding work.
```

## Running the examples

The included examples in `examples_ios/`:

| Example       | Demonstrates                                                  |
|---------------|---------------------------------------------------------------|
| `counter`     | view! macro + #[component] + reactive label                   |
| `greeter`     | `bind:value` on a text field; reactive label echoes input     |
| `switch_demo` | `<switch bind:checked>` + `<slider bind:value>` together      |
| `controls`    | Full showcase: every supported control inside a `<scroll_view>` |
| `counters`    | `<For>` keyed iteration; dynamic add/remove rows              |
| `checkbox`    | `bind:value` + `on:input` + `on:change` coexist on one field; switch |
| `login_form`  | `<text_field>` + `<secure_text_field>` + `Memo`-gated submit  |
| `settings`    | slider/switch/segmented_control with derived `enabled=`       |
| `timer`       | `set_interval_with_handle` + a stepper-driven dynamic interval |
| `todomvc`     | Full TodoMVC: `<For>`, persistence via `local_storage`        |

To run one:

```sh
cd examples_ios/counter
./run_ios.sh
```

The script:
1. Builds the example for `aarch64-apple-ios-sim`.
2. Wraps the binary into a minimal `.app` bundle with an Info.plist.
3. Boots an iPhone simulator (creating one if needed).
4. Terminates any prior instance of the app.
5. Installs and launches the app, opening the Simulator window.

Adjust the simulator device by editing the script's `iPhone 16` literal,
or boot a specific simulator before running and the script will pick
it up.

## Writing your own app

Add a new crate under `examples_ios/` (or anywhere else) with a
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

[profile.dev]
panic = "abort"   # iOS doesn't support unwinding out of objc frames
```

Then in `src/main.rs`:

```rust
use leptos::prelude::*;

#[component]
fn App() -> impl IntoView {
    view! { <label>"Hello, UIKit"</label> }
}

fn main() {
    leptos::mount_ios::run(|| view! { <App /> });
}
```

Copy `examples_ios/counter/run_ios.sh` and replace the bundle-name /
bundle-id references. Build and launch with `./run_ios.sh`.

Unlike the macOS port, iOS has only one entry point — `run`. There's
no `mount_to_window` builder because iPhone apps run as a single
fullscreen scene, and iPad multi-window is system-initiated via
`UISceneSession`, not declared at launch.

## Available controls and attributes

Element tags are snake_case (deliberate convention — see
[CLAUDE.md](CLAUDE.md)):

| Tag                     | UIKit class            | Notes                               |
|-------------------------|------------------------|-------------------------------------|
| `<view>`                | `UIView`               | generic flexbox container           |
| `<vstack>` / `<hstack>` | `UIView`               | preset flex_direction               |
| `<button>`              | `UIButton` (system)    |                                     |
| `<switch>`              | `UISwitch`             | iOS uses switches, not checkboxes   |
| `<label>`               | `UILabel`              | non-editable                        |
| `<text_field>`          | `UITextField`          | single-line editable                |
| `<secure_text_field>`   | `UITextField`          | with `isSecureTextEntry = true`     |
| `<text_view>`           | `UITextView`           | multi-line editable, scrolls        |
| `<slider>`              | `UISlider`             | continuous (drag fires)             |
| `<stepper>`             | `UIStepper`            | +/- numeric                         |
| `<segmented_control>`   | `UISegmentedControl`   | items + selection                   |
| `<date_picker>`         | `UIDatePicker`         | `.style()` for Wheels/Compact/Inline|
| `<progress_indicator>`  | `UIProgressView`       | determinate bar (0..1)              |
| `<image_view>`          | `UIImageView`          | source = local file path            |
| `<scroll_view>`         | `UIScrollView`         | generic over children               |

Attributes:

- **Layout**: `padding=N`, `gap=N`, `flex_grow=N`, `flex_direction=…`
- **Static**: `title=…`, `value=…`, `placeholder=…`, `min_value=N`,
  `max_value=N`, `items=vec![…]`, `selection=N`
- **Bool**: `enabled=true|closure`, `checked=true|closure`
- **Universal**: `alpha=…`, `text_color=…`, `alignment=…`, `font_size=…`
- **Events**: `on:click`, `on:input` (text), `on:change` (text),
  `on:focus`, `on:blur`
- **Two-way bind**: `bind:value=signal` (text/slider/stepper/
  date_picker/text_view), `bind:checked=signal` (switch),
  `bind:selection=signal` (segmented_control)

The `on:event` list is constrained at compile time per builder type
via the `SupportsEvent<E>` trait — `<button on:input=…>` won't
compile.

For `<switch>` / `<slider>` / `<stepper>` / `<segmented_control>` /
`<date_picker>`, use `on:click` to react to value changes — UIKit
fires `UIControlEventValueChanged`, which the iOS port routes
through the same target/action machinery as a button tap. (Or use
`bind:` for the more declarative path.)

## What works

- Layout: flexbox via Taffy, padding/gap, flex_grow.
- Safe area: status bar / notch / home indicator avoided automatically
  via `view.safeAreaInsets`.
- Keyboard avoidance: when the keyboard appears, content gets a
  bottom inset to stay visible. Driven by `UIKeyboardLayoutGuide`.
- Rotation / split-view: `viewDidLayoutSubviews` re-runs Taffy on
  every bounds change.
- Single-fullscreen on iPhone, iPad fullscreen.

## Known limitations

- **Single scene only.** iPad multi-window is deferred — would need
  a `Scene` builder integrated with `UISceneDelegate`.
- **No modern scene delegate.** Window setup uses the
  `UIApplicationDelegate.window` path with `#[allow(deprecated)]`.
  iOS 15–18 still accept it; eventually we should switch to a
  `UISceneDelegate` registered via `UIApplicationSceneManifest`.
  Tracked in [TODO_ios.md](TODO_ios.md) (P3 / 3a).
- **No hardware-keyboard events.** `on:keydown` / `on:keyup` on text
  fields are no-ops on iOS; software keyboard captures everything.
  `UIKeyCommand` / `pressesBegan:` integration is a future stage.
- **No Dynamic Type / VoiceOver polish.** Defaults work; explicit
  Dynamic Type scaling on `font_size` and richer `accessibilityLabel`
  configuration are deferred.
- **No dark-mode reactivity.** `UIColor.systemBackgroundColor`
  adapts to light/dark automatically, but custom colours don't yet
  re-fire effects on `traitCollectionDidChange:`.
- **No navigation / tab / list builders.** UINavigationController,
  UITabBarController, UICollectionView aren't wrapped yet.
- **No `mount_to_window`.** iOS has no user-facing window concept;
  use `run`.

See [TODO_ios.md](TODO_ios.md) for the full list.

## Where to look next

- **[implementation_ios.md](implementation_ios.md)** — design-decision
  journal. Newest entries at the top.
- **[audit_ios.md](audit_ios.md)** — running audit / status doc.
- **[TODO_ios.md](TODO_ios.md)** — priority-ordered outstanding work.
- **[CLAUDE.md](CLAUDE.md)** — architecture overview written for AI
  agents but useful for human onboarding too. Covers the three-layer
  structure (`ios_dom` / `tachys::ios` / `*_ios` facades) and the
  conventions / gotchas.
- **[implementation_log.md](implementation_log.md)** — the macOS
  port's design journal. Many concepts (Taffy bridging, event-handler
  storage, the `mount_before` synthetic-parent dance) are shared.
