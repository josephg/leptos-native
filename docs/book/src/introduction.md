# Introduction

**Leptos Native** is a native-only fork of [Leptos](https://leptos.dev),
the fine-grained reactive Rust framework created by Greg Johnston
and contributors. This fork keeps everything that makes Leptos
pleasant — the `view!{}` macro, `#[component]` functions, signals,
effects, memos, error boundaries, keyed iteration with `<For>` —
and replaces the web (DOM / WASM / SSR) renderer with three native
UI backends:

- **macOS** via AppKit (Cocoa)
- **iOS** via UIKit
- **Linux** via GTK4

Your `<button>` becomes an `NSButton`, `UIButton`, or
`gtk::Button`. Your `<text_field>` becomes an `NSTextField`,
`UITextField`, or `gtk::Entry`. Your `bind:value` two-way binds a
signal to a real native control. Layout is driven by
[Taffy](https://github.com/DioxusLabs/taffy), so flexbox and grid
work the way you'd expect them to from the web.

There is no embedded WebView, no WASM, no client-server split, no
hydration. Your app is a Rust binary that links AppKit, UIKit, or
GTK directly.

```rust
use leptos::prelude::*;

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

## Credits

This fork would not exist without the [Leptos](https://github.com/leptos-rs/leptos)
project. Greg Johnston designed the reactive system, the `view!{}`
macro, and the `tachys` renderer that this fork's `common/renderer`
crate is unbundled from; the broader Leptos contributor community
built the surrounding crates. The reactive primitives
(`RwSignal`, `Signal`, `Memo`, `Effect`), the component machinery,
control-flow components (`<For>`, `<Show>`, `<ErrorBoundary>`,
`<Switch>` / `<Match>`), and the reactive stores crate all come
from upstream Leptos, lightly adapted.

The native backends, layout integration with Taffy, and per-platform
plumbing (windows, menus, toolbar, split view, safe-area handling,
etc.) are the contribution of this fork.

## What's not here

The following upstream Leptos features are intentionally **not part
of this fork**:

- **No SSR / hydration / WASM.** This is a native binary, not a
  webapp.
- **No router or `leptos_meta`.** Navigation patterns are
  platform-specific (split view + toolbar on macOS, navigation
  stack on iOS, etc.).
- **No server functions, `Resource`, or `Suspense`.** There is no
  client-server boundary to bridge. Async is still supported via
  `AsyncDerived` and `spawn_local`.
- **No `<Transition>`, `<AnimatedShow>`, or `<Slots>`** yet —
  these are on the punch-list.

The [Migrating from Web Leptos](./appendix/migration.md) appendix
covers these in more detail.

## How to read this book

If you've used Leptos on the web, you can skim Parts 1 and 2 —
the component model and reactivity story are unchanged. Spend your
time on:

- **[Layout](./layout/flexbox.md)** — flexbox/grid replace CSS.
- **[Element Reference](./elements/README.md)** — native widgets
  with their real attribute lists.
- **[Platform Features](./platform/cocoa/README.md)** — menus,
  toolbar, split view, safe-area, etc.

If you're new to Leptos entirely, read in order. The
[Getting Started](./getting_started/README.md) chapter for your
platform shows you the prerequisites and first build.
