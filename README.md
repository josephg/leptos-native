# leptos-native

This project is a fork of [Leptos](https://leptos.dev), targetting native UI rendering instead of the web.

Here's an app:

```rust
// Cargo.toml: leptos = { package = "leptos_cocoa" }   (or leptos_uikit / leptos_gtk)
use leptos_native::prelude::*;

#[component]
fn Counter() -> impl IntoView {
    let count = RwSignal::new(0);
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
    mount_to_window("Counter", (320.0, 200.0), || view! { <Counter /> });
}
```

This app can run on any of our supported platforms: Currently Linux (via GTK), MacOS (Cocoa) and iOS (UIKit).

Unlike many other UI libraries for rust (eg Slint), leptos-native does not implement our own renderer. We do not invent our own library of UI components. This library wraps the native rendering & component libraries of our target platforms. As a result:

- There are inconsistencies in our API and rendering results between platforms. GTK apps look like GTK apps, because we're using GTK under the hood. MacOS apps look native because we're using apple's own UI toolkit.
- There are platform differences. Some components (eg SplitView, Toolbar) are only provided by a single platform.
- Your code will probably not work unmodified across all our supported platforms. This is expected. Down the road we'll provide some examples of how to write cross-platform code.
- Binary sizes are tiny. A Hello World Electron app is hundreds of megabytes. A hello world leptos-native app is about 500kb. (Honestly thats still too big, but much better).

We do make some affordances to platform consistency:

- Common components (buttons, labels, etc) have standard attributes wherever that makes sense.
- We use [Taffy](https://taffylayout.com/) for layout instead of the platform-native layout engines. Taffy provides *Flexbox*, *Grid* and *Block* layout primitives.


## Why

Over the last 20 years, web UI frameworks have gotten better than native UI component libraries. On the web, we have:

- Pure components
- Reactivity
- Declarative layout
- Signals

This is thanks to the constant innovation cycle of Elm, jQuery, Angular, React, Svelte, SolidJS and many many others.

Its gotten to the point that people are making desktop applications out of web browsers. Discord. Spotify. Slack. MS Teams. And so on. These applications are huge downloads, and performance is terrible. But developers figure its easier to spend your RAM than spend their own time. What selfish rubbish. Especially in the era of AI RAM shortages. Electron apps (and friends) also never quite look or feel like native applications. I think my mac should feel like a mac. GTK has a whole thing going on too. Applications should fit in to the look and feel of the operating system. Not replace it.

Another answer is to use native UI replacement libraries. Like egui, Slint, Cushy and so on. These libraries reinvent their own look and feel from scratch. They almost always mess up native keyboard shortcuts and break accessibility (eg screen readers). I want native apps on my phone and laptop to look and feel like part of the ecosystem. I don't want every application to invent its own tab bar, or reinvent what buttons look like or how they work. I want screen readers to work correctly using the native platform libraries.

Hence Leptos-native.

Combine the best recent ideas from web development (SolidJS's signals, reactivity, declarative views) with native controls (native cocoa, GTK, iOS, etc platform support). Tiny binaries (everything is just native code). Low memory footprint. Platform-native look and feel. Native platform accessibility support. Extensibility (if we don't support some macos component you need, you can just add it). Its great!


## Why a fork

Greg (Leptos author & maintainer) said he'd rather this code was in a fork than upstreamed.

We're still using upstream `reactive_graph` and signal code. We just changed what the signals actually do.


## Workspace layout

```
common/
  leptos_native/         ← the core crate: IntoView<R>, Show/For/
                            ErrorBoundary/Provider, children, mount
                            machinery. NO RenderHtml, NO SSR.
    src/renderer/        ← the `renderer` module (was the standalone
                            `tachys` / `common/renderer` crate; now
                            inlined here): Render<R>, Mountable<R>,
                            Renderer trait, the Taffy LayoutTree<B>,
                            and shared attribute plumbing.
  leptos_macro/          ← view!{}, #[component] proc macros
  devtools/              ← `leptos_devtools`: CDP devtools server

# reactive_graph / reactive_stores are now plain crates.io deps
# (0.3.0-alpha / 0.5.0-alpha), no longer vendored under common/.

cocoa/
  leptos_cocoa/          ← macOS Renderer impl + tachys-shaped
                            re-export tree + builder API (button(),
    src/dom/             ← vstack(), …). The `dom` module (was the
                            standalone `cocoa_dom` crate) is the
                            DOM-shaped façade over AppKit.
  examples/              ← working examples (counter, counters,
                            error_boundary, etc.)
uikit/
  dom/                   ← `ios_dom`: same façade over UIKit (still a
                            standalone crate — not yet inlined)
  leptos_uikit/          ← iOS Renderer impl
  examples/              ← working iOS examples
  xcuitests/             ← Swift XCTest harness (macOS host;
                            re-targeting to iOS sim still TODO)
gtk/
  leptos_gtk/            ← GTK4 Renderer impl + builder API
    src/dom/             ← the `dom` module (was `gtk_dom`): GTK4 façade
  examples/
```

The per-port `dom` façades for cocoa and gtk used to be separate
crates (`cocoa_dom`, `gtk_dom`); they're now the `dom` **module** of
`leptos_cocoa` / `leptos_gtk`. The iOS façade (`ios_dom`) is still
its own crate at `uikit/dom/`.

## Per-platform getting started

- **macOS** — see [`README_macos.md`](./README_macos.md). AppKit via
  the [`objc2`](https://crates.io/crates/objc2) crate family. Layout
  via [Taffy](https://crates.io/crates/taffy).
- **iOS** — see [`README_ios.md`](./README_ios.md). UIKit; same
  layout engine. Each example ships a `run_ios.sh` that bundles a
  `.app`, installs on the booted simulator, and launches.
- **Linux** — see [`README_gtk.md`](./README_gtk.md). GTK4 via
  [`gtk4-rs`](https://crates.io/crates/gtk4); uses the same Taffy
  layout engine as cocoa/iOS, plugged into GTK via a custom
  `gtk::LayoutManager`.

## Build & test

```sh
# Workspace (cocoa side + common crates)
cargo build --workspace
cargo test                                   # 53 binaries, ~250 tests

# Run a cocoa example
cargo run --manifest-path cocoa/examples/counter/Cargo.toml

# iOS — needs a booted simulator
cd uikit/examples/counter && ./run_ios.sh         # interactive
cd uikit/examples/counter && ./run_ios.sh -t 3    # auto-terminate after 3s

# iOS layout regression tests run on the simulator
cargo test --manifest-path uikit/dom/Cargo.toml \
  --target aarch64-apple-ios-sim --test layout
```

## What's working / what isn't

- **macOS:** 18/22 examples build and run. Counter, counters,
  greeter, persistent_counter, error_boundary, two_windows
  (multi-window), showcase (every supported control), all
  end-to-end.
- **iOS:** 10/10 examples build for the simulator. Counter etc.
  launch + render correctly via `run_ios.sh`.
- **Linux/GTK:** the `leptos_gtk` builder layer (analogous to
  `leptos_cocoa` / `leptos_uikit`) now exists and drives the GTK
  examples via the shared Taffy layout engine. Still maturing
  relative to the macOS port.
- ❌ **Deferred features** (kept off the punch list pending design
  work):
  - `<Slots>` — multi-named-children components.
  - `Resource` / `Suspense` — async-data view rendering. Upstream
    leans on the SSR streaming story; native equivalents need a
    different design.
  - `<Transition>` / `<AnimatedShow>` — need CoreAnimation
    (macOS/iOS) or GTK transition integration.
  - Type-erased `AnyView` / untyped `Children`. Components currently
    take `TypedChildren<C, R>` with a generic `C`.
  - Keyed `<For>` diffing. The current `<For>` is unkeyed
    (position-based diff via `Vec<T>: Render<R>`); rows reorder
    correctly but per-row state can't follow keys yet.

## Layout reference

The native-side architecture is documented in three places:

- **[`CLAUDE.md`](./CLAUDE.md)** — Top-level architecture, conventions,
  failure-mode hierarchy, per-port specifics. Reference for ongoing
  work.
- **[`implementation_log.md`](./implementation_log.md)** —
  Chronological design-decision log for the macOS port. Newest
  entries at top; critical context for layout / eventing / multi-
  window / macro plumbing.
- **[`gtk_implementation_log.md`](./gtk_implementation_log.md)**,
  **[`implementation_ios.md`](./implementation_ios.md)** — same shape
  for the GTK + iOS ports.
