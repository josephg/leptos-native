# Implementation log — Leptos Linux/GTK port

A running record of design decisions made during the Linux/GTK port,
especially the ones we deliberately deferred. Newest entries at the
top.

---

## 2026-05-19 — Direct typed attribute setters (port mirror)

Mirrored the cocoa attribute-setter cleanup. Removed `StringAttr`
and `BoolAttr` enums + their dispatch (`set_string_attribute` /
`set_bool_attribute` / `remove_string_attribute` /
`remove_bool_attribute`) and the string-keyed `set_attribute` /
`remove_attribute` (both on `Node` and the renderer surface).
Replaced with direct typed methods on `Node`:

- `set_title(&str)` — gtk::Button.set_label / CheckButton.set_label /
  Label.set_label.
- `set_value(&str)` — Entry/PasswordEntry.set_text / Label.set_label.
- `set_placeholder(&str)` — Entry/PasswordEntry.set_placeholder_text.
- `set_hidden(bool)` — Widget.set_visible(!value).
- `set_enabled(bool)` — Widget.set_sensitive.
- `set_checked(bool)` — CheckButton.set_active.

Builders in `leptos_gtk` and the bind layer migrated to direct
calls.

---

## 2026-05-19 — Typed element constructors + Element/Node unification (port mirror)

Mirrored two cocoa refactors landed earlier the same day:

1. Removed the `match tag { ... }` body in `Element::create` — each
   builder now calls a uniquely-named typed constructor
   (`Node::create_button`, `create_label`, `create_vstack`, ...)
   from the new `gtk/dom/src/make_view.rs`. `Node::from_view` is the
   shared registration primitive. See cocoa's
   `implementation_log.md` entry for the full rationale.

2. Unified `Element` and `Node` into a single type. `pub type
   Element = Node;` aliased for backwards-compat; `as_node` /
   `into_node` / `from_node_unchecked` kept as identity methods so
   existing call sites work unchanged. `WeakElement` is now a type
   alias for `WeakNode`. `Mountable<Dom>` / `CastFrom<Node>` /
   `LayoutElement` / `UniversalElement` impls collapsed to single
   `Node` impls.

GTK-specific note: the `container_widget()` helper moved from
`node.rs` to `make_view.rs` (pub(crate)); `node.rs` imports it for
the legacy `Node::create_container` entry point.

`Renderer::create_element(tag, namespace)` was unused inside the
workspace and got deleted in step 1.

---

## 2026-05-19 — Node refactor part 2 (port mirror)

Mirrored the cocoa changes; see `implementation_log.md` for the
full rationale. GTK specifics:

- `GtkBackend::Handlers = ()` (unchanged) — GTK signal handlers are
  owned by the widget's signal connection. So `NodeData::handlers`
  on GTK is just a unit slot; no cycle risk from delegate-style
  retains.
- `WeakElement` / `WeakNode` / `WeakText` / `WeakPlaceholder` added
  for API parity with the other ports, even though the cycle risk
  is moot here.
- Node = `Rc<NodeInner { tree, id, kind, widget, is_borrowed }>`
  with no state enum.
- `Element::create(tree, "foo")` eagerly allocates into the arena.
- `register_in_tree` reduced to a near-no-op that publishes root id.

16/16 lifecycle tests pass. counter_gtk builds.

---

## 2026-05-18 — Node ownership refactor (port mirror)

Mirrored the cocoa Node refactor; see `implementation_log.md` for
the full rationale. GTK specifics:

- `GtkBackend::Handlers = ()` — GTK signal handlers are owned by
  the widget's signal connection, not by our Node. So the
  port-side `NodeHandlers` is just unit; the arena's
  `RefCell<()>` field is a zero-cost slot.
- `NodeState::Unmounted` only carries `style` (meta is `()` too)
  — effectively a `(Style)` payload.
- No `NodeHandlers::Drop` workaround needed (no ObjC quirks).

Same Node accessor surface as cocoa: `with_style`, `with_style_mut`,
`tree_id`, `mounted_handle`, `mount_into_tree`,
`unmount_from_tree`. `layout_slot()` is gone.

---

## 2026-05-15 — Async runtime integration (port mirror)

GTK side of the tokio-integration work (see top entry in
`implementation_log.md`). Two examples ported:
`gtk/examples/ipify` and `gtk/examples/async_patterns`.

Added `gtk_dom::on_main` wrapping `glib::idle_add_once`. Same
signature and call shape as `leptos_apple_shared::on_main` (which
wraps libdispatch's `DispatchQueue::main().exec_async`), so user
code is portable between the cocoa/iOS and GTK ports — just swap
the import.

Why `idle_add_once` rather than `MainContext::invoke`: invoke
runs the closure *inline* when the calling thread owns the
context (which it does during a GTK signal callback), and defers
only when called from another thread. `idle_add_once` always
defers — matching libdispatch's `exec_async` semantics across the
two Apple ports, which makes the cross-port contract uniform.

For the same lifecycle reason that motivated SIGNAL_MT.md on the
Apple side, GTK examples' pattern-4 demo uses the `thread_local!`
workaround to anchor a `!Send` `RwSignal` on the main thread for
repeated push-from-tokio updates. The design discussion in
SIGNAL_MT.md applies to GTK identically; when we ship a cleaner
abstraction it'll land in `common/leptos` with a port-registered
dispatcher (cocoa wires libdispatch, GTK wires `idle_add_once`).

No GTK-specific framework changes were needed beyond `on_main`
itself — `reqwest` + tokio + `glib::MainContext` coexist because
the framework's spawner is `MainContext::spawn_local`, which
already runs on the GTK thread and can poll `tokio::JoinHandle`
freely (JoinHandle doesn't require a tokio context to poll).

GTK examples added to `members` (so `cargo build --workspace`
covers them on Linux), kept out of `default-members` (network +
TLS deps shouldn't run on a vanilla smoke build). Cross-checked
from macOS host via `cargo check` only — no native runs verified
since gtk4 isn't installed here.

---

## 2026-05-14 — Native menus (`<menu_bar>` / `<menu>` / `<menu_item>`)

Cross-cutting feature; the cross-port design rationale lives in
`implementation_log.md`'s entry of the same date. This entry covers
the GTK-specific deltas.

**`gio::Menu` is declarative, `NSMenu` is imperative.** AppKit's
menu objects mutate in place — set the title, the check-mark state,
the action, then add to the parent menu. `gio::MenuItem` is
immutable once attached to a `gio::Menu`. So reactive title
updates (and the future reactive check / submenu shape) work by
*replacing* the item at its index: `remove(i); insert_item(i, new)`.
The Render layer carries the `(parent, index)` pair on each
`MenuItemState` so the reactive setter can find itself.

**Action wiring is by name, not by target.** Each `<menu_item>`
allocates a process-unique action name `app.menuitem_N` (atomic
counter in `gtk_dom::menu`), registers a `gio::SimpleAction` on the
`gtk::Application`'s action group, and connects its
`activate` signal to the user closure. `gtk_application_set_menubar`
walks the `gio::Menu` model and binds `app.menuitem_N` references
to the registered action. Keyboard accels go through
`Application::set_accels_for_action("app.menuitem_N", &["<Primary>r"])`.

**`MenuParent` carries both menu + app.** Unlike cocoa (where
`NSApplication::sharedApplication` is a process-wide singleton),
the GTK `gtk::Application` isn't globally addressable. The Render
cascade threads `&gtk4::Application` through `MenuParent::{Bar,
Menu}` so leaf items can `add_action` / `set_accels_for_action`
on it. `MenuBar::build` discovers the app at the top via
`gio::Application::default()` (downcast to `gtk::Application`),
which works inside `run()`'s activate handler — anywhere else
panics with a clear diagnostic.

**Separators are sections.** GTK doesn't have a "separator" item
kind; the convention is to group items into `gio::Menu` *sections*
(via `append_section`). Adjacent sections render with a divider.
`<menu_separator/>` appends an empty section, so subsequent items
added to the parent menu visually start a new group. Functionally
equivalent to AppKit's `+[NSMenuItem separatorItem]` but
structurally different — worth knowing if you read the
`gio::Menu` directly.

**`set_checked` is a v1 stub.** `gio::SimpleAction` exposes
check-mark state via stateful actions
(`SimpleAction::new_stateful`), but rebinding an existing action
to be stateful means rebuilding it — non-trivial for the
build-once shape of our reactive setters. The `set_checked` method
on `gtk_dom::menu::MenuItem` is wired through but no-ops today.
Use a toggle via `on:action=…` + reactive `title=move || …` for
the same UX in v1.

**Files touched.** GTK side:
`gtk/dom/src/menu.rs` (new),
`gtk/dom/src/lib.rs` (mod export),
`gtk/leptos_gtk/src/gtk/menu.rs` (new),
`gtk/leptos_gtk/src/gtk/mod.rs`,
`gtk/leptos_gtk/src/element_gtk.rs`,
`gtk/leptos_gtk/src/event_gtk.rs` (`ActionEvent`),
`gtk/leptos_gtk/src/lib.rs` (prelude + `window` re-export for
`run()` users).
Example: `gtk/examples/menu_demo/`.

---

## 2026-05-12 — `<grid>` container (Taffy CSS-Grid)

Cross-cutting addition. Full design notes in
`implementation_log.md` — the GTK port took the same shape as
cocoa/iOS because all three ports share `common/native_layout`'s
Taffy storage tree.

GTK-specific bits worth flagging:

- The underlying widget for `<grid>` is still a `gtk::Box`
  (`container_widget()` in `gtk/dom/src/node.rs`). Taffy assigns
  final frames to each child via the existing `TaffyLayout`
  manager — the GTK widget class is layout-agnostic at that point.

- GTK's `Label` builder doesn't have a `font_size` setter (cocoa
  / iOS do). The GTK grid example accordingly drops `font_size=`
  attrs that the cocoa / iOS versions carry.

---

For the macOS sibling port (which this one mirrors), see
[`implementation_log.md`](./implementation_log.md). The macOS log is
more detailed because the macOS port hit thornier integration issues
(Taffy bridging, AppKit's lack of native flex layout, the
`mount_before` synthetic-parent dance, etc.). On Linux many of those
problems just don't exist — GTK does its own layout, signal-handler
closures are owned by widgets, glib drives futures natively — so each
GTK stage's log will be correspondingly shorter.

---

## 2026-05-04 — `native-ui` feature flag replaces `target_os` auto-detect

Reworked the `leptos_native` cfg machinery from auto-set-by-target_os
to opt-in via a `native-ui` Cargo feature. The cfg flag itself is
unchanged; only the trigger moved from "Linux or macOS" to "native-
ui feature is enabled".

### Why

Auto-detecting native UI from `target_os` conflated two intents that
share the same target:

- "I'm building a native UI app on Linux" → wants gtk routing, no
  leptos_router
- "I'm building an SSR server on Linux" → wants the web tachys path,
  needs leptos_router + leptos_meta + integrations

Both build on `target_os = "linux"`, so target_os alone can't
disambiguate. With the auto-detect, SSR-on-Linux from this checkout
broke (and SSR-on-macOS was already broken for the same reason). The
feature flag lets the user pick.

### What changed

Build scripts (eight of them: `tachys`, `leptos`, `leptos_dom`,
`router`, `meta`, `integrations/{utils,actix,axum}`) now check the
`CARGO_FEATURE_NATIVE_UI` env var instead of `CARGO_CFG_TARGET_OS`:

```rust
if std::env::var_os("CARGO_FEATURE_NATIVE_UI").is_some() {
    println!("cargo:rustc-cfg=leptos_native");
}
```

Each crate's `Cargo.toml` got a `native-ui` feature:

- **`tachys`**: `native-ui = ["dep:cocoa_dom", "dep:gtk_dom"]`. The
  `cocoa_dom`/`gtk_dom` deps became *optional* — they're only pulled
  in when the feature activates them. The target_os gate on each dep
  stays, so on macOS only cocoa_dom is in scope; on Linux only
  gtk_dom is.
- **`leptos`**: `native-ui = ["tachys/native-ui",
  "leptos_dom/native-ui", "dep:cocoa_dom"]`. `cocoa_dom` (used by
  `mount_macos`) also became optional + feature-activated. The Linux
  side has no `gtk_dom` dep here yet — Stage 5 adds it for
  `mount_gtk`.
- **`leptos_dom`**: `native-ui = ["tachys/native-ui"]`.
- **Web-only crates** (`router`, `meta`, `integrations/*`):
  `native-ui = []`. Defensive: lib.rs is `#![cfg(not(leptos_native))]`,
  so when something in a build enables their `native-ui`, the crate
  goes empty.

Source-level cfgs: every site that previously gated on
`cfg(target_os = "macos")` and pointed to cocoa_dom-dependent code
got tightened to `cfg(all(target_os = "macos", leptos_native))`.
Without this, macOS-without-feature would try to pull in cocoa_dom
(now optional, not pulled), and compilation would break. Same for
Linux/gtk_dom. Affected files:

- `tachys/src/renderer/mod.rs` — `pub mod cocoa`/`gtk`, `Rndr`
  aliases, `types::*` re-exports
- `tachys/src/lib.rs` — prelude `Dom` re-exports, `pub mod cocoa`,
  `svg_macos` facade
- `tachys/src/html/mod.rs` — `element_macos`, `event_macos` facades
- `tachys/src/html/attribute/mod.rs` — `Selection` re-export
- `leptos/src/lib.rs` — `pub mod mount_macos`, `pub use tachys::cocoa
  as cocoa`, prelude re-exports of `mount_macos::*`,
  `cocoa::BindAttribute`, `Suspend`

### Trade-off in feature unification

Cargo unifies features per-crate per-resolution. If `cargo build
--workspace --features native-ui` were attempted, cargo would try
to enable `native-ui` on every workspace member. Members without the
feature would error.

In practice the user's workflow is one of:

- `cargo build --workspace` (no features) — everything in web/SSR
  mode, including router, meta, integrations. ✓
- `cargo run -p my-native-app` where `my-native-app` is a binary
  whose `[dependencies]` enables `leptos = { features =
  ["native-ui"] }` — only my-native-app's dep tree is built, with
  `native-ui` propagated through tachys/leptos/leptos_dom. ✓
- `cargo check -p tachys --features native-ui` — single-crate
  feature build for testing. ✓

The unusual case `cargo build --workspace --features native-ui`
errors today and isn't on the roadmap. If it ever needs to work,
add a no-op `native-ui = []` feature to *every* workspace member.

### Validated

- `cargo build --workspace --exclude cocoa_dom` (default features) —
  clean. router, meta, integrations all compile their full lib.rs.
  ✓
- `cargo check -p tachys --features native-ui` — tachys compiles
  with `leptos_native` set; on Linux gtk_dom is pulled in and the
  `tachys::renderer::gtk` module is included. ✓
- `cargo check -p leptos --features native-ui` — leptos +
  leptos_dom + tachys all in native-ui mode together. ✓
- `cargo build -p gtk_dom --example counter` (no features) — counter
  still builds, since it uses gtk_dom directly and doesn't need the
  tachys native path. ✓

### Updates to tracked TODOs

The "SSR coexistence" item under *Open items* below is now resolved
in principle by this feature flag. Real-world SSR-on-native still
needs Stage-5+ machinery to actually do anything useful, but the
build doesn't break either way.

The reverse case — *native UI without `--features native-ui`* — is
also fine: the user just gets the web tachys path, which is useless
for native rendering but doesn't error.

---

## 2026-05-04 — Stage 2: tachys/leptos compile against gtk_dom on Linux

Wired `tachys::renderer::gtk` and flipped `leptos_native` on for
Linux. After this stage, `cargo check -p tachys -p leptos -p
leptos_dom -p gtk_dom -p cocoa_dom` is clean on Linux: tachys's
`Rndr` resolves to `gtk::Dom` (which delegates to
`gtk_dom::Renderer`), and the rest of the framework follows along.
leptos_native flag is now true on macOS *and* Linux; the cocoa_dom
side is still untouched.

### What shipped

- `tachys/src/renderer/gtk.rs` — direct port of
  `tachys/src/renderer/cocoa.rs`. `Dom` unit struct, methods
  delegating to `gtk_dom::Renderer`, `Mountable` impls for
  `Node`/`Element`/`Text`/`Placeholder`, `CastFrom` impls.
- `tachys/src/renderer/mod.rs` — added `pub mod gtk` and `pub type
  Rndr = gtk::Dom` under `cfg(target_os = "linux")`. Same for the
  `types::*` re-export block.
- `tachys/src/lib.rs` prelude — added `pub use crate::renderer::gtk::Dom`
  for Linux. Generalised the `IntoAttributeValue` /
  `IntoAnyAttribute` re-export from `cfg(target_os = "macos")` to
  `cfg(leptos_native)` since both backends need it.
- `tachys/Cargo.toml` — added `[target.'cfg(target_os = "linux")'.
  dependencies] gtk_dom = ...`.
- `leptos/src/lib.rs` prelude — added a Linux-only re-export of
  `tachys::reactive_graph::Suspend` (needed by `leptos/src/await_.rs`).
  `BindAttribute` is intentionally absent on Linux until Stage 5.
- All three `build.rs` files (`tachys`, `leptos`, `leptos_dom`) —
  flipped Linux's `leptos_native` flag on by changing the conditional
  from `target_os == "macos"` to `target_os == "macos" || target_os
  == "linux"`. Removed the deferral comments.

### Decisions worth flagging

#### `mount_before` is a one-liner on GTK

cocoa_dom's `Dom::mount_before` synthesises an `Element` wrapper
around `before`'s parent NSView *and* derives a `LayoutHandle` from
`before`'s tree slot, so the new child registers in the same Taffy
tree. The `synthesise_parent_element` helper does that bookkeeping.

On GTK the function collapses to "look up `before.widget().parent()`,
wrap it as a synthetic `Element`, call `mount`". No layout tree to
register against; child layout falls out of `gtk::Box` as soon as
the widget is parented. `synthesise_parent_element` exists in
gtk.rs purely as a `Some/None` wrapper around the `parent()` lookup
to share code between `mount_before` and `try_mount_before`.

#### Keep `synthesise_parent_element` even though it's a thin wrapper

Could inline it into both call sites; chose to keep it as a named
helper for symmetry with the cocoa renderer (where it's substantial)
and to keep the `mount_before` body readable. Cost: one extra
function definition.

#### Generalise the prelude re-exports for `IntoAttributeValue` etc.

Was previously `cfg(target_os = "macos")`; both backends need these
re-exports for the same reasons (their builders accept user-typed
values that need converting). Changed to `cfg(leptos_native)`. This
is a small cleanup of a sub-optimal cocoa-stage cfg; the alternative
of duplicating the re-export with `cfg(target_os = "linux")` would
have been noisier.

#### `BindAttribute` is intentionally absent from the Linux prelude

The macOS prelude exports `tachys::cocoa::BindAttribute`. Linux's
parallel `tachys::gtk::BindAttribute` doesn't exist yet — it lands
in Stage 5 alongside the rest of the builder layer. Until then,
Linux's prelude omits `BindAttribute` and `bind:` syntax in `view!{}`
won't resolve. Acceptable because we're not running `view!{}` on
Linux until Stage 5 anyway.

### Validated

- `cargo check -p tachys` ✓ (Linux, native path through `gtk::Dom`)
- `cargo check -p tachys -p leptos -p leptos_dom -p gtk_dom -p cocoa_dom`
  ✓ (workspace subset that's relevant to the gtk path)
- `cargo build -p gtk_dom --example counter` ✓ (counter still builds
  end-to-end after the leptos_native flip; this confirms the
  conditional include of tachys's web vs native paths still does the
  right thing on Linux)
- cocoa_dom still compiles to an empty stub on Linux (its lib.rs
  is `#![cfg(target_os = "macos")]`) ✓

### Web-only crates now gate themselves out of native builds

`leptos_router` and `leptos_meta` both unconditionally import web-only
items (`tachys::dom`, `tachys::html::element`, `leptos_native::ev`,
`leptos_native::html`, `web_sys::*`). Before Stage 2, Linux had
`leptos_native = false`, which kept those modules visible — the
crates compiled. After flipping the flag, those modules disappear
on Linux too, so the crates fail to build on native targets.

User decision: gate them at the crate level instead of leaving them
broken. Same pattern as `cocoa_dom/src/lib.rs` (`#![cfg(target_os
= "macos")]`): the `lib.rs` of each web-only crate gets
`#![cfg(not(leptos_native))]`, so it compiles to an empty rlib on
native targets.

Crates affected:

- `leptos_router` (`router/`) — extended its existing `build.rs` and
  added the cfg attribute.
- `leptos_meta` (`meta/`) — added a new `build.rs` and the cfg
  attribute.
- `leptos_integration_utils` (`integrations/utils/`) — server-side
  rendering helper that depends on leptos_meta + leptos_router.
- `leptos_actix` (`integrations/actix/`) — Actix Web SSR
  integration.
- `leptos_axum` (`integrations/axum/`) — Axum SSR integration.

Each got a small `build.rs` mirroring the pattern from `tachys/build.rs`:

```rust
fn main() {
    println!("cargo:rustc-check-cfg=cfg(leptos_native)");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" || target_os == "linux" {
        println!("cargo:rustc-cfg=leptos_native");
    }
}
```

After this, `cargo build --workspace --exclude cocoa_dom` is fully
clean on Linux (cocoa_dom is excluded because it's macOS-only and
already compiles to empty there).

#### Trade-off: SSR on native targets is now blocked too

Server-side rendering on Linux/macOS shares `target_os` with the
native UI backends but a fundamentally different intent (produce
HTML strings, not native widgets). With this gating in place, you
can't run a Leptos SSR server *and* a native UI app from the same
checkout — the SSR integrations compile to empty rlibs whenever
`leptos_native` is set.

This matches the existing macOS behavior: nobody was running Leptos
SSR on macOS before, because leptos_router/leptos_meta were already
breaking there. Stage 2 didn't introduce this; it just made the
behavior intentional rather than accidental.

If we ever want to support both modes from the same checkout, the
right fix is a Cargo feature flag (e.g. `native-ui`) that crates
opt into instead of inferring from `target_os`. Tracked in *Open
items* below.

### Known gaps / TODOs from this stage

- **`tachys::svg` is undefined on Linux.** The macOS port has
  `svg_macos.rs` aliased as `svg`. Linux has no parallel yet. Not
  needed until Stage 5 enables `view!{}` macro routing (which emits
  `tachys::svg::view` for the `<view>` tag).
- **`tachys::html::element` / `tachys::html::event` are undefined on
  Linux.** Macro facades come in Stage 5.
- **`leptos_native::gtk` re-export not yet added.** Mirrors `leptos_native::cocoa`
  on macOS; lands when `tachys::gtk` (the builder layer) does, in
  Stage 5.

---

## 2026-05-04 — Stage 1: core types + low-level counter

Wired up `gtk_dom`'s façade types and demonstrated end-to-end signal
→ effect → widget reactivity with a 70-line counter example that
imports nothing more than `gtk_dom + reactive_graph`. Specifically:

- `gtk_dom/src/node.rs` — `Node` (wraps `gtk::Widget` via
  `SendWrapper`), `Element`, `Text`, `Placeholder`, `NodeKind`. Tag →
  widget mapping in `Element::create`. Attribute setters (string and
  bool). Tree mutation (`insert_node` / `remove_child` /
  `clear_children`). `on_click` for buttons.
- `gtk_dom/src/renderer.rs` — `Renderer` unit struct mirroring the
  tachys API surface. Web-only methods panic with `unimplemented!()`;
  hydration walkers (`get_parent` / `first_child` / `next_sibling`)
  do too.
- `gtk_dom/src/spawner.rs` — `any_spawner::CustomExecutor` routing to
  `glib::MainContext::default().spawn_local`.
- `gtk_dom/src/app.rs` — `init_app(application_id) → gtk::Application`
  + `run_loop(&app)`. `init_app` also registers the spawner.
- `gtk_dom/src/window.rs` — `open_window(app, title, size) →
  OpenedWindow { gtk_window, content_root }`. The content root is a
  vertical `gtk::Box` installed as the window's child.
- `gtk_dom/examples/counter.rs` — single-counter demo (`-1` / `Reset`
  / `+1` buttons updating a label via `Effect::new` driven by an
  `RwSignal<i32>`).

### Decisions worth flagging

#### No per-`Node` layout slot

cocoa_dom's `Node` carries an `Rc<RefCell<NodeLayout>>` so each node
can hold its Taffy `Style` and (once mounted) its `LayoutHandle`.
gtk_dom doesn't need any of that — GTK widgets self-layout via their
parent's `LayoutManager`. The `Node` struct is therefore much
smaller: just `SendWrapper<gtk::Widget>` + `NodeKind`.

This also means none of the cocoa-side machinery survives:
`LayoutTree`, `register_in_tree`, `attach_child`, `schedule_relayout`,
`mark_dirty`, the `compute_layout_with_measure` callback, the per-tree
`PENDING` set, the `WindowDelegate` that re-runs layout on resize.
GTK handles all of that.

#### `<view>` defaults to **Vertical** orientation

cocoa_dom's generic `<view>` tag maps to a `FlippedView` with no
explicit flex direction (Taffy default = Row). On GTK, `gtk::Box`
demands an orientation at construction time, so `Element::create`
has to pick one for the unspecified case.

I went with `Vertical` because:

- The most common ergonomic case for "container of stuff" in
  SwiftUI-flavoured UIs is vertical stacking.
- It matches the orientation `open_window`'s `content_root` already
  uses (also a vertical Box).
- Users who want horizontal can use `<hstack>` explicitly.

This is a small but visible divergence from the macOS port. May want
to revisit if it bites — see *Open items* at the bottom.

#### Placeholder is just a hidden `gtk::Box`

cocoa_dom's `Placeholder` defaults to `position: Absolute` with size
0×0 in Taffy, otherwise it'd take a flex slot. On GTK,
`set_visible(false)` removes the widget from layout entirely, so no
trick is needed — a plain hidden `Box` works.

#### `set_attribute` / `set_bool_attribute` diff first

Mirrors the cocoa decision: read the current value, compare, only
write if it changed. This is a load-bearing behavior for the
forthcoming `bind:` support — without it, a `RenderEffect` that
writes back an unchanged value still fires the widget's
`changed`/`clicked`/etc. signal, which can cycle through user
handlers and reactive state.

GTK adds one extra reason to diff: programmatic `set_text` on a
`gtk::Entry` re-fires the `notify::text` and `changed` signals. We'll
add `signal_handler_block`/`unblock` guards in Stage 5 (bind:), but
diff-first is cheap belt-and-braces for now.

#### `on_click` stacks; no handler store

cocoa_dom keeps a thread-local `HANDLER_STORE: HashMap<usize,
Vec<Retained<ActionTarget>>>` keyed by NSView pointer to retain its
ObjC target objects past the call. NSControl's target/action only
holds *one* target, so multiple `on_click` calls overwrite earlier
handlers but the previously-retained `ActionTarget` lingers in the
store (currently leaks until view drop).

On GTK the model inverts: each `connect_clicked` call appends a new
signal connection, with the closure owned by the connection itself.
Multiple calls stack — every handler fires per click. The closure
drops when the widget drops (gobject teardown disconnects all
signals automatically). No retention store needed.

The reentrancy guard inside `on_click` (`RefCell::try_borrow_mut` →
log + skip if reentrant) mirrors cocoa's `ActionTarget::action_fired`
guard. Whether GTK actually triggers reentrancy in practice is
unclear — synchronous signal firing during a click handler doesn't
recurse into `clicked` on the same button, but a click handler that
mutates an `RwSignal` driving a `RenderEffect` that writes back to
the same button's title doesn't re-enter either. Cheap insurance;
remove if it never trips.

#### `teardown` calls `unparent` only

cocoa_dom's teardown does three things: drop Taffy node, drop
event-handler retains, remove NSView from superview. The first two
are unnecessary on GTK (no Taffy, no retain store), so `teardown` is
just `widget.unparent()` (guarded by a parent-check to make it a
no-op when called twice).

The widget itself isn't explicitly freed — gobject reference counting
takes care of that once both (a) all clones of the wrapping `Node`
drop and (b) no parent holds it.

#### `insert_node` only handles `gtk::Box` and windows

The marker-aware insert path uses `gtk::Box::insert_child_after` with
the marker's previous sibling (so the new child lands immediately
before the marker). For `gtk::Window` and `gtk::ApplicationWindow`,
which only accept a single child, we call `set_child(Some(...))` and
ignore the marker.

Other container classes (`gtk::Frame`, `gtk::Grid`, `gtk::Stack`,
`gtk::ScrolledWindow`, `gtk::HeaderBar`, `gtk::Notebook`, …) silently
drop the insert. Stage 1 doesn't ship support for them; we'll wire
each as the higher-level API needs them.

#### Spawner: route both `spawn` and `spawn_local` through `MainContext::spawn_local`

cocoa_dom's spawner has to do real work — implement `Wake`, queue
`poll_on_main` onto the libdispatch main queue, manage the `queued`
atomic flag for coalescing wakeups. None of that on GTK: glib's
`MainContext::spawn_local` already integrates a future as a
`GSource`, polled by the same main loop GTK is draining. We just
hand it the future and forget about it.

The `PinnedFuture<()>` parameter to `CustomExecutor::spawn` is
nominally `Send`, but glib doesn't care — single-threaded model.
Routing both through `spawn_local` collapses two code paths into
one.

#### Spawner is initialized inside `init_app`

Same pattern as cocoa. The user calls `init_app(application_id)`
which:

1. Calls `spawner::init()` — idempotent, ignored if already set.
2. Builds the `gtk::Application`.

Activation (`connect_activate`) runs later, inside `app.run()`, on
the main loop. By then the spawner is wired so any `Effect::new` /
`RwSignal` created inside the activate callback works correctly.

#### `<text_field>` → `gtk::Entry`, not `gtk::Text`

GTK4 has both. `gtk::Text` is the lower-level single-line text widget
that `Entry` uses internally; `Entry` adds the visible bordered
chrome users expect from a text field. Going with `Entry` matches
the visual appearance of cocoa_dom's `<text_field>`
(NSTextField, which has a focus ring and bordered bezel). Most
real-world Linux apps use `Entry`.

#### `<pop_up_button>` → `gtk::DropDown`, not `gtk::ComboBoxText`

`ComboBoxText` is the legacy GTK3-style combo. `DropDown` is the
modern GTK4 widget — it uses a `GListModel`-backed factory pattern
which is more aligned with the kind of dynamic-items + bindable-
selection wiring we'll want in Stage 5/6. Going forward.

`DropDown::default()` gives an empty initial model; we'll set the
items list in Stage 5 via a `StringList` + `set_model`.

### Validated

- `cargo build -p gtk_dom --example hello_window` ✓
- `cargo build -p gtk_dom --example counter` ✓
- `cargo check -p gtk_dom -p tachys -p leptos -p leptos_dom -p cocoa_dom` ✓
  (the macOS-side crates still compile clean on Linux as cfg-empty
  stubs, confirming the cfg refactor in Stage 0 didn't regress the
  shared codepaths.)

User-side runtime validation: counter window opens, three buttons,
clicking updates the label. (Pending — I can't run a GUI app from
this session, the user reported success on hello_window after Stage
0.)

### Known gaps / TODOs from this stage

- **No `event.rs` module yet.** `on_click` lives on `Element`
  directly. When Stage 3 adds text-input events, slider value-change
  events, dropdown selection events, etc., factor those into
  `gtk_dom/src/event.rs` for parity with cocoa_dom.
- **No tachys integration.** The Linux build still compiles via the
  *web* path (`leptos_native` flag is off on Linux). Stage 2 wires
  `tachys::renderer::gtk` and flips the flag.
- **Leaked Owner in counter example.** Same lifecycle gap as cocoa's
  examples — no `UnmountHandle`-on-window-close story yet. Track in
  Stage 5 alongside `mount_to_window`.
- **No `SignalHandlerId` tracking on `connect_clicked`.** Currently
  fire-and-forget; the closure drops with the widget. Once we add
  `bind:` (Stage 5) we'll need to disconnect specific handlers
  during reactive cleanup, which requires holding the
  `SignalHandlerId` somewhere.
- **`clear_children` on non-Box/Window parents is a no-op.** Same
  as cocoa's "we don't have a back-map from raw widget to wrapping
  Node" limitation. Probably fine until `<For>` actually uses a
  non-Box parent.

---

## 2026-05-04 — Stage 0: cfg machinery + gtk_dom scaffold

Set up the workspace plumbing so a third target (Linux/GTK) can be
added alongside macOS without further multiplication of cfg
directives, and shipped a minimal `hello_window` example to confirm
GTK4 system deps are wired up.

### Decisions worth flagging

#### `cfg(leptos_native)` umbrella, not `cfg(target_os = ...)`

The macOS port introduced cfg gates of the form:

```rust
#[cfg(target_os = "macos")]      // macOS-specific code
#[cfg(not(target_os = "macos"))]  // implicitly: web-only code
```

With Linux entering the picture, every `not(target_os = "macos")`
site would have to grow into `not(any(target_os = "macos",
target_os = "linux"))`. Verbose, easy to forget one, and conflates
"this is web" with "this is everything-other-than-macOS".

Decided to introduce a custom cfg flag, `leptos_native`, emitted by
each crate's `build.rs` when `target_os` is `macos` or `linux`. The
new vocabulary:

- `cfg(leptos_native)` — the native path (either backend)
- `cfg(not(leptos_native))` — the web path
- `cfg(target_os = "macos")` — macOS-specific (the cocoa_dom backend)
- `cfg(target_os = "linux")` — Linux-specific (the gtk_dom backend)

Implementation: extended `tachys/build.rs` and `leptos/build.rs` to
emit `cargo:rustc-cfg=leptos_native`. Added a new
`leptos_dom/build.rs` to do the same. Each also emits
`cargo:rustc-check-cfg=cfg(leptos_native)` so the `unexpected_cfgs`
lint doesn't complain.

Refactored 80 source-level `not(target_os = "macos")` occurrences
across 11 files (tachys + leptos + leptos_dom) into
`not(leptos_native)` via a single sed pass. A handful of
`cfg(target_os = "macos")` sites that were actually "any-native"
stubs (the hydration `failed_to_cast_*` panic helpers, the
`Vec<&str>`/`Vec<String>` `IntoAttributeValue` escape hatches, an
unused `let _ = prop_name;` in `any_attribute.rs`) also moved to
`cfg(leptos_native)` because they apply equally to macOS and Linux.

#### Linux's `leptos_native` flag is *intentionally* not flipped on yet

If `leptos_native` were set on Linux at this stage, the source code
gates would exclude the web path (`pub mod dom`, `pub type Rndr =
dom::Dom`) but there's no Linux replacement yet — so `Rndr` would be
undefined and tachys wouldn't compile.

The build.rs files therefore only flip `leptos_native` on for
`macos` for now. Linux falls into `cfg(not(leptos_native))` =
"web", which is what currently keeps the Linux build working: it
compiles the wasm-bindgen-flavoured renderer, just like before.

Stage 2 will: (a) add `tachys::renderer::gtk::Dom` and the parallel
`pub type Rndr = gtk::Dom` for `cfg(target_os = "linux")`, then (b)
extend the build.rs check from `target_os == "macos"` to also include
`"linux"`. Until both happen together, Linux builds against the web
renderer (which is fine because we don't yet have Linux examples
that depend on the native path).

A comment in each build.rs file documents this.

#### Crate name: `gtk_dom`

User confirmed. Mirrors `cocoa_dom` — the name hints "DOM-shaped
façade over the platform UI toolkit". Leaves room for future ports
(`linux_qt_dom`, `win_dom`, etc.) if anyone wants them, since the
crate name commits to one specific toolkit rather than "all Linux".

#### GTK4 only, no GTK3 fallback

GTK3 is in maintenance. GTK4 is what every modern Linux distro ships
with. Pinning to GTK4 lets us use `gtk::DropDown`, `gtk::Box::append`,
the modern event controller / gesture API, etc. — and keeps the
binding surface manageable.

#### Skip Taffy on GTK

GTK already does flex-style layout natively (`gtk::Box` with
orientation + spacing + child `hexpand`/`vexpand` covers most
SwiftUI-flavoured needs). The macOS port reaches for Taffy because
AppKit doesn't reflow itself; on GTK, paying the Taffy tax — measure
callbacks, dirty-tracking, scheduled relayouts, NSView↔Node back-
mapping concerns — buys nothing.

The user agreed and explicitly noted "though if we run into trouble,
I might change my mind on that later." If we do switch:

1. Add `gtk_dom/src/layout.rs` mirroring `cocoa_dom`'s.
2. Replace each `gtk::Box`-using container with a custom widget
   subclassing `gtk::Widget` and using a custom `LayoutManager` that
   walks its children's Taffy `Layout`s and `set_size_request`s them.
3. Keep `Element::insert_node`'s flex/orientation setters for
   compatibility, but route them into Taffy `Style` mutations.

Estimated cost: about 2 days of work, mostly the custom
`LayoutManager`. Defer until we hit a concrete layout problem GTK
can't express.

#### Application ID configurable from day one

User asked for this. `init_app(application_id: &str)` takes the ID
explicitly, no global default. Each example picks its own
(`org.leptos.gtk_dom.counter`, `org.leptos.gtk_dom.hello_window`).

### Validated

- `cargo check -p tachys` ✓ (Linux, web path)
- `cargo check -p leptos -p leptos_dom` ✓ (Linux, web path)
- `cargo check -p cocoa_dom` ✓ (Linux; compiles to empty stub via
  `#![cfg(target_os = "macos")]`)
- `cargo check -p gtk_dom` ✓ (after `libgtk-4-dev` install)
- `cargo run -p gtk_dom --example hello_window` ✓ (user-confirmed)

### Known gaps / TODOs from this stage

- **`leptos_native` not yet enabled on Linux.** Flipped on in Stage 2
  alongside the `tachys::renderer::gtk` module addition.
- **No way to influence application ID at the Cargo level yet.** OK
  for now — the `mount_to_window`-style helper coming in Stage 5
  will accept it as a parameter (or wrap it in a builder).
- **GTK4 system deps** (`libgtk-4-dev` on Debian/Ubuntu, `gtk4-devel`
  on Fedora, `gtk4` on Arch) need to be installed by the user before
  any gtk_dom code can compile. Not a code issue, but worth
  documenting in CLAUDE.md eventually.

---

## Open items — aggregated TODOs across all stages

A consolidated view of deferred work, roughly ordered by which stage
will pick each one up.

### Stage 2 — cfg-out web, wire tachys::gtk *(done 2026-05-04)*

All items shipped — see Stage 2 entry above. The `gtk_dom` dep was
added to `tachys/Cargo.toml` only; `leptos/Cargo.toml` will get its
own dep in Stage 5 when `mount_gtk` lands and leptos starts using
`gtk_dom` types directly. Today, leptos picks up gtk_dom transitively
via tachys.

### Stage 3 — events, spawner refinements (next)

- Factor `on_click` out of `Element` into a new
  `gtk_dom/src/event.rs` mirroring cocoa_dom's structure. Add:
  - text-input via `gtk::Entry::connect_changed` (per keystroke) and
    `connect_activate` (return key) — fan-out to multiple handlers
    via a shared `Vec` like cocoa's `TextFieldDelegate`.
  - checkbox toggled via `gtk::CheckButton::connect_toggled`.
  - slider value-changed via `gtk::Scale::connect_value_changed`.
  - dropdown selected-notify via
    `gtk::DropDown::connect_selected_notify`.
- Decide whether to track `SignalHandlerId`s on the `Node` so we can
  disconnect them in `teardown`. (The widget drop already disconnects
  everything, so this is mostly relevant for `bind:` cleanup where a
  long-lived widget wants to drop a specific subscription.)
- Replace the `RefCell::try_borrow_mut` reentrancy guard with
  something less verbose (or delete it if it never trips).

### Stage 4 — layout (likely empty)

GTK does its own layout. This stage may end up being just:

- Wire `<view>`/`<vstack>`/`<hstack>` builder methods to the GTK
  setters: `set_orientation`, `set_spacing`, `set_margin_*`,
  `set_hexpand`, `set_vexpand`, `set_halign`, `set_valign`.
- Document the `flex_grow` weight loss: on macOS it's an `f32`, on
  GTK it collapses to `bool` because `hexpand` is binary.

### Stage 5 — tachys::gtk builders, bind:, view!{} macro

- Mirror `tachys/src/cocoa/{element,attr,bind,window,render_html_stub}.rs`
  in `tachys/src/gtk/`.
- `bind:value`/`bind:checked`/`bind:selection` need
  `glib::SignalHandlerId` + `signal_handler_block` /
  `signal_handler_unblock` around effect-driven writes to suppress
  the inverse round-trip. Diff-first stays as belt-and-braces.
- Macro facades: `tachys/src/html/element_gtk.rs`,
  `tachys/src/html/event_gtk.rs`, `tachys/src/svg_gtk.rs`. Largely
  paste-and-rename from the macOS facades.
- `leptos/src/mount_gtk.rs` with `run` + `mount_to_window`.
  `mount_to_window` should accept the application ID (currently
  hard-coded in examples) — propose
  `mount_to_window(app_id, title, size, view_fn)`. Or a builder.
- `pub use tachys::gtk::BindAttribute` re-export in
  `leptos_native::prelude` for `target_os = "linux"`, mirroring the cocoa
  re-export.

### Stage 6 — dynamic children, more controls

- Verify `<For>` works against `gtk::Box` parents via the existing
  `insert_child_after` path. Likely no synthetic-parent dance needed
  (unlike cocoa) because there's no Taffy tree to register against.
- Port the rest of the example set: `counters_gtk`, `login_form_gtk`,
  `checkbox_gtk`, `greeter_gtk`, `settings_gtk`.

### Cross-cutting / nice-to-have

- **Shared `tachys::native` module.** `MaybeReactive`, `install`,
  `PendingHandler` are renderer-agnostic and currently duplicated
  between `tachys::cocoa` and (will be) `tachys::gtk`. Extract once
  the GTK port stabilises. Don't do this preemptively — let the
  duplication settle first so we know exactly which pieces are
  shape-compatible.
- **`RenderHtml` feature-gated out of `IntoView`.** Tracked in the
  cocoa log too. Eliminates the `cocoa_stub_view_impls!` /
  `gtk_stub_view_impls!` macros entirely. Larger upstream-ish
  change.
- **flex_grow weight loss.** Document in code comments where the
  builder accepts an `f32` but the underlying GTK widget reads it
  as a bool. Maybe warn if `flex_grow > 0.0 && < 1.0` since that's
  meaningless on GTK.
- **`<view>` Vertical default vs cocoa's Row default.** Revisit if
  users get confused. The simplest reconciliation if it bites:
  pick one default and apply it to both backends — would need to
  flip cocoa's default container too, which is a backwards-
  incompatible change for the macOS examples.
- **Reentrancy guard on `on_click`.** May be unnecessary on GTK.
  Trace if it ever logs the "reentrant click handler skipped"
  message and remove if it doesn't.
- **Owner cleanup on window close.** Both ports leak the `Owner`
  currently. A real `UnmountHandle`-style story should land
  eventually, ideally shared between the two backends.
- **CI / automated UI tests.** User said deferred. macOS is having
  XCUIAutomation tests added in parallel; once that settles, the
  GTK side could use `gtk4::test_utils` or one of the GTK
  integration test frameworks.
- **Multi-window.** macOS's Stage "Multi-window" log entry covers
  per-window TaffyTree separation. On GTK each window is naturally
  independent — `gtk::ApplicationWindow` instances share an
  `Application` but layout separately. Should Just Work, but worth
  validating with a `two_windows_gtk` example mirroring cocoa's.

## 2026-05-20 — TLS node store + `Copy` `NodeId` (see implementation_log.md)

The cross-cutting `Node`-becomes-`NodeId`-over-a-thread-local-store
refactor landed on this port too, mirroring cocoa one-for-one. Full
rationale + the shared design is in the top entry of
`implementation_log.md` (2026-05-20). Port-local notes: same
`LayoutBackend::with_tree` + `thread_local!` store, `Node` is a `Copy`
id (no `Rc`/`SendWrapper`/refcount), explicit teardown+cascade
lifecycle, and the walk-up-to-root relayout scheduler.
