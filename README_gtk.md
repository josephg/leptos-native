# Leptos — GTK4 native port (Linux)

This fork extends [Leptos](https://leptos.dev) with a native Linux UI
backend built on **GTK4**. The same `view!{}`, `#[component]`, and
fine-grained reactive primitives that drive a web Leptos app drive
GTK widgets instead. The web framework is **still intact** on the
same branch — `cargo build` against the default features uses the
existing wasm/SSR pipeline; the GTK path is opt-in via a Cargo
feature.

There's a parallel macOS/Cocoa port on the same branch (see
[`CLAUDE.md`](./CLAUDE.md)). The two ports share an architectural
shape; this README focuses on the GTK side.

## Status

Early in development. Built up in numbered stages:

| Stage | Status | What it ships |
|-------|--------|---------------|
| 0 | ✅ done | `gtk_dom` crate scaffold, `cfg(leptos_native)` machinery, hello\_window example |
| 1 | ✅ done | `Node`/`Element`/`Text`/`Placeholder` over `gtk::Widget`, `Renderer`, `app`/`window`/`spawner`, low-level counter example |
| 2 | ✅ done | `tachys::renderer::gtk` Mountable/CastFrom impls, `Rndr = gtk::Dom` on Linux native |
| — | ✅ done | `native-ui` Cargo feature replaces target\_os auto-detect |
| 3 | 🚧 next | Events module (text input, slider, dropdown, checkbox toggle), spawner refinements |
| 4 | — | Layout convenience setters routing to GTK natives (mostly already implicit) |
| 5 | — | `tachys::gtk::*` builders, macro facades (`view!{}` macro on Linux), `mount_gtk.rs` |
| 6 | — | `bind:`, `<For>` keyed iteration, the rest of the example set |

The day-to-day decisions journal lives in
[`gtk_implementation_log.md`](./gtk_implementation_log.md) (newest at
top).

## Prerequisites

You need the GTK4 development headers + pkg-config:

```sh
# Debian / Ubuntu
sudo apt install libgtk-4-dev pkg-config

# Fedora
sudo dnf install gtk4-devel pkg-config

# Arch
sudo pacman -S gtk4 pkgconf
```

The Rust side picks the toolchain up via `pkg-config` automatically
(no manual env var needed).

## Quickstart

Two minimal demos live under `gtk_dom/examples/`:

```sh
# An empty 400×220 window
cargo run -p gtk_dom --example hello_window

# A counter with -1 / Reset / +1 buttons updating a reactive label
cargo run -p gtk_dom --example counter
```

Both examples target the **low-level façade** directly (no `view!{}`
macro yet — that lands in Stage 5). They demonstrate the bare
imperative API: build widgets, wire signals, wire effects, run the
loop.

## Using it in your own project

> ⚠️ The high-level API (`view!{}` + `#[component]` + `mount_to_window`)
> is Stage 5 work and isn't usable on Linux yet. The instructions
> below are what *will* work once Stage 5 lands; for now you can
> only use `gtk_dom` directly, as in the counter example.

```toml
# Cargo.toml of your binary
[dependencies]
leptos = { git = "...", features = ["native-ui"] }
```

The `native-ui` feature flag triggers the native UI rendering path,
and `target_os` chooses between the two backends automatically:

| Build target | `native-ui` | Renderer |
|---|---|---|
| `wasm32-unknown-unknown` | irrelevant | web (DOM) |
| `x86_64-unknown-linux-gnu` | off | web/SSR (DOM strings, server fns) |
| `x86_64-unknown-linux-gnu` | **on** | **GTK4** |
| `x86_64-apple-darwin` | off | web/SSR |
| `x86_64-apple-darwin` | **on** | **AppKit/Cocoa** |

There's no separate `gtk` or `cocoa` feature — pick `native-ui` and
the build picks the right one.

## How it's wired up

Four layers, lowest first. Mirrors the macOS port one-for-one in
shape; the differences are flagged inline.

### `gtk_dom/` — DOM-shaped façade over GTK4

Lowest layer. Provides `Node`, `Element`, `Text`, `Placeholder`
types that loosely mirror their `web_sys` equivalents, but are
backed by `gtk::Widget` (and subclasses like `gtk::Button`,
`gtk::Entry`).

Modules:

- `node.rs` — the wrappers + `Element::create(tag)` + attribute
  setters + tree mutation (`insert_node`/`remove_child`/
  `clear_children`).
- `renderer.rs` — the `Renderer` unit struct presenting the
  tachys-shaped imperative API.
- `app.rs` — `init_app(application_id) → gtk::Application` +
  `run_loop`. `init_app` also registers the spawner.
- `window.rs` — `open_window(app, title, size) → OpenedWindow`. The
  content root is a vertical `gtk::Box` set as the window's child.
- `spawner.rs` — `any_spawner::CustomExecutor` over
  `glib::MainContext::spawn_local`. Lets `RwSignal`/`Effect` work
  out of the box once `init_app` has run.

**No `layout.rs`.** GTK does its own layout. The macOS port has a
~600-line Taffy bridge here; on GTK that's all unnecessary. Setting
flex direction, gap, padding, expand on a container goes directly to
the corresponding GTK setter.

### `tachys/src/renderer/gtk.rs` — bridges `gtk_dom` to tachys

Adds the `Mountable` and `CastFrom` impls that have to live in
tachys for orphan-rule reasons. `Dom` is a unit struct (not a type
alias) so we can attach `mount_before` / `try_mount_before` —
methods that depend on `Mountable`.

Compared with the macOS sibling, this file is much shorter:
`mount_before` is a one-liner (`widget.parent()` → wrap → mount)
versus cocoa's elaborate Taffy `LayoutHandle` propagation.

### `tachys/src/gtk/` — element builders *(Stage 5)*

`Button`/`Checkbox`/`Label`/`TextField`/`Slider`/`PopUpButton`/
`View<Ch>` builder structs that implement `Render`. The `view!{}`
macro emits paths into this module via the macro facades in
`tachys/src/html/element_gtk.rs` and `event_gtk.rs`.

### `leptos/src/mount_gtk.rs` — entry points *(Stage 5)*

`run(closure)` and `mount_to_window(app_id, title, size, closure)`.

## Tag → widget mapping

The current `Element::create` map (snake\_case tag names by
deliberate convention, even where the GTK class is PascalCase):

| Tag | GTK widget |
|-----|------------|
| `<button>` | `gtk::Button` |
| `<checkbox>` | `gtk::CheckButton` |
| `<label>` | `gtk::Label` |
| `<text_field>` | `gtk::Entry` |
| `<secure_text_field>` | `gtk::PasswordEntry` |
| `<slider>` | `gtk::Scale` (horizontal) |
| `<pop_up_button>` | `gtk::DropDown` |
| `<vstack>` / `<stack_view>` / `<view>` (default) | vertical `gtk::Box` |
| `<hstack>` | horizontal `gtk::Box` |

HTML compatibility is a non-goal. We're free to invent tags
(`<vstack>`, `<hstack>`, `<checkbox>`) without worrying about HTML
semantics.

## Attribute setters

Diff-first to avoid redundant signal fires (which would cause
`bind:` cycles):

| Attribute | Routes to |
|-----------|-----------|
| `title` | `Button::set_label` / `CheckButton::set_label` / `Label::set_label` |
| `value` | `Entry::set_text` / `Label::set_label` |
| `placeholder` | `Entry::set_placeholder_text` / `PasswordEntry::set_placeholder_text` |
| `enabled` (bool) | `Widget::set_sensitive` |
| `hidden` (bool) | inverted `Widget::set_visible` |
| `checked` (bool) | `CheckButton::set_active` |

## Events

Currently `on_click` lives on `Element` (calls
`Button::connect_clicked`). Event-handler closures are owned by the
signal connection itself, which is owned by the widget — when the
widget drops, all handlers drop with it. No thread-local handler
store needed (unlike the macOS port's `keep_target_alive`).

Stage 3 will add a dedicated `gtk_dom/src/event.rs` with helpers
for text-input (`Entry::connect_changed`, `connect_activate`),
checkbox toggle, slider value-change, and dropdown selected-notify.

## Threading

`gtk::Widget` is `!Send` — GTK widgets are main-thread-only. We wrap
each `Node` in `SendWrapper` so it's nominally `Send + 'static`
(needed for tachys/reactive\_graph generic plumbing) with a runtime
panic if accessed off-main. This mirrors the single-threaded model
that `web_sys` uses in the browser.

`Effect::new`-spawned tasks are routed through
`glib::MainContext::spawn_local` and run on the same loop GTK is
draining — so signal updates, effect re-runs, and widget mutations
all happen on the GTK main thread.

## The `native-ui` Cargo feature

Internally a build-script-emitted `cfg(leptos_native)` triggers the
native code paths in tachys and friends. The feature, declared on
each affected crate, just turns the cfg on:

```rust
// tachys/build.rs (and seven siblings)
if std::env::var_os("CARGO_FEATURE_NATIVE_UI").is_some() {
    println!("cargo:rustc-cfg=leptos_native");
}
```

Crates that participate:

- `tachys`, `leptos`, `leptos_dom` — toggle their renderer between
  web (default) and native (feature on).
- `leptos_router`, `leptos_meta`, `leptos_integration_utils`,
  `leptos_actix`, `leptos_axum` — these are inherently web-only.
  Each has a defensive `native-ui = []` feature that, if turned on,
  gates the crate's `lib.rs` to empty (so a build that accidentally
  pulls them in doesn't fail when `tachys::dom` is excluded).

The optional native-renderer crates (`gtk_dom` on Linux, `cocoa_dom`
on macOS) are activated through `[target.'cfg(target_os = "X")'.
dependencies]` blocks gated on the feature:

```toml
# tachys/Cargo.toml
[target.'cfg(target_os = "linux")'.dependencies]
gtk_dom = { path = "../gtk_dom", optional = true }

[features]
native-ui = ["dep:cocoa_dom", "dep:gtk_dom"]
```

`dep:gtk_dom` is a no-op on macOS (where the dep isn't declared);
same for `dep:cocoa_dom` on Linux. So enabling `native-ui` does the
right thing on either OS without further config.

### Workflow combinations

| Command | Result |
|---|---|
| `cargo build --workspace` | Default features. Web/SSR mode. router, meta, integrations all compile. |
| `cargo build -p gtk_dom --example counter` | Counter builds (uses `gtk_dom` directly). |
| `cargo check -p tachys --features native-ui` | tachys with the GTK renderer (on Linux) or Cocoa (on macOS). |
| `cargo run -p some_app` (where `some_app`'s Cargo.toml enables `leptos/native-ui`) | Native UI app, full stack. |

`cargo build --workspace --features native-ui` doesn't currently
work because not every workspace member has a `native-ui` feature
(only the ones that need to). Not a real workflow; flagged in the
implementation log if you ever need it.

## What works today

Hand-built, low-level GTK apps using `gtk_dom` directly with
`reactive_graph::{RwSignal, Effect}` for reactivity:

- Open one or more `GtkApplicationWindow`s.
- Build a tree of `Element`s + `Text` nodes.
- Wire `Effect` to re-run on signal changes (label updates etc.).
- Wire `on_click` to mutate signals.

The full counter example is ~70 lines (see
[`gtk_dom/examples/counter.rs`](./gtk_dom/examples/counter.rs)).

## Known limitations

Mostly because of stage ordering — these are scheduled work, not
fundamental constraints:

- **No `view!{}` macro on Linux yet** (Stage 5).
- **No `#[component]` on Linux yet** (Stage 5 — depends on the
  builder layer).
- **No `bind:`** (Stage 5).
- **Most events not wired up.** Only `on_click` ships today; text
  input, slider, dropdown, checkbox toggle land in Stage 3.
- **Most controls have no builder layer yet.** `Element::create`
  knows the full tag set but only `<button>` and `<label>` have
  high-level reactivity wired through Stage 1's example.
- **No `<For>` keyed iteration** (Stage 6).
- **No `mount_to_window` entry point** (Stage 5 — for now you write
  the `init_app` + `connect_activate` boilerplate yourself).
- **`flex_grow` is binary on GTK.** `gtk::Widget::set_hexpand`
  /`set_vexpand` are bools, not weighted floats. Tracked in the
  implementation log.
- **`<view>` defaults to vertical orientation**, not Row like the
  macOS port. Different default; flagged in the log if it ever
  causes confusion.

See [`gtk_implementation_log.md`](./gtk_implementation_log.md)
*Open items* section for the full TODO list, organized by stage.

## Contributing / further reading

- **[`gtk_implementation_log.md`](./gtk_implementation_log.md)** —
  chronological design-decision journal. Newest at top. Read the
  entries for whichever stage's behavior you're investigating.
- **[`CLAUDE.md`](./CLAUDE.md)** — the macOS port's instructions for
  Claude Code. Mostly applicable here too; differences (no Taffy,
  signals instead of target/action, GTK4 system deps) are flagged in
  the GTK log.
- **[`implementation_log.md`](./implementation_log.md)** — the macOS
  port's own decision log. Useful background on what
  `gtk_implementation_log.md` mirrors and where it deliberately
  diverges.
- **[Leptos book](https://book.leptos.dev/)** — upstream framework
  docs. The reactive primitives (`RwSignal`, `Effect`, `Memo`, etc.)
  work identically; only the rendering target changes.
