# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this fork is

This is a **macOS-native port of Leptos** in progress. The web Leptos framework lives intact on the same branch (web examples still build via wasm targets), but on `cfg(target_os = "macos")` the renderer is swapped out so the same `view!{}` / `#[component]` / signals API drives an AppKit UI instead of the DOM.

Read these before diving in:
- `implementation_log.md` — chronological design-decision journal (newest at top). Critical context for anything related to layout, eventing, multi-window, or the macro plumbing.
- `tests.md` — comprehensive test plan (XCUIAutomation harness deferred; useful to see what behaviors exist).
- `ARCHITECTURE.md` — upstream Leptos architecture (web-focused, but explains the reactive system and renderer layering this port reuses).

## Build & run

The macOS port has no test runner yet; the iteration loop is "build an example and click around."

```sh
# Build & run a specific example (current state; more under examples/*_macos):
cargo run --manifest-path examples/counter_macos/Cargo.toml
cargo run --manifest-path examples/counters_macos/Cargo.toml
cargo run --manifest-path examples/greeter_macos/Cargo.toml
cargo run --manifest-path examples/checkbox_macos/Cargo.toml

# Just typecheck a crate (the workspace is huge — scope to what changed):
cargo build --manifest-path cocoa_dom/Cargo.toml
cargo build --manifest-path tachys/Cargo.toml
```

The non-macOS Leptos crates (`leptos`, `tachys`, `reactive_graph`, etc.) live in the workspace root and are still active — when you change `tachys` or `leptos` for the macOS path, make sure the web-target build still compiles too if you're touching shared code.

## Architecture of the macOS port

Three layers, lowest first:

### `cocoa_dom/` — DOM-shaped façade over AppKit

The lowest layer. Provides `Node`, `Element`, `Text`, `Placeholder` types that loosely mirror their `web_sys` equivalents in shape but are backed by `NSView` (and subclasses like `NSButton`, `NSTextField`). Also owns:

- **Layout** (`layout.rs`): each window has its own `TaffyTree` (`LayoutTree { tree, root: Option<NodeId> }`). Every `Node` carries a `LayoutHandle { tree, node_id }`. Layout recompute is **manual** — AppKit doesn't auto-reflow, so `set_attribute` / `set_text` / `attach_child` / etc. each call `schedule_relayout`, which dedupes via thread-local `PENDING` and dispatches one `compute_layout` pass per main-loop tick. **Always `tree.mark_dirty(node_id)` when content changes** (otherwise Taffy's measure cache is stale).
- **Events** (`event.rs`): NSButton uses `ActionTarget` (target/action). NSTextField uses a single `TextFieldDelegate` that fans out to `Vec<Box<dyn FnMut(String)>>` for both `controlTextDidChange:` (input) and `controlTextDidEndEditing:` (change). Each per-view delegate retain is stashed in a thread-local store (entries currently leak; see `tests.md` / log).
- **Spawner** (`spawner.rs`): `any_spawner::CustomExecutor` backed by `DispatchQueue::main()`. Pin soundness: don't add an outer `Pin` — the inner `Pin<Box<dyn Future>>` already has a stable address.
- **Window** (`window.rs`): `open_window` returns an `OpenedWindow` with the NSWindow, content_root `Element` (a `FlippedView` for top-left coords), the new `TreeRef`, and the resize delegate.
- **App** (`app.rs`): `init_app(mtm)` builds NSApp + menu bar + AppDelegate (returns true from `applicationShouldTerminateAfterLastWindowClosed:`).

Everything in this crate panics off the main thread; `SendWrapper` enforces it at runtime.

### `tachys/src/cocoa/` — bridges cocoa_dom to tachys' `Render`/`Mountable` traits

- `element.rs`: `Button`, `Checkbox`, `Label`, `TextField`, `View<Ch>` builder structs with `.title()` / `.value()` / `.child()` / `.on(event, handler)` / `.bind(key, signal)` methods, plus `Render::build` impls. **Children are deliberately NOT mounted at build time** — mounting is deferred until `ElementState::mount` runs. This is the cascade pattern that lets tree-aware `insert_node` register each child in the right Taffy tree as it goes.
- `attr.rs`: `MaybeReactive<T>` (Static or Reactive closure) + `IntoMaybeReactive<T>` + `install` helper that wraps a closure in a `RenderEffect`.
- `bind.rs`: `IntoSignal<T>` trait + per-control `BindAttribute` impls. `bind:value` on text_field and `bind:checked` on checkbox both wire **two directions** — outgoing via the AppKit observer, incoming via a `RenderEffect`.
- `render_html_stub.rs`: `cocoa_stub_view_impls!` macro emits no-op `RenderHtml`/`AddAnyAttr` impls. These exist purely to satisfy `IntoView`'s supertrait bound (`Render + RenderHtml + Send`) — SSR is unreachable on native. Eventually we want to feature-flag `RenderHtml` out of `IntoView` instead; tracked in the implementation log.

### `tachys/src/html/{element,event}_macos.rs` + `tachys/src/svg_macos.rs` — macro facades

The `view!{}` macro emits paths like `tachys::html::element::button()`, `tachys::html::event::on(event::click, handler)`, etc. These facade modules re-export the cocoa builders at the paths the macro expects. **Don't change the macro** — just expand the facades when adding new tags or events.

The `view` element is a real SVG tag, so the macro routes `<view>` through `tachys::svg::view`; `tachys/src/svg_macos.rs` aliases it back to the cocoa container.

### `leptos/src/mount_macos.rs` — entry points

`run(closure)` and `mount_to_window(title, size, closure)`. Currently leaks the `Owner` (no real `UnmountHandle` story for window close).

## Conventions and gotchas

- **Tag names are snake_case** by deliberate choice, even when they correspond to PascalCase NSView types (so the macro's auto-routing works). Live with the convention clash.
- **`set_attribute` diffs against current AppKit state** before mutating, e.g. `setStringValue:` only fires if the value actually changed. This is the macOS analog of the browser's natural same-value grace, and protects against focus-ring flashes on `bind:` cycles. Keep it.
- **`<text_field>` forces width=0 in its measure callback** so the parent decides the width. Otherwise the field grows with each keystroke (its intrinsic width tracks content). Don't "fix" this without understanding the resize cascade.
- **`Placeholder` defaults to `position: Absolute`** so it doesn't take a flex slot — `Render for ()` builds a Placeholder, and many tachys constructs leave them lying around.
- **NSButton needs `buttonWithTitle:target:action:`**, not `initWithFrame:` — the latter gives a default bezel with bad intrinsic sizing (titles get clipped: "Reset" → "Rese").
- **`HTML compatibility is a non-goal.**" The UI is built specifically as macOS apps. We are free to invent tags (`<vstack>`, `<hstack>`, `<checkbox>`) without worrying about HTML semantics.
- **Don't put new tachys cocoa code behind `cfg(feature = "...")`** — the macOS path is gated by `cfg(target_os = "macos")` at the workspace level. Web-only deps are gated to `cfg(not(target_os = "macos"))` in `tachys/Cargo.toml`.

## When you change something

- **If you add a new control**: builder in `tachys/src/cocoa/element.rs` + tag handling in `cocoa_dom/src/node.rs::Element::create_with` + facade re-export in `tachys/src/html/element_macos.rs` + `cocoa_stub_view_impls!` for the new builder.
- **If you add a new event**: `EventDescriptor` impl + `PendingHandler` variant in `tachys/src/html/event_macos.rs` + corresponding install hook in `cocoa_dom/src/event.rs` and a passthrough method on `cocoa_dom::Element`.
- **If you change layout behavior**: re-test resize on at least `counter_macos` (static) and `counters_macos` (dynamic add/remove). Layout regressions are the most common breakage.
- **Always log non-obvious decisions in `implementation_log.md`** with a clear rationale and the alternatives considered. The log is how future-you (and other instances) understand why something is the way it is.
