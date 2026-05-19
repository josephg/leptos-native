# leptos-native

A native-only fork of [Leptos](https://leptos.dev) targeting macOS
(AppKit), iOS (UIKit), and Linux (GTK4). The same `view!{}` macro,
`#[component]` attribute, and fine-grained reactive signals you'd
use on the web drive a real native UI on each platform — no
embedded WebView, no client-server split, no WASM.

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

That program produces a native AppKit window with three NSButtons
on macOS (or three UIButtons on iOS, or three GtkButtons on Linux,
depending on which `leptos_<platform>` crate the example depends on).

## Why a fork

Upstream Leptos is built around the SSR/hydration shape: server
renders HTML, the client hydrates an interactive shell, server
functions bridge the two. That whole architecture is the wrong fit
for a native app — there's no server, no DOM, no WASM-in-a-browser.
This fork **deletes** the web-specific layers (`tachys/html`,
`leptos_router`, `leptos_meta`, `server_fn`, `hydration_context`,
SSR/CSR/hydrate Cargo features, …) and replaces the renderer with
target-specific platform impls.

Greg Johnston (Leptos author) gave explicit blessing to fork rather
than upstream — the divergence is too sharp for a shared codebase.

## Workspace layout

```
common/
  reactive_graph/        ← unchanged from upstream (vendored)
  reactive_stores/       ← unchanged
  reactive_stores_macro/ ← unchanged
  renderer/              ← was tachys; stripped + R-genericized
                            (Render<R>, Mountable<R>, Renderer trait)
  leptos_macro/          ← view!{}, #[component] proc macros
  leptos/                ← renderer-agnostic core: IntoView<R>,
                            Show/For/ErrorBoundary/Provider, children,
                            etc. NO RenderHtml, NO AnyView, NO SSR.
cocoa/
  dom/                   ← `cocoa_dom`: DOM-shaped façade over AppKit
  leptos_cocoa/          ← `leptos_cocoa`: macOS Renderer impl +
                            tachys-shaped re-export tree + builder
                            API (button(), vstack(), …)
  examples/              ← 18 working examples (counter, counters,
                            error_boundary, etc.)
uikit/
  dom/                   ← `ios_dom`: same shape over UIKit
  leptos_uikit/          ← `leptos_uikit`: iOS Renderer impl
  examples/              ← 10 working iOS examples
  xcuitests/             ← Swift XCTest harness (macOS host;
                            re-targeting to iOS sim still TODO)
gtk/
  dom/                   ← `gtk_dom`: GTK4 façade
  examples/              ← Stage 0–4 examples; tachys-builder layer
                            in progress
```

## Per-platform getting started

- **macOS** — see [`README_macos.md`](./README_macos.md). AppKit via
  the [`objc2`](https://crates.io/crates/objc2) crate family. Layout
  via [Taffy](https://crates.io/crates/taffy).
- **iOS** — see [`README_ios.md`](./README_ios.md). UIKit; same
  layout engine. Each example ships a `run_ios.sh` that bundles a
  `.app`, installs on the booted simulator, and launches.
- **Linux** — see [`README_gtk.md`](./README_gtk.md). GTK4 via
  [`gtk4-rs`](https://crates.io/crates/gtk4); GTK does its own
  layout so no Taffy bridge here.

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

- ✅ **macOS:** 18/22 examples build and run. Counter, counters,
  greeter, persistent_counter, error_boundary, two_windows
  (multi-window), showcase (every supported control), all
  end-to-end.
- ✅ **iOS:** 10/10 examples build for the simulator. Counter etc.
  launch + render correctly via `run_ios.sh`.
- 🚧 **Linux/GTK:** lower-level direct-`gtk_dom` examples work; the
  `leptos_gtk` builder layer (analogous to `leptos_cocoa` /
  `leptos_uikit`) is the next port.
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
