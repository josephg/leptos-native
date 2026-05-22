# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this fork is

This fork is a **native-only** rework of Leptos with three UI ports:

- **macOS / AppKit (Cocoa)** — the original, most mature port.
- **Linux / GTK4** — mirrors the Cocoa port's structure one-for-one.
- **iOS / UIKit** — mirrors the Cocoa port closely (same Taffy
  bridge, same target/action handler-store pattern). Differs in
  window/scene model (no user-facing windows; one fullscreen scene),
  safe-area + keyboard avoidance via `viewDidLayoutSubviews` +
  `UIKeyboardLayoutGuide`, and per-control name/feature deltas
  (e.g. `<switch>` instead of `<checkbox>`, no PopUpButton).

**Web / SSR is no longer in this fork.** The upstream
`leptos_router`, `leptos_meta`, and `integrations/*` crates were
removed; `tachys` was stripped down into a renderer-agnostic core
that now lives as the `renderer` **module** inside the
`leptos_native` crate (`common/leptos_native/src/renderer/`), driven
by per-port crates (`cocoa/leptos_cocoa`, `gtk/leptos_gtk`,
`uikit/leptos_uikit`). There is no `native-ui`
feature flag or `cfg(leptos_native)` toggle — every binary picks a
port crate directly via its `Cargo.toml`. Build commands operate on
the host OS's port (cocoa on macOS, gtk on Linux); iOS examples
live in an inner workspace at `uikit/examples/` that defaults to
the `aarch64-apple-ios-sim` target and shares the parent
`target/` directory.

Read these before diving in:
- `implementation_log.md` — chronological design-decision journal for
  the **macOS** port (newest at top). Critical context for anything
  related to layout, eventing, multi-window, or the macro plumbing.
- `gtk_implementation_log.md` — same shape, for the **Linux/GTK** port.
- `implementation_ios.md` + `TODO_ios.md` — iOS port design log
  and priority-ordered outstanding work. The TODO_ios doc
  consolidates what used to live across separate audit / tasks /
  photosite-gaps files. Shorter than the macOS log because the
  port is younger.
- `README_gtk.md` / `README_macos.md` / `README_ios.md` — user-facing
  overviews per port: status, prerequisites, examples.
- `tests_macos.md` / `tests_gtk.md` / `tests_ios.md` — per-port test
  plans (XCUIAutomation harness for cocoa still deferred; the iOS
  plan is the shortest).
- `API_REVIEW.md` — critique + prioritised cleanup recommendations
  for the public API. Lives between the implementation logs (history)
  and the per-port TODOs (forward work).
- `MEMORY_POLICY.md` — **prescriptive** policy for how state is owned
  and released across Rust, ObjC retains, autorelease pools, and
  reactive_graph. Mandatory reading before touching `event.rs`,
  `bind.rs`, `NodeHandlers`, or anything that installs an ObjC
  delegate / target-action. Every memory bug we've fixed has a
  corresponding entry in its anti-pattern catalogue.

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

## Workspace layout

```
common/                          # renderer-agnostic shared crates
  leptos_native/                 # the core crate. IntoView, mount machinery,
    src/                         #   error boundary, control flow, ...
      renderer/                  #   the `renderer` module — Render / Mountable /
                                 #     AddAnyAttr core + shared LayoutAttrs /
                                 #     UniversalAttrs / TextAttrs + Taffy
                                 #     LayoutTree<B> + setters/apply_layout.
                                 #     (Formerly the standalone `tachys` /
                                 #     `common/renderer` crate; inlined as a
                                 #     module so IntoMaybeReactive impls for
                                 #     taffy types satisfy the orphan rule.)
  leptos_macro/                  # view!{} + #[component] proc macros
  devtools/                      # `leptos_devtools` — CDP devtools server

# reactive_graph / reactive_stores are now unmodified crates.io deps
# (reactive_graph 0.3.0-alpha, reactive_stores 0.5.0-alpha), no longer
# vendored under common/.

cocoa/                           # macOS / AppKit port
  leptos_cocoa/                  # element builders + macro facades + mount
    src/dom/                     #   — NSView/NSButton/... façade (was the
                                 #     standalone `cocoa_dom` crate; now the
                                 #     `dom` module of this crate)
    src/cocoa/element.rs         #   — Button, Label, Stack, Grid, ...
    src/element_macos.rs         #   — `tachys::html::element` macro facade
    src/event_macos.rs           #   — `tachys::html::event` macro facade
    src/lib.rs                   #   — `pub mod tachys` re-export shim
  examples/<name>/               # workspace-member binaries

gtk/                             # Linux / GTK4 port — same shape as cocoa/
  leptos_gtk/
    src/dom/                     #   — gtk::Box/Button/... façade (was the
                                 #     standalone `gtk_dom` crate; now the
                                 #     `dom` module of this crate)
  examples/<name>/

uikit/                           # iOS / UIKit port — same shape as cocoa/
  dom/                           # UIView/UIButton/... façade — still the
                                 #   standalone `ios_dom` crate (NOT yet
                                 #   inlined like cocoa/gtk)
  leptos_uikit/
  examples/<name>/               # NOT workspace members — iOS sim only

apple_shared/                    # bits shared between cocoa & uikit
```

The per-port `dom` façades for cocoa and gtk used to be separate
crates (`cocoa_dom`, `gtk_dom`); they've been **inlined as the `dom`
module** of `leptos_cocoa` / `leptos_gtk`. The iOS façade (`ios_dom`)
is still its own crate at `uikit/dom/`.

Each example pulls in **exactly one** of `leptos_cocoa` /
`leptos_gtk` / `leptos_uikit` under the alias `leptos = { package =
"leptos_<port>" }`, so example code uses `leptos_native::prelude::*` and
the macro paths resolve to that port's `tachys` shim. There is no
shared `leptos` crate that switches backends via feature flag.

## Build & run

Each port iterates via "build an example and click around." Unit
tests live with each port crate (`cocoa/leptos_cocoa/tests/`,
`gtk/leptos_gtk/tests/` — including `tests/dom/` for the inlined
façade — and `uikit/dom/tests/`); see `tests_<port>.md`.

### macOS / Cocoa

```sh
cargo run -p counter_cocoa
cargo run -p counters_cocoa
cargo run -p greeter_cocoa
cargo run -p grid_cocoa
# ...etc. See cocoa/examples/.

# Just typecheck the port crate (the dom façade is now a module of it):
cargo check -p leptos_cocoa
```

### Linux / GTK

System prereqs: `libgtk-4-dev` + `pkg-config` (Debian/Ubuntu) or
`brew install gtk4` (macOS, for cross-checking). See `README_gtk.md`
for distro-specific commands.

```sh
cargo run -p counter_gtk
cargo run -p grid_gtk
# ...etc. See gtk/examples/.

cargo check -p leptos_gtk
# Contributor mode (typecheck without linking gtk4):
cargo check -p leptos_gtk --no-default-features
```

GTK examples are workspace members (so `cargo build --workspace`
discovers them) but excluded from `default-members` because
their `leptos_gtk` dependency uses the default features (which
include `gtk`) and therefore links against gtk4 at build time.

### iOS / UIKit

iOS examples live in their own Cargo workspace at
`uikit/examples/`, which sets `aarch64-apple-ios-sim` as the
default target and shares `target/` with the parent workspace.
Each example ships a `run_ios.sh` shim that calls the shared
`uikit/tools/run_ios.sh` — which builds the example, hand-rolls
a `.app` bundle, terminates any prior instance, then
`xcrun simctl install`s + launches it. No Xcode project required.

```sh
# Interactive: launches the app, leaves it running for the user to
# poke at. Without `-t`, the script blocks streaming app stdout via
# `xcrun simctl launch --console`; you have to Cmd-Q the simulator
# app or kill the process to get your terminal back.
cd uikit/examples/counter && ./run_ios.sh

# Non-interactive (USE THIS FROM AGENTS / CI / ANY AUTOMATED FLOW):
# `-t SECONDS` auto-terminates the app after the given timeout.
# Without it the script hangs indefinitely streaming console output
# from a running iOS app — agents in particular WILL stall waiting
# for the script to return. ~3s is plenty to verify the app launched
# and didn't immediately crash.
cd uikit/examples/counter && ./run_ios.sh -t 3

# Or call the shared script directly from anywhere:
uikit/tools/run_ios.sh uikit/examples/counter -t 3

# Just typecheck the iOS-target build (uikit/leptos_uikit and
# uikit/dom are top-level workspace members):
cargo check -p ios_dom --target aarch64-apple-ios-sim
cargo check -p leptos_uikit --target aarch64-apple-ios-sim

# Build the iOS examples (from inside the inner workspace, the
# iOS target and shared target/ are configured by default):
cd uikit/examples && cargo build --workspace
```

Prereqs: Xcode + the iOS Rust targets (`rustup target add
aarch64-apple-ios-sim` on Apple Silicon, also `x86_64-apple-ios` on
Intel, and `aarch64-apple-ios` for real devices). The example scripts
auto-create / boot a simulator if none is running.

### Workspace-wide checks

```sh
cargo build --workspace            # host-OS port + its examples + shared crates
cargo check -p leptos_native       # shared core only (incl. the renderer module)
```

Touching shared code in `common/` (especially the `renderer`
module of `leptos_native`, which now
houses both the view-tree core and the Taffy layout engine) can
break a port the host OS can't compile (e.g. editing on macOS but
breaking GTK). Cross-checking is hard from one
machine; read carefully and trust CI / the other dev.

## Architecture of the macOS port

Three layers, lowest first.

### `cocoa/leptos_cocoa/src/dom/` — DOM-shaped façade over AppKit (the `dom` module)

The lowest layer (formerly the standalone `cocoa_dom` crate, now the
`dom` module of `leptos_cocoa`). Provides `Node` and `Element` types that mirror
the shape of their `web_sys` equivalents but are backed by `NSView`
(and subclasses like `NSButton`, `NSTextField`). The renderer's
"text node" and "placeholder" varieties are just Element
constructors (`Element::create_text`, `Element::create_placeholder`);
there's no distinct wrapper type for them — the per-port renderer
wrapper aliases `type Text = Element` / `type Placeholder = Element`.
Owns:

- **Layout** (`dom/layout.rs`): the storage tree itself lives in
  the `renderer` module (`LayoutTree<CocoaBackend>`); this file
  plugs cocoa-specific types into it via `CocoaBackend`
  (`measure_leaf` reads `intrinsicContentSize`, `first_baseline`
  reads `firstBaselineOffsetFromTop`, plus a scroll-view second-pass
  hook). Each window has its own tree; every `Node` is a thin
  handle (`Rc<NodeInner { tree, id, view, is_borrowed }>`)
  pointing into the arena. Layout recompute is **manual** — AppKit
  doesn't auto-reflow, so `set_attribute` / `set_text` /
  `attach_child` / etc. each call `schedule_relayout`, which dedupes
  via the per-tree `relayout_queued: Cell<bool>` flag and dispatches
  one `compute_layout` pass per main-loop tick. **Always
  `tree.mark_dirty(node_id)` when content changes** (otherwise
  Taffy's measure cache is stale).
- **Events** (`dom/event.rs`): NSButton uses `ActionTarget`
  (target/action). NSTextField uses a single `TextFieldDelegate`
  that fans out to `Vec<Box<dyn FnMut(String)>>` for both
  `controlTextDidChange:` (input) and `controlTextDidEndEditing:`
  (change). Each per-view delegate retain lives in the arena's
  `NodeData::handlers` slot (`NodeHandlers` struct); deterministic
  cleanup when the last `Node` clone drops via
  `tree.decref(id)` → arena removes the entry → `NodeHandlers::Drop`
  nils setTarget/setDelegate and releases the retains.
- **Spawner** (`dom/spawner.rs`): `any_spawner::CustomExecutor`
  backed by `DispatchQueue::main()`. Pin soundness: don't add an
  outer `Pin` — the inner `Pin<Box<dyn Future>>` already has a
  stable address.
- **Window** (`dom/window.rs`): `open_window` returns an
  `OpenedWindow` with the NSWindow, content_root `Element` (a
  `FlippedView` for top-left coords), the new `TreeRef`, and the
  resize delegate.
- **App** (`dom/app.rs`): `init_app(mtm)` builds NSApp + menu bar +
  AppDelegate (returns true from
  `applicationShouldTerminateAfterLastWindowClosed:`).

Everything in this module panics off the main thread; `SendWrapper`
enforces it at runtime.

### `cocoa/leptos_cocoa/src/cocoa/` — bridges the `dom` module to `renderer`'s `Render`/`Mountable` traits

- `element.rs`: `Button`, `Checkbox`, `Label`, `TextField`,
  `Stack<Ch>` (used by `vstack` / `hstack` / `view`), `Grid<Ch>`,
  etc. builder structs with `.title()` / `.value()` / `.child()` /
  `.on(event, handler)` / `.bind(key, signal)` methods, plus
  `Render::build` impls. **Children are deliberately NOT mounted at
  build time** — mounting is deferred until `ElementState::mount`
  runs. This is the cascade pattern that lets tree-aware
  `insert_node` register each child in the right Taffy tree as it
  goes.
- `attr.rs`: port-local `MaybeReactive<T>` (Static or Reactive
  closure) + `IntoMaybeReactive<T>` + `install` helper that wraps a
  closure in a `RenderEffect`. (Distinct from
  `renderer::attrs::MaybeReactive` in the shared `renderer` module,
  which the shared `WithLayout` / `WithUniversal` traits use.)
- `bind.rs`: `IntoSignal<T>` trait + per-control `BindAttribute`
  impls. `bind:value` on text_field and `bind:checked` on checkbox
  both wire **two directions** — outgoing via the AppKit observer,
  incoming via a `RenderEffect`.
- AddAnyAttr machinery for spread attrs (`<MyComponent
  on:click=…>`): the `impl_add_any_attr_for_leaf!` macro at the
  bottom of `element.rs` emits typed-attribute pipeline impls per
  leaf builder; container builders (`Stack`, `Grid`, `Block`,
  `ScrollView`) get explicit panic-on-spread impls. There is no
  `RenderHtml` trait — `IntoView<R>` requires only `Render<R> +
  AddAnyAttr<R> + Send`.

### `cocoa/leptos_cocoa/src/{element,event,svg}_macos.rs` — macro facades

The `view!{}` macro emits paths like
`::leptos_native::tachys::html::element::button()`,
`::leptos_native::tachys::html::event::on(event::click, handler)`, etc.
`leptos_cocoa` has a `pub mod tachys { ... }` in `lib.rs` that
re-exposes those paths, backed by the cocoa builders. **Don't
change the macro** — just expand the facades when adding new tags
or events.

All tag names — including SVG-namespaced ones like `<switch>`
— route through `tachys::html::element::*` on native.
The web Leptos macro routed those through `tachys::svg::*` and
emitted `.attr(name, value)` for every attribute; we stripped that
path because no native builder has an `.attr()` shim. See the
"SVG removal" entry in `implementation_log.md` for the rationale.

### `cocoa/leptos_cocoa/src/mount.rs` — entry points

`run(closure)` and `mount_to_window(title, size, closure)`.
Currently leaks the `Owner` (no real `UnmountHandle` story for
window close).

## Architecture of the Linux/GTK port

Mirrors the macOS layering one-for-one. **GTK uses Taffy too**, via
the shared `renderer`-module storage tree; the Stage-1 plan to
let GTK self-lay-out was reversed because the SwiftUI-flavoured
mental model (`vstack`/`hstack`/`flex_grow`/percent widths) doesn't
map cleanly onto GTK's measure/allocate negotiation. GTK widgets
get a per-container `TaffyLayout` manager that delegates back to
Taffy; the widget class (usually `gtk::Box`) is layout-agnostic at
that point.

### `gtk/leptos_gtk/src/dom/` — DOM-shaped façade over GTK4 (the `dom` module)

Same shape as cocoa's `dom` module (formerly the standalone
`gtk_dom` crate, now the `dom` module of `leptos_gtk`). Modules:

- `node.rs` + `make_view.rs` — typed per-control constructors
  (`Node::create_button`, `create_label`, `create_vstack`, ...). Each
  builder calls one constructor; no tag-string dispatch.
- `layout.rs` — same `LayoutBackend` plug-in pattern as cocoa.
  Plus `taffy_layout.rs` which exposes Taffy as a custom
  `gtk::LayoutManager`, installed per container at mount time.
- `event.rs` — `on_click` calls `gtk::Button::connect_clicked`;
  closures are owned by the signal connection (held by the widget),
  so no thread-local handler store like cocoa's `keep_target_alive`
  is needed.
- `spawner.rs` — `any_spawner::CustomExecutor` over
  `glib::MainContext::default().spawn_local`.
- `window.rs` — `open_window(app, title, size)`.
- `app.rs` — `init_app(application_id)` builds a
  `gtk::Application` and registers the spawner.

`gtk::Widget` is `!Send`; `SendWrapper` wraps each `Node` so the
type is nominally `Send + 'static` for the renderer's generic
plumbing, with a runtime panic if accessed off-main.

### `gtk/leptos_gtk/src/gtk/` — element builders

Same shape as `cocoa/leptos_cocoa/src/cocoa/element.rs`. The set of
builders (`button`, `checkbox`, `label`, `pop_up_button`,
`secure_text_field`, `slider`, `stack`, `stack_view`, `text_field`,
`vstack`, `hstack`, `grid`, `view`) is a subset of cocoa's — GTK
deltas (no NSDatePicker, no NSColorWell, no NSSegmentedControl) are
just absent.

### `gtk/leptos_gtk/src/{element,event,svg}_gtk.rs` — macro facades

Same role as `element_macos.rs` etc. on cocoa.

### `gtk/leptos_gtk/src/mount.rs` — entry points

`run(closure)` and `mount_to_window(app_id, title, size, closure)`.

## Architecture of the iOS port

Mirrors the macOS layering one-for-one. The Taffy bridge is
identical (UIView's intrinsic-size measurement closure +
per-scene `LayoutTree<IosBackend>`), and the target/action
handler-store pattern from cocoa carries over directly (UIControl
is structurally NSControl). The big shape changes are at the
window/scene boundary: there's no NSWindow / NSApplicationDelegate
run-loop you can drive yourself — UIApplicationMain owns the loop
— and there's no menu bar.

### `uikit/dom/` — DOM-shaped façade over UIKit (crate `ios_dom`)

- **Layout** (`src/layout.rs`): same `LayoutBackend` plug-in pattern,
  same manual-relayout discipline as cocoa. Always
  `tree.mark_dirty(node_id)` when content changes.
- **Events** (`src/event.rs`): `ActionTarget` ObjC class wraps a
  Rust closure; `on_control_action` chooses the right
  `UIControlEvents` mask based on the concrete control
  (TouchUpInside for UIButton, ValueChanged for UISwitch/UISlider/
  UISegmentedControl/UIDatePicker/UIStepper, EditingChanged for
  UITextField input, etc.). Handler retains live in a thread-local
  `HANDLER_STORE` keyed by view pointer (entries currently leak;
  same as cocoa). UITextView uses a `UITextViewDelegate` because
  UITextView isn't a UIControl.
- **Spawner** (`src/spawner.rs`): `any_spawner::CustomExecutor`
  over `dispatch2::DispatchQueue::main()`. Identical to cocoa.
- **App + RootViewController** (`src/app.rs`): `AppDelegate`
  creates the UIWindow on
  `application:didFinishLaunchingWithOptions:`,
  `RootViewController` overrides `viewDidLayoutSubviews` to re-run
  Taffy on every bounds change *and* push `view.safeAreaInsets` +
  `view.keyboardLayoutGuide().layoutFrame()` derived bottom-inset
  onto the content root's padding. Both `AppDelegate` and
  `RootViewController` define an ObjC `-init` so UIKit's own
  `[Class alloc] init]` lands on initialised ivars (a non-obvious
  objc2 gotcha — without it the first `self.ivars()` panics).
- **`uiapplication_main`**: passes the *runtime-mangled* class name
  via `AppDelegate::class().name()`, not the literal `"AppDelegate"`
  string. The mangled name is what objc2's `define_class!` actually
  registers.

### `uikit/leptos_uikit/src/ios/` — bridges `ios_dom` to renderer's `Render`/`Mountable` traits

Same shape as `cocoa/leptos_cocoa/src/cocoa/`. `element.rs` defines
the builders; `bind.rs` defines `IntoSignal` / `BindAttribute<Key,
Sig>` impls (with cocoa-port-style `BoundValue`/`BoundFloat`/
`BoundChecked`/`BoundDate`/`BoundIndex` payloads). The same
`apply_universal` / `apply_text_attrs` helpers and
`impl_universal_attrs!` / `impl_text_attrs!` /
`impl_typed_attrs_for!` macros that DRY out cocoa's element.rs are
ported here.

Builders implemented: `Button`, `Label`, `TextField` /
`SecureTextField`, `Switch`, `Slider`, `Stepper`, `SegmentedControl`,
`DatePicker`, `ProgressIndicator` (UIProgressView under the hood,
named for cocoa parity), `ImageView`, `ScrollView`, `TextView`,
`View` / `vstack` / `hstack`, `Grid`. `bind:value`, `bind:checked`,
`bind:selection` all wired.

Not implemented (no native UIKit equivalent):
- **PopUpButton** — UIMenu / UIPickerView, both quite different from
  NSPopUpButton.
- **ColorWell** — UIColorPickerViewController is a modal sheet, not
  inline.

### `uikit/leptos_uikit/src/{element,event}_ios.rs` — macro facades

Same role as the cocoa equivalents. `<switch>` is a Rust keyword
collision (and an SVG element name in the web spec), so the macro
emits a raw identifier — `element_ios.rs` defines `r#switch` as a
function delegating to `tachys::ios::element::switch_()`, since the
builder itself can't take the bare name either.

### `uikit/leptos_uikit/src/mount.rs` — entry point

Single entry point: `run(closure)`. Stores the user closure in a
thread-local, calls `UIApplicationMain` (which never returns).
`AppDelegate::application:didFinishLaunchingWithOptions:` invokes
the stored closure inside a fresh `Owner` scope, builds the view,
mounts it under the content root.

There's no `mount_to_window` builder. iPhone apps run as a single
fullscreen scene; iPad multi-window is scene-based, not
window-builder-based.

## Shared layout & attribute plumbing (`leptos_native::renderer`)

The shared core lives in the `renderer` **module** of `leptos_native`
(`common/leptos_native/src/renderer/`). Port crates reach it via
`use leptos_native::renderer` (or the `renderer::*` re-exports their
preludes set up).

- **`renderer` module** — `LayoutTree<B>` is a generic Taffy
  storage tree owning per-node
  style/cache/layout/parent/children/handlers/view/meta plus a
  refcount. Each port implements `LayoutBackend` to supply its
  platform view type plus three operations (measure a leaf, query
  baseline, apply a frame). Allocation comes via `tree.new_leaf`
  (refcount=1 — the caller's `Node` handle) or `tree.new_internal_leaf`
  (refcount=0 — for entries no `Node` owns, kept alive solely by
  their parent edge; e.g. cocoa's scroll-view documentView wrapper).
  Removal is via `tree.remove(id)` (eager) or implicit through
  `tree.decref(id)` when refcount→0 AND parent==None.
  `tree.remove` transitively GCs any orphaned children whose
  refcount=0, so internal entries clean up automatically when
  their parent goes away. Layout *driving* (when to call
  `compute_layout`, how to dispatch to the main thread) is left to
  each port. Re-exports Taffy's `Display`, `FlexDirection`,
  `AlignItems`, `GridAutoFlow`, etc., plus the track-sizing helpers
  (`fr`, `length`, `auto`, `min_content`, `max_content`, `minmax`,
  `fit_content`, `repeat`). Grid types are pre-monomorphized:
  `pub type GridTemplateComponent = taffy::GridTemplateComponent<String>`.

- **`renderer/attrs.rs`** — `LayoutAttrs` (`padding`,
  `margin`, `width`/`height`/`min_*`/`max_*`, `flex_grow`,
  `align_self`, `grid_column_start`/`end`, `grid_row_start`/`end`),
  `UniversalAttrs` (`alpha`, `tool_tip`), `TextAttrs<C, A>` (text
  color, alignment, font size). The corresponding traits
  (`WithLayout`, `WithUniversal`, `WithText<C, A>`) provide the
  chainable setters. Each builder embeds the struct as a field and
  implements the trait by handing back `&mut self.foo`; the
  default methods provide `.padding(...)` / `.flex_grow(...)` /
  `.grid_column((1, -1))` / etc. consistently across every builder.

- **`renderer` module** — core `Render` / `Mountable` /
  `AddAnyAttr` / `ApplyAttr` traits, plus the `view::*` helpers
  (the renderer's "view core" — fragment, iterators, conditional,
  etc.). All web-only machinery (`Attribute`, `RenderHtml`,
  `to_html`, hydration) is gone.

## Conventions and gotchas

### Failure-mode hierarchy

When a feature is unimplemented, partially supported, or genuinely
broken, prefer failure modes in this order:

1. **Compile error** — make the construct ill-typed, so the broken
   code doesn't build.
2. **Panic at mount / launch time** — for definitively-broken state
   that's detectable as soon as the view tree is built. Fail before
   the run loop starts. Include a clear message with the *kind* of
   view that hit the limitation, why it doesn't work, and a
   workaround.
3. **Warning** (`eprintln!`, `tracing::warn!`, `#[deprecated]`) +
   **graceful degrade** — for subtle, runtime-dependent cases where
   the UI will still mostly work even if the misconfiguration isn't
   addressed (e.g. a scroll view whose parent doesn't supply a
   bounded main-axis size — the user just won't be able to scroll).
4. **Silent no-op** — only as an absolute last resort, and only
   when the silence itself is the contract (e.g. event delegate
   receives a notification it doesn't care about).

**When to panic vs warn.** Panic when the broken state is:
- Detectable at mount/launch time (predictable, deterministic).
- A bug the developer will hit during their first run of the app.
- Without panicking, the symptom would be silent and confusing
  (the dropped `on:click` handler scenario).

Warn-and-degrade when the broken state is:
- Subtle / context-dependent (a layout that *might* work in some
  window sizes and not others).
- Runtime-triggered by user-action paths the developer may not
  have tested — panicking here would crash user apps in
  production for non-showstopper issues.
- Recoverable (the UI does *something* reasonable, even if not
  the intended thing).

The temptation to "make it compile by stubbing" is the bug-hiding
pattern this hierarchy exists to prevent. A silently-dropped
`on:click` handler is a UI bug that surfaces as "the button does
nothing" hours later — far worse than an immediate panic at the
call site.

But the inverse is also a trap: panicking on every
runtime-discovered inconsistency turns subtle layout issues into
hard crashes in user apps. Reserve panics for "the developer
made this broken; they'll see the panic the first time they run
the app." For "this might not behave perfectly under all
inputs," prefer warn + degrade.

Concrete examples:

- **Compile error**: `<button on:foo=...>` for an unknown event.
  The macro-generated `SupportsEvent<FooEvent>` bound isn't
  satisfied; the user sees a type error.
- **Mount-time panic**: two `on:click` handlers on the same
  NSControl (NSControl has one target/action slot; doubling up
  silently would lose a handler). The Cocoa Button builder
  asserts at install time. Detectable at view-build, fixable by
  combining the closures.
- **Warn + degrade**: a `<scroll_view>` whose parent doesn't
  bound its main-axis size. The scroll view still renders; it
  just sizes to its content (no scrolling). The framework
  `eprintln!`s an explanation the first time it happens for a
  given element.
- **Compile error via `AddAnyAttr<R>`**: branching wrappers
  (`Option<T>`, `Either`, `Vec<T>`, `ErrorBoundary`) `panic!()`
  with diagnostics if spread-attrs are installed on them. See
  `common/leptos_native/src/renderer/view/add_any_attr.rs`. This is a panic
  because the broken state is determined at view-tree
  construction time, not at runtime.

### Shared (all ports)

- **Tag names are snake_case** by deliberate choice, even when they
  correspond to PascalCase widget types (so the macro's
  auto-routing works). Live with the convention clash.
- **`set_attribute` diffs against current widget state** before
  mutating, on every backend. This protects against `bind:` cycles
  (an `Effect`-driven write firing the widget's change signal that
  re-fires the effect) and against focus-ring flashes. Keep it.
- **HTML compatibility is a non-goal.** UIs are built specifically
  as native apps. We are free to invent tags (`<vstack>`,
  `<hstack>`, `<checkbox>`, `<grid>`) without worrying about HTML
  semantics.
- **Reactive attrs go through `MaybeReactive<T>` + `install(...)`.**
  `MaybeReactive` (and `install`, the driver that wraps a setter in
  a `RenderEffect`) are defined in `renderer::attrs` and re-exported
  per port. Each port additionally defines its own `IntoMaybeReactive`
  trait (port-local) so it can supply impls for platform value types
  (`Color`, `NSTextAlignment`, etc.) without orphan-rule violations.
  Builder methods explicitly bound on whichever trait holds the impl
  they need.
- **Generic `apply_layout` / `apply_universal`** live in
  `renderer::setters`, driven by the `LayoutElement` /
  `UniversalElement` traits. Each port impls those for its element
  type in the dom crate and gets the install-loop for every shared
  attr (padding, margin, sizing, flex_grow, align_self, grid
  placement, alpha, tooltip) for free. New layout attrs land in one
  place — the shared `LayoutAttrs` struct + the relevant `set_*` in
  `renderer::setters`.

### Avoid `thread_local!` for new state — use the alternatives

Thread-locals are tempting on cocoa/iOS ("everything's on the main
thread anyway") but they make reasoning about state hard, complicate
testing, and tear down in unspecified order at process exit (which
caused a real shutdown-panic bug when one TLS slot's `Drop` accessed
another that had already been torn down — see
`briefing_scroll_overflow.md` history if you want the story).

Default to the following alternatives, roughly in order of
preference for the use case:

- **Per-NSObject sideband state for a Node-backed view → field on
  `NodeHandlers`** (cocoa: `leptos_cocoa::dom::event::NodeHandlers`,
  iOS: `ios_dom::event::IosNodeHandlers`). `NodeHandlers` lives as a
  `RefCell<B::Handlers>` field on the per-window arena's
  `NodeData<B>` entry. Install functions go through
  `node.with_handlers_mut(|h| ...)` which calls
  `tree.with_handlers_mut(id, f)` against the arena. The Node
  itself is a thin `Rc<NodeInner { tree, id, kind, view, is_borrowed }>`
  handle; the arena owns the actual per-node state.

  Lifecycle = arena entry lifecycle: when the last Node clone of
  an owning Node drops, `NodeInner::Drop` calls `tree.decref(id)`.
  Under the removal rule (`refcount == 0 AND parent == None`),
  the arena removes the entry. `NodeData` field-drop order
  (`handlers` before `view`) fires `NodeHandlers::Drop`, which
  nils target/delegate on the still-live view, then releases the
  handler retains.

  **Don't use ObjC associated objects** for this. We tried — they
  tie handler lifetime to the NSView/UIView's ObjC reference
  count, and AppKit/UIKit retains views in places outside our
  control (autorelease pools, undo manager, focus chain, gesture
  recognizer lists). Those retains caused a slow but persistent
  handler leak. Rust-owned storage decouples from that and gives
  deterministic disconnect on drop.

  **Avoid capturing `Element` / `Node` clones in callback closures
  stored on the same Node's handlers** — that forms a cycle
  (closure → captured Element → `Rc<NodeInner>` → entry stays
  alive forever → handlers stay alive → closure stays alive).
  The cycle is structurally possible because Node::clone bumps
  the Rc. Use one of:

   * **Typed `Retained<NSView>` / `Retained<NSControl>`** when
     the closure only needs to call ObjC methods on the view.
     `Retained<NSButton>` doesn't pull the Rust Node into the
     cycle. See `Node::ns_view_retained()` / `Element::ns_view_retained()`.
   * **`WeakElement` / `WeakNode`** when the closure needs to
     re-enter the element's Rust API (style, meta, handlers).
     `el.weak()` returns a `WeakElement` (a `Weak<NodeInner>`);
     `weak.upgrade()` recovers the Element at fire time, returning
     `None` if the original has dropped. No cycle because Weak
     refs don't keep the Rc alive.

  **Text-field/text-view delegate drop ordering**: the cocoa and
  iOS `NodeHandlers::Drop` impls explicitly release the
  text-field / text-view delegate `Retained`s BEFORE calling
  `disconnect_view_handlers` (which sends `setDelegate(None)`).
  AppKit/UIKit's text-system pins an extra retain on the delegate
  the moment that setter clears the property; releasing our
  Retained afterwards leaves the delegate stuck at retainCount=1.
  Drop order is load-bearing — see the comments in
  `cocoa/leptos_cocoa/src/dom/event.rs::NodeHandlers::drop`. Action targets /
  hover trackers don't have this issue; only the text delegates
  do.

- **Per-NSObject state for non-Node wrappers** (NSMenuItem,
  NSToolbarItem): give the Rust wrapper struct a field for the
  Retained handler. `MenuItem::action_target`,
  `ToolbarItemRegistration::action_target` are the existing
  examples. Add a `Drop` impl that nils out `setTarget` /
  `setAction` first.

- **Per-tree state → field on `LayoutTree<B>`.** Each window/scene
  has its own `LayoutTree`; data that's logically "about this tree"
  (scheduler flags, dirty sets, debug overlays) goes on the
  struct, not in a global keyed by tree pointer. Example: the
  relayout-dedup `Cell<bool>` on `LayoutTree::relayout_queued`.

- **Per-Node state → field on the port's `NodeMeta`.** Anything
  needed during layout (sizing flags, scroll-axis selection,
  intrinsic-width opt-in) goes on `CocoaMeta` / `IosMeta` and is
  copied into the tree on registration. The layout pass receives
  it alongside the view in `measure_leaf`. Example:
  `CocoaMeta::intrinsic_width_from_content`.

- **Process-wide counters / IDs → `static AtomicU64`.** No reason
  to make them per-thread; the IDs are globally unique anyway.
  Example: `NEXT_AUTO_IDENTIFIER` for auto-generated toolbar
  identifiers.

- **App-scoped pinning (Owners + root State) → return an
  `AppHandle` from the mount entry point.** The user's `main`
  binds the handle and calls `.run()` to enter the AppKit run
  loop. When the run loop returns, the handle's `Drop` fires in
  field-declared order (root `State` → reactive `Owner` →
  `Retained<NSApplication>` → `Retained<AppDelegate>`), so
  reactive cleanup happens before the process exits. See
  `cocoa/leptos_cocoa/src/mount.rs::AppHandle`. **Do not** put
  this state in `thread_local!`, and **do not** use
  `std::mem::forget` to skip teardown — both are
  short-cuts that hide ownership and silently bypass cleanup.

The remaining `thread_local!`s in framework code are
`cocoa/leptos_cocoa/src/dom/debug_overlay.rs` (debug-overlay state, behind
feature flag) and `uikit/dom/src/app.rs::BUILDER` (single-shot
view-builder slot consumed by `scene:willConnectToSession:`) —
both fall under `MEMORY_POLICY.md` §2's app-scoped carve-out
because each is a single value / fixed-size collection that
lives until process exit. See `MEMORY_POLICY.md` for the full
rules.

TLS is also acceptable for vendored reactive_graph internals
(Owner, current effect, current subscriber) — those are the
right shape for "current reactive scope on this thread" and
shouldn't be refactored.

### macOS / Cocoa specifics

- **`<text_field>` forces width=0 in its measure callback** so the
  parent decides the width. Otherwise the field grows with each
  keystroke (its intrinsic width tracks content). Don't "fix" this
  without understanding the resize cascade.
- **`Placeholder` defaults to `position: Absolute`** so it doesn't
  take a flex slot — `Render for ()` builds a Placeholder, and many
  renderer constructs leave them lying around.
- **NSButton needs `buttonWithTitle:target:action:`**, not
  `initWithFrame:` — the latter gives a default bezel with bad
  intrinsic sizing (titles get clipped: "Reset" → "Rese").
- **Layout recompute is manual**: AppKit doesn't auto-reflow, so
  `set_attribute` / `set_text` / `attach_child` / etc. each call
  `schedule_relayout`, which dedupes via the per-tree
  `relayout_queued: Cell<bool>` flag and dispatches one
  `compute_layout` pass per main-loop tick. **Always
  `tree.mark_dirty(node_id)` when content changes** (otherwise
  Taffy's measure cache is stale).
- **Two click handlers on one NSControl panic at build time.**
  NSControl has a single target/action slot. We deliberately don't
  fan out (Vec-of-closures + a wrapper class would add allocations
  for the 99% case where there's one handler). Instead
  `on_control_action` checks the control's existing target and
  panics on a duplicate install. This catches `<button on:click=A
  {..on(click, B)}/>`, `<MyComponent on:click=outer>` where the
  inner component already has its own on:click, and `bind:checked +
  on:click` combinations. Workaround: combine into one closure, or
  have your component accept a `Callback<()>` prop and call it
  inside its own click handler.
- **`<scroll_view>` needs a bounded parent.** Wrap your top-level
  vstack in `flex_grow=1.0` (or give it a fixed height) —
  otherwise the outer container sizes to content and the scroll
  view never gets a viewport to clip against, so scroll bars never
  appear. The scroll view's children take their natural sizes via
  a separate Taffy pass; see `leptos_cocoa::dom::layout::relayout_scroll_views`.
- **Use `<stack>{closure_returning_Result}</stack>`, not `<label>`.**
  `Label::child` only accepts `IntoMaybeReactive<String>` (a leaf).
  To render a `Result<T, E>` (which `Render` impls handle by
  throwing into the nearest `<ErrorBoundary>`), use `<stack>` whose
  `.child<NewCh: Render>` accepts arbitrary children. (Whether
  Label should accept generic children at all is an open P3
  question — see API_REVIEW.md.)

### Linux / GTK specifics

- **GTK uses Taffy via the `renderer` module** — the layout-driver
  shape is the same as cocoa/iOS, just plugged into GTK's
  `gtk::LayoutManager` protocol via `leptos_gtk::dom::taffy_layout`. Don't
  try to fall back to `gtk::Box`'s native orientation/spacing for
  new constructs; route through Taffy like everything else.
- **`<view>` defaults to vertical orientation** (vs cocoa's
  no-direction-preset). Existing convention from before the Taffy
  bridge landed; kept for parity with example code.
- **`Placeholder` is a hidden `gtk::Box`** — `set_visible(false)`
  removes it from layout entirely on GTK. No `position: Absolute`
  trick needed.
- **Signal handlers stack**: each `connect_clicked` (and the future
  text/slider/dropdown helpers) appends a new handler. cocoa
  target/action overwrites; GTK doesn't. Nothing in the rest of the
  port relies on the single-handler limitation.
- **No thread-local handler store.** Closures are owned by the
  signal connection itself (held by the widget). When the widget
  drops, the closure drops. No equivalent to the cocoa `dom`
  module's `keep_target_alive` is needed.

## When you change something

When a change touches multiple ports, list each port's paths so
reviewers can scan it. When it touches only one, name the port
explicitly.

- **If you add a new control:**
  - cocoa: builder in `cocoa/leptos_cocoa/src/cocoa/element.rs` +
    typed constructor `Element::create_<tag>` in
    `cocoa/leptos_cocoa/src/dom/make_view.rs` (alloc the concrete NSView, build
    default Style, call `Element::from_view`) + facade re-export
    in `cocoa/leptos_cocoa/src/element_macos.rs` +
    `impl_add_any_attr_for_leaf!` line for the new builder.
  - gtk: builder in `gtk/leptos_gtk/src/gtk/element.rs` + typed
    constructor `Element::create_<tag>` in `gtk/leptos_gtk/src/dom/make_view.rs`
    (alloc the gtk widget, build default Style, call
    `Node::from_view`) + facade re-export in
    `gtk/leptos_gtk/src/element_gtk.rs` +
    `impl_add_any_attr_for_leaf!` (or container panic-on-spread).
  - ios: builder in `uikit/leptos_uikit/src/ios/element.rs` + typed
    constructor `Element::create_<tag>` in `uikit/dom/src/make_view.rs`
    (alloc the UIView subclass, build default Style, call
    `Node::from_view`) + facade re-export in
    `uikit/leptos_uikit/src/element_ios.rs` +
    `impl_add_any_attr_for_leaf!`.
- **If you add a new event:**
  - cocoa: `EventDescriptor` impl + `PendingHandler` variant in
    `cocoa/leptos_cocoa/src/event_macos.rs` + install hook in
    `cocoa/leptos_cocoa/src/dom/event.rs` + passthrough method on `Element`.
  - gtk: same in `gtk/leptos_gtk/src/event_gtk.rs` +
    `gtk/leptos_gtk/src/dom/event.rs`.
  - ios: same in `uikit/leptos_uikit/src/event_ios.rs` +
    `uikit/dom/src/event.rs`.
- **If you add a new layout attribute that's port-agnostic:**
  add the field on `LayoutAttrs` (or `UniversalAttrs`) in
  `common/leptos_native/src/renderer/attrs.rs`, add the chainable
  setter on the matching trait, then add the per-port `set_*` helper
  in each port's `dom` layout module
  (`cocoa/leptos_cocoa/src/dom/layout.rs`,
  `gtk/leptos_gtk/src/dom/layout.rs`, `uikit/dom/src/layout.rs`) and
  install it in each port's
  `apply_layout` in the corresponding `element.rs`. The shared
  trait approach saves N per-builder edits.
- **If you change layout behavior:**
  - cocoa: re-test resize on at least `counter_cocoa` (static) and
    `counters_cocoa` (dynamic add/remove). Layout regressions are
    the most common breakage.
  - gtk: re-test the affected examples. GTK now uses Taffy too, so
    the failure modes match cocoa's.
  - ios: launch the iOS sim example with `run_ios.sh -t 3` and
    eyeball the layout.
- **If you touch shared code in `common/`:**
  - Verify the host-OS port build still passes: `cargo build
    --workspace`.
  - Verify the *other* native port via `cargo check` —
    `cargo check -p leptos_uikit --target aarch64-apple-ios-sim`
    on macOS hosts, `cargo check -p leptos_cocoa` on linux hosts
    (with the macOS SDK available — usually not, so trust CI).
- **Always log non-obvious decisions** in the right journal:
  - macOS-only decisions → `implementation_log.md`
  - GTK-only decisions → `gtk_implementation_log.md`
  - iOS-only decisions → `implementation_ios.md`
  - Cross-cutting decisions → log in `implementation_log.md` and
    cross-link from the GTK / iOS logs. The logs are how future-you
    (and other instances) understand why something is the way it
    is.
