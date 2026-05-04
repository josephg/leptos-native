# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this fork is

This fork extends Leptos with **two native UI ports** in progress:

- **macOS / AppKit (Cocoa)** — the original, more mature port.
- **Linux / GTK4** — newer, mirrors the Cocoa port's structure with
  GTK-specific simplifications (no Taffy bridge, signal-based events
  instead of target/action).

The web Leptos framework lives intact on the same branch (web
examples still build via wasm targets, SSR works against axum/actix).
The native UI path is **opt-in via the `native-ui` Cargo feature**;
once enabled, `target_os` picks the backend automatically (macOS →
Cocoa, Linux → GTK).

Read these before diving in:
- `implementation_log.md` — chronological design-decision journal for
  the **macOS** port (newest at top). Critical context for anything
  related to layout, eventing, multi-window, or the macro plumbing.
- `gtk_implementation_log.md` — same shape, for the **Linux/GTK**
  port. Read whichever is relevant to what you're touching; both if
  you're touching shared code.
- `README_gtk.md` — user-facing overview of the GTK port: status,
  prerequisites, examples, the `native-ui` feature flag.
- `tests.md` — comprehensive test plan for the macOS port
  (XCUIAutomation harness deferred). GTK has no test plan yet.
- `ARCHITECTURE.md` — upstream Leptos architecture (web-focused, but
  explains the reactive system and renderer layering both ports
  reuse).

### System documentation

The GTK4 and GLib C reference docs are installed on this system at:

- **GTK4:** `/usr/share/doc/libgtk-4-doc/gtk4/`
- **GLib:**  `/usr/share/doc/libglib2.0-dev/glib/`
- **GIO:**   `/usr/share/doc/libglib2.0-dev/gio/`

These are the **canonical docs** for GLib async primitives —
`g_main_context_invoke`, `g_idle_add`, `g_source_attach`,
`g_source_remove`, `GSource`, `GMainLoop` — and their exact
semantics (ownership, inline-vs-deferred dispatch, thread-safety,
auto-remove-on-return-FALSE). Always consult them before writing
or debugging spawner / event-loop code. The Rust bindings (`glib`
crate, `gtk4` crate) are thin wrappers; the C docs are the
authoritative reference for behavior.

Key pages to know:
- `method.MainContext.invoke.html` — inline vs idle dispatch rules
- `func.idle_add.html` — always-deferred idle callback (attaches to
  global-default context)
- `func.idle_add_full.html` — priority + `GDestroyNotify` variant
- `struct.MainContext.html` — ownership, acquire/release, thread-
  default contexts
- `struct.MainLoop.html` — run-loop lifecycle during `app.run()`

## Build & run

Neither native port has a test runner yet; the iteration loop is
"build an example and click around."

### macOS / Cocoa

```sh
# Build & run a specific example (current state; more under examples/*_macos):
cargo run --manifest-path examples_cocoa/counter/Cargo.toml
cargo run --manifest-path examples_cocoa/counters/Cargo.toml
cargo run --manifest-path examples_cocoa/greeter/Cargo.toml
cargo run --manifest-path examples_cocoa/checkbox/Cargo.toml

# Just typecheck a crate (the workspace is huge — scope to what changed):
cargo build --manifest-path cocoa_dom/Cargo.toml
cargo build --manifest-path tachys/Cargo.toml
```

### Linux / GTK

System prereqs: `libgtk-4-dev` + `pkg-config` (Debian/Ubuntu;
analogues elsewhere). See `README_gtk.md` for distro-specific
commands.

```sh
# Low-level examples that don't go through the tachys builder layer
# yet (Stages 0-1 only ship these):
cargo run -p gtk_dom --example hello_window
cargo run -p gtk_dom --example counter

# Build & run examples using view!{} + #[component]:
cargo run --manifest-path examples/counter_gtk/Cargo.toml
cargo run --manifest-path examples/greeter_gtk/Cargo.toml
cargo run --manifest-path examples/checkbox_gtk/Cargo.toml
cargo run --manifest-path examples/login_form_gtk/Cargo.toml
cargo run --manifest-path examples/settings_gtk/Cargo.toml
cargo run --manifest-path examples/counters_gtk/Cargo.toml

# Typecheck the GTK façade:
cargo build -p gtk_dom

# Typecheck tachys against the native path (Linux → GTK, macOS → Cocoa):
cargo check -p tachys --features native-ui

# Typecheck the full leptos stack against the native path:
cargo check -p leptos --features native-ui
```

The `native-ui` Cargo feature is what tells tachys/leptos to use the
native renderer. Without it (the default), all crates compile against
the web/SSR path even on macOS/Linux.

### Workspace / web/SSR

The non-native Leptos crates (`leptos`, `tachys`, `reactive_graph`,
`leptos_router`, `leptos_meta`, integrations) all build clean against
the default features:

```sh
cargo build --workspace --exclude cocoa_dom
```

`cocoa_dom` is excluded because its source is `#![cfg(target_os =
"macos")]` (compiles to empty on Linux, but sometimes pkg-config etc.
churns on it). On macOS the equivalent excludes `gtk_dom`.

When you change `tachys` or `leptos` for either native port, make
sure both the **default workspace build** (web/SSR mode) and the
**`--features native-ui` build** still compile. Touching shared code
without checking both is the usual way to break the other backend.

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

## Architecture of the Linux/GTK port

Mirrors the macOS layering one-for-one in shape; the main difference
is that **GTK does its own layout**, so the entire Taffy bridge from
the macOS port disappears here. Three layers, lowest first:

### `gtk_dom/` — DOM-shaped façade over GTK4

Provides `Node`, `Element`, `Text`, `Placeholder` types that loosely
mirror their `web_sys` equivalents in shape but are backed by
`gtk::Widget` (and subclasses like `gtk::Button`, `gtk::Entry`,
`gtk::Box`). Modules:

- **Node + Element** (`node.rs`): `Element::create(tag)` maps tag
  names to GTK widget classes (see `README_gtk.md` for the table).
  `set_attribute` / `set_bool_attribute` route to the appropriate
  widget setter (`set_label`, `set_text`, `set_sensitive`,
  inverted `set_visible`, `set_active`). `insert_node` /
  `remove_child` / `clear_children` handle `gtk::Box` and
  `gtk::Window`/`gtk::ApplicationWindow` parents (other container
  classes are silently dropped — extend as needed).
- **No layout module.** `gtk::Box` with orientation/spacing/margin
  setters does the work the macOS port does in Taffy. Containers
  with `<vstack>` / `<hstack>` / `<view>` map directly to
  `gtk::Box::new(orientation, spacing)`.
- **Events** (currently inline on `Element`; `event.rs` extraction
  is Stage 3 work): `on_click` calls `gtk::Button::connect_clicked`.
  Closures are owned by the signal connection itself (which is owned
  by the widget), so there is **no thread-local handler store** like
  cocoa_dom's `keep_target_alive` — the closure drops with the
  widget.
- **Spawner** (`spawner.rs`): `any_spawner::CustomExecutor` over
  `glib::MainContext::default().spawn_local`. Both `spawn` and
  `spawn_local` route through `spawn_local`; glib drives polling on
  the main loop GTK is already draining.
- **Window** (`window.rs`): `open_window(app, title, size)` returns
  an `OpenedWindow` with the `GtkApplicationWindow` and a
  `content_root: Element` (a vertical `gtk::Box` set as the window's
  child).
- **App** (`app.rs`): `init_app(application_id)` builds a
  `gtk::Application` and registers the spawner. Application IDs are
  configurable per-call (no global default).

`gtk::Widget` is `!Send`; `SendWrapper` wraps each `Node` so the type
is nominally `Send + 'static` for tachys's generic plumbing, with a
runtime panic if accessed off-main.

### `tachys/src/renderer/gtk.rs` — bridges `gtk_dom` to tachys' `Render`/`Mountable` traits

Same shape as `tachys/src/renderer/cocoa.rs`. The `Dom` unit struct
delegates the imperative API to `gtk_dom::Renderer`, plus
`Mountable` and `CastFrom` impls for orphan-rule reasons.

`mount_before` is a one-liner — `widget.parent()` → wrap as a
synthetic `Element` → `mount` — versus cocoa's elaborate
`synthesise_parent_element` + `LayoutHandle` propagation. No Taffy
tree to register against.

### `tachys/src/gtk/` — element builders *(Stage 5, not yet built)*

When this lands, it will mirror `tachys/src/cocoa/` with `Button`,
`Checkbox`, `Label`, `TextField`, `Slider`, `PopUpButton`,
`View<Ch>` builder structs. For now the GTK side has no high-level
`Render` builders; users build view trees against `gtk_dom`
directly.

### `tachys/src/html/element_gtk.rs` + `event_gtk.rs` *(Stage 5, not yet built)*

Macro facades — same role as `element_macos.rs` / `event_macos.rs`.
Until these exist, the `view!{}` macro doesn't resolve on Linux
native.

### `leptos/src/mount_gtk.rs` *(Stage 5, not yet built)*

Will provide `run(closure)` and `mount_to_window(app_id, title,
size, closure)`. Until then, callers handle the `init_app` +
`connect_activate` boilerplate themselves (see
`gtk_dom/examples/counter.rs`).

## Conventions and gotchas

### Shared (both ports)

- **Tag names are snake_case** by deliberate choice, even when they correspond to PascalCase widget types (so the macro's auto-routing works). Live with the convention clash.
- **`set_attribute` diffs against current widget state** before mutating, on both backends. This protects against `bind:` cycles (an `Effect`-driven write firing the widget's change signal that re-fires the effect) and against focus-ring flashes. Keep it.
- **HTML compatibility is a non-goal.** UIs are built specifically as native apps. We are free to invent tags (`<vstack>`, `<hstack>`, `<checkbox>`) without worrying about HTML semantics.
- **The `native-ui` Cargo feature** is what flips the `cfg(leptos_native)` flag in source. *Don't* add ad-hoc `cfg(feature = "...")` blocks for the cocoa or gtk paths — use `cfg(leptos_native)` for the web/native split, and `cfg(target_os = "macos")` / `cfg(target_os = "linux")` to disambiguate within native code. Code that depends on the optional `cocoa_dom` / `gtk_dom` deps must be gated on `cfg(all(target_os = "X", leptos_native))`, otherwise it'll try to compile when the feature is off and the dep isn't pulled in.
- **Web-only crates** (`leptos_router`, `leptos_meta`, `integrations/*`) gate their lib.rs on `cfg(not(leptos_native))` so they compile to empty rlibs when `native-ui` is enabled. Defensive — a native binary normally shouldn't depend on them.

### macOS / Cocoa specifics

- **`<text_field>` forces width=0 in its measure callback** so the parent decides the width. Otherwise the field grows with each keystroke (its intrinsic width tracks content). Don't "fix" this without understanding the resize cascade.
- **`Placeholder` defaults to `position: Absolute`** so it doesn't take a flex slot — `Render for ()` builds a Placeholder, and many tachys constructs leave them lying around.
- **NSButton needs `buttonWithTitle:target:action:`**, not `initWithFrame:` — the latter gives a default bezel with bad intrinsic sizing (titles get clipped: "Reset" → "Rese").
- **Layout recompute is manual**: AppKit doesn't auto-reflow, so `set_attribute` / `set_text` / `attach_child` / etc. each call `schedule_relayout`, which dedupes via thread-local `PENDING` and dispatches one `compute_layout` pass per main-loop tick. **Always `tree.mark_dirty(node_id)` when content changes** (otherwise Taffy's measure cache is stale).

### Linux / GTK specifics

- **No Taffy.** GTK's natural layout (`gtk::Box` with orientation +
  spacing + margin + `set_hexpand`/`set_vexpand`) covers the
  SwiftUI-flavoured surface area. Don't add a layout module unless
  GTK's natives prove insufficient for some specific case (none yet).
- **`<view>` defaults to vertical orientation** (vs cocoa's Row).
  `gtk::Box` requires an orientation at construction; vertical
  matches the more common stack expectation.
- **`Placeholder` is a hidden `gtk::Box`** — `set_visible(false)`
  removes it from layout entirely on GTK. No `position: Absolute`
  trick needed.
- **`flex_grow` is binary on GTK.** `gtk::Widget::set_hexpand`
  /`set_vexpand` are bools, not weighted floats. Builder API still
  takes `f32` for cocoa parity; the truthiness of the value is what
  carries through.
- **Signal handlers stack**: each `connect_clicked` (and the future
  text/slider/dropdown helpers) appends a new handler. cocoa
  target/action overwrites; GTK doesn't. Nothing in the rest of the
  port relies on the single-handler limitation.
- **No thread-local handler store.** Closures are owned by the
  signal connection itself (held by the widget). When the widget
  drops, the closure drops. No equivalent to cocoa_dom's
  `keep_target_alive` is needed.

## When you change something

When a change touches both ports, list both sets of paths so reviewers can scan it. When it touches only one, name the port explicitly.

- **If you add a new control:**
  - macOS: builder in `tachys/src/cocoa/element.rs` + tag handling in `cocoa_dom/src/node.rs::Element::create_with` + facade re-export in `tachys/src/html/element_macos.rs` + `cocoa_stub_view_impls!` for the new builder.
  - GTK: builder in `tachys/src/gtk/element.rs` *(Stage 5, not yet built)* + tag handling in `gtk_dom/src/node.rs::Element::create` + facade re-export in `tachys/src/html/element_gtk.rs` *(Stage 5)* + `gtk_stub_view_impls!` *(Stage 5)*.
- **If you add a new event:**
  - macOS: `EventDescriptor` impl + `PendingHandler` variant in `tachys/src/html/event_macos.rs` + install hook in `cocoa_dom/src/event.rs` + passthrough method on `cocoa_dom::Element`.
  - GTK: `EventDescriptor` impl + `PendingHandler` variant in `tachys/src/html/event_gtk.rs` *(Stage 5)* + install helper in `gtk_dom/src/event.rs` *(Stage 3 will create this)* + passthrough method on `gtk_dom::Element`.
- **If you change layout behavior:**
  - macOS: re-test resize on at least `counter (examples_cocoa/counter)` (static) and `counters_macos` (dynamic add/remove). Layout regressions are the most common breakage.
  - GTK: re-test the affected examples; GTK self-lays-out so layout regressions are rarer, but resize behavior of `gtk::Box` with mixed `hexpand` children is worth eyeballing.
- **If you touch shared `tachys` / `leptos` code:**
  - Verify default workspace build still passes: `cargo build --workspace --exclude cocoa_dom` (or exclude `gtk_dom` on macOS).
  - Verify the native path on the host OS still passes: `cargo check -p tachys --features native-ui`.
  - Ideally also verify the native path on the *other* OS. Cross-checking is hard from one machine; failing that, read carefully and trust CI.
- **Always log non-obvious decisions** in the right journal:
  - macOS-only decisions → `implementation_log.md`
  - GTK-only decisions → `gtk_implementation_log.md`
  - Cross-cutting decisions → log in *both* journals (or pick one and link from the other). The logs are how future-you (and other instances) understand why something is the way it is.
