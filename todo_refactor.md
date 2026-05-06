# Refactor: split leptos-mac into upstream patch + standalone leptos-native repo

This file tracks the in-progress refactor that splits this fork into:

1. A minimal, upstream-friendly patch to Leptos.
2. A standalone repository at `~/src/leptos-native` that holds all native rendering crates and glue.

The full plan with rationale lives at `~/.claude/plans/tender-sniffing-star.md`. This file is the in-repo summary + checklist.

## Context

This fork pollutes core leptos crates with: a `leptos_native` cfg routed through six `build.rs` files; per-OS facade modules in `tachys::html::{element,event}_*`; `tachys::svg_{macos,gtk,ios}`; per-OS renderer modules in `tachys::renderer/`; per-OS element trees in `tachys::{cocoa,gtk,ios}/`; per-OS `mount_*` modules in `leptos`; native-only `IntoAttributeValue` impls; `Selection` re-exports for `bind:`; plus `#![cfg(not(leptos_native))]` gating on five web-only crates that compile to empty rlibs in native builds. A `RenderHtml` stub module per backend exists solely to satisfy `IntoView`'s unreachable `RenderHtml` bound.

User decisions:
- **A**: Drop `RenderHtml` from `IntoView` on native. Delete all stub code.
- **B/C**: JSX-style configurable elements import. Macro emits `__leptos_view::*`; users `use {leptos|leptos_native}::view_prelude::*;`. SVG/MathML special-casing in the macro is removed.
- **D**: Drop the empty-rlib hack on `meta`/`router`/`integrations`. Document them as web-only. For genuinely wasm-only paths, add `compile_error!` when not building for wasm.
- **E**: Stop using `cfg(leptos_native)`. Use `cfg(feature = "native-ui")` directly. Stop using `target_os` for web-vs-native dispatch. Add a `web` feature, make `web` and `native-ui` mutually exclusive with exactly one required (compile-time check in `leptos/src/lib.rs`).
- **F**: Workspace member relocation is part of the move, not its own step.
- **G** (`into_attribute_value` refactor): **deferred** to a separate work item.
- **H**: `Selection` and any other `bind:`-key types follow the same macro path as elements/events/attrs, re-exported in the same `view_prelude`.

## Phases

### Phase 0 — Bootstrap leptos-native, relocate self-contained native crates ✅

- [x] Created `~/src/leptos-native/Cargo.toml` workspace.
- [x] Moved `cocoa_dom/`, `gtk_dom/`, `ios_dom/`.
- [x] Moved `examples_cocoa/`, `examples_gtk/`, `examples_ios/`, `xcuitests/`.
- [x] Moved implementation-log MDs.
- [x] Updated `tachys/Cargo.toml` and `leptos/Cargo.toml` path deps to `../../leptos-native/<crate>`.
- [x] Removed native members from leptos workspace; removed `examples_*` from excludes; removed `gio`/`gtk4` workspace deps.
- [x] Verified: cocoa counter example builds end-to-end across the split repos.

Known issue (deferred to Phase 1): on macOS, `cargo build --workspace` (without `native-ui`) fails because `tachys/Cargo.toml` puts `wasm-bindgen`/`web-sys`/`js-sys` under `[target.'cfg(not(target_os = "macos"))'.dependencies]`. Phase 1's feature-based gating fixes this.

### Phase 1 — Replace `cfg(leptos_native)` with `cfg(feature = "native-ui")`; add `web` feature; mutex check ✅

- [x] Added `web = []` feature to `leptos`, `tachys`, `leptos_dom`. csr/hydrate/ssr each implicitly activate `web`.
- [x] Mass-replaced `cfg(leptos_native)` → `cfg(feature = "native-ui")` and `cfg(not(leptos_native))` → `cfg(feature = "web")`.
- [x] Replaced `cfg(not(target_os = "macos"))` gating of wasm-bindgen/web-sys with `feature = "web"` gating.
- [x] Deleted redundant build.rs files in `leptos_dom`, `meta`, `integrations/{actix,axum,utils}`. Trimmed `leptos/build.rs` and `tachys/build.rs` and `router/build.rs`.
- [x] Compile-time mutex check at `tachys/src/lib.rs` (fires before any tachys source compiles, the right place; mirrored in `leptos/src/lib.rs`).
- [x] Cleaned up `cfg(target_os = "macos")` in leptos prelude that should have been gated on native-ui too.

Discovered a Cargo gotcha: `default-features = false` on a member crate is silently ignored unless the workspace-level dep declaration also has `default-features = false`. Fixed by adding `default-features = false` to `tachys`/`leptos_dom`/`leptos_server` workspace deps in both repos. As a consequence, dropped `web` from default features on tachys/leptos/leptos_dom — Cargo's feature unification can otherwise leak `web` into native builds via transitive deps. Web users now opt in via `csr`/`hydrate`/`ssr` (which all activate `web`) or by explicitly enabling `web`.

Also folded in the lib.rs cfg removal for `meta`, `router`, `integrations/{actix,axum,utils}` (Phase 2's mechanical bit) since handling them via the rename was uglier than just deleting the gating.

Verified:
- Cocoa counter example builds end-to-end.
- iOS counter example builds (cross-compiled to aarch64-apple-ios-sim).
- Web workspace builds clean (`cargo check --workspace`).
- Mutex compile_error! fires on both `web,native-ui` and `--no-default-features` with neither enabled.

Remaining nit: GTK example doesn't build on macOS (GTK isn't available there); not a regression.

### Phase 2 — Drop empty-rlib hack on web-only crates (partially folded into Phase 1)

- [x] Removed `#![cfg(feature = "web")]` from `meta`, `router`, `integrations/*` lib.rs (folded into Phase 1).
- [x] Removed `native-ui` feature from those crates' Cargo.toml (folded into Phase 1).
- [x] Doc comments updated to "this crate is web-only".
- [ ] **Still TODO**: Replace `cfg(feature = "web")` with `cfg(target_arch = "wasm32")` for genuinely wasm-only code paths in `leptos_dom::helpers`, `leptos::portal`, `leptos::form`, `leptos::hydration`, `leptos::animated_show`. Add `compile_error!` for impossible cfg combos. (These are still gated on `cfg(feature = "web")` from the Phase 1 sed pass; need to audit which ones are actually wasm-specific vs. web-feature-specific.)

### Phase 3 — Drop `RenderHtml` from `IntoView` on native; delete stub modules (deferred to Phase 5)

Attempted in this pass; reverted because the coupling between `IntoView`, `RenderHtml`, `AsyncOutput`, `AddAnyAttr`, and `to_html_with_buf` permeates the leptos crate beyond just `into_view.rs`:

- `leptos/src/children.rs` (TypedChildren / TypedChildrenMut / TypedChildrenFn) demands `C::AsyncOutput: Send`.
- `leptos/src/attribute_interceptor.rs` (AttributeInterceptorInner) provides a `RenderHtml` impl that requires `T::AsyncOutput`.
- `leptos/src/suspense_component.rs` and `leptos/src/nonce.rs` carry `RenderHtml` impls referencing `AsyncOutput`.
- `tachys::view::add_attr::AddAnyAttr` requires `Output: RenderHtml`, and tens of native-builder impls inside `tachys::{cocoa,gtk,ios}::element` carry `where Self::Output<NewAttr>: RenderHtml` clauses.

Splitting just `IntoView`'s bound (or just `AddAnyAttr`'s) leaves these other call sites broken on native. The right time to do this surgery is alongside Phase 5: when the per-OS builder code moves out into glue crates, those impls move out too, and what remains in tachys is just the trait + the web impls. At that point the cfg gating is straightforward.

Plan when revisited (Phase 5):
- [ ] Split `IntoView`'s bound: `Render + RenderHtml + Send` on web, `Render + Send` on native.
- [ ] Cfg-gate `View<T>`'s `RenderHtml`/`ToTemplate`/`AddAnyAttr` impls behind `feature = "web"`.
- [ ] Cfg-gate the matching impls in `children.rs`/`attribute_interceptor.rs`/`suspense_component.rs`/`nonce.rs` and any `C::AsyncOutput: Send` bounds behind `feature = "web"`.
- [ ] Cfg-split `AddAnyAttr` so `Output: RenderHtml` is web-only.
- [ ] Delete `tachys/src/{cocoa,gtk,ios}/render_html_stub.rs` and the `*_stub_view_impls!` macros.
- [ ] Strip the `RenderHtml` emissions from `impl_typed_attrs_for!` (cocoa/gtk/ios) and ad-hoc `RenderHtml for ...` blocks in `element.rs`/`renderer/*.rs`.
- [ ] Delete native `failed_to_cast_*` stubs in `tachys/src/hydration.rs`.

### Phase 4 — Macro refactor: emit `__leptos_view::*` ✅

- [x] `leptos_macro/src/view/mod.rs` now emits `__leptos_view::elements::tag()`, `__leptos_view::events::on(__leptos_view::events::ev, …)`, `__leptos_view::attrs::*`, `__leptos_view::bind::*`. SVG/MathML/custom now flow through `__leptos_view::elements::*` (one configurable path; the per-renderer `elements` module decides which of svg/mathml names exist). The `bind:group` special-cased path also routes through `__leptos_view::bind::Group` now.
- [x] `leptos/src/view_prelude.rs` provides `__leptos_view` for web (sourced from `tachys::html`/`tachys::svg`/`tachys::mathml`/`tachys::reactive_graph::bind`) and **transitionally** for the three native targets (sourced from `tachys::cocoa`/`tachys::ios`/`tachys::gtk`). Phase 5 moves the native variants out into the leptos_<backend> glue crates and removes them from leptos.
- [x] `leptos::prelude::*` glob-imports `view_prelude::*` → users get `__leptos_view` in scope automatically.
- [x] Internal leptos files that invoke `view!{}` (`animated_show.rs`, `attribute_interceptor.rs`, `await_.rs`, `error_boundary.rs`, `for_loop.rs`, `form.rs`, `hydration/mod.rs`, `provider.rs`, `show_let.rs`, `suspense_component.rs`, `transition.rs`) got `use crate::view_prelude::*;` prepended.

Verified:
- `cargo check --workspace --features web` clean.
- Web examples build (counter, todomvc, router cross-compiled to wasm32).
- Cocoa counter builds.
- iOS counter cross-compiles for the simulator.

### Phase 5 — Move tachys per-OS code out into glue crates (in progress)

#### Phase 5a (done) — delete obsolete facades

- [x] Deleted `tachys/src/html/element_{macos,ios,gtk}.rs` (post-Phase-4, the macro no longer routes through them).
- [x] Deleted `tachys/src/svg_{macos,ios,gtk}.rs`.
- [x] Removed mod declarations from `tachys/src/lib.rs` and `tachys/src/html/mod.rs`.

#### Phase 5b — cocoa glue crate (done)

- [x] Created `~/src/leptos-native/leptos_cocoa/` crate.
- [x] Moved `tachys/src/cocoa/{element,attr,bind,directives,node_ref,window,render_html_stub}.rs` → `leptos_cocoa/src/{elements,attrs,bind,directives,node_ref,window,render_html_stub}.rs`.
- [x] Moved `tachys/src/html/event_macos.rs` → `leptos_cocoa/src/events.rs`.
- [x] Moved `leptos/src/mount_macos.rs` → `leptos_cocoa/src/mount.rs`.
- [x] Updated all internal paths (`crate::cocoa::*` → `crate::*`, `crate::html::*` → `tachys::html::*`, `crate::view::*` → `tachys::view::*`, `$crate::view` → `::tachys::view`, etc.).
- [x] `leptos_cocoa::view_prelude::__leptos_view` provides the macro's namespace.
- [x] `leptos_cocoa` re-exports cocoa_dom's public surface (`Color`, `set_interval`, `local_storage`, etc.).
- [x] Removed cocoa-related references from `leptos/src/lib.rs` (mount_macos module, prelude re-exports, `pub use tachys::cocoa as cocoa`, BindAttribute re-export).
- [x] Removed cocoa from `tachys::lib.rs` (`pub mod cocoa`), `tachys::html::attribute::Selection` re-export, `tachys::html::attribute::value` impls for `crate::cocoa::attr::Dim`.
- [x] Native cocoa examples (most of them) updated to `use leptos_cocoa::*; use leptos_cocoa::view_prelude::*;` and added `leptos_cocoa = { path = "../../leptos_cocoa" }` to their Cargo.toml.

Verified: 5 representative cocoa examples (counter, counters, todomvc, settings, login_form, scroll_view, greeter) build clean end-to-end through the new `leptos_cocoa` crate. block_layout example has a feature-passthrough nit (block_layout feature not yet wired through the new glue crate).

#### Phase 5c — iOS glue crate ✅

- [x] Created `leptos_ios` crate in leptos-native.
- [x] Moved `tachys/src/ios/*` → `leptos_ios/src/*`.
- [x] Moved `tachys/src/html/event_ios.rs` → `leptos_ios/src/events.rs`.
- [x] Moved `leptos/src/mount_ios.rs` → `leptos_ios/src/mount.rs`.
- [x] `leptos_ios::view_prelude` provides `__leptos_view`. Aliased `switch_` as `switch` in `elements` so `<switch>` resolves.
- [x] Removed iOS-specific entries from `leptos/src/lib.rs` (mount_ios, prelude re-exports of `tachys::ios`, `pub use tachys::ios as ios`).
- [x] Removed iOS view_prelude branch from `leptos/src/view_prelude.rs`.
- [x] Removed `pub mod event_ios` and `pub use event_ios as event` from `tachys/src/html/mod.rs`.
- [x] iOS example Cargo.tomls + main.rs updated.

Verified: 5 representative iOS examples (counter, greeter, switch_demo, controls, counters) build via `cargo check --target aarch64-apple-ios-sim`.

#### Phase 5d — GTK glue crate ✅ (build-tested only by syntax check; needs Linux for runtime)

- [x] Created `leptos_gtk` crate in leptos-native.
- [x] Moved `tachys/src/gtk/*` → `leptos_gtk/src/*`.
- [x] Moved `tachys/src/html/event_gtk.rs` → `leptos_gtk/src/events.rs`.
- [x] Moved `leptos/src/mount_gtk.rs` → `leptos_gtk/src/mount.rs`.
- [x] `leptos_gtk::view_prelude` provides `__leptos_view`.
- [x] Removed GTK-specific entries from `leptos/src/lib.rs` (`pub mod mount_gtk`, prelude re-exports of `tachys::gtk`, `pub use tachys::gtk as gtk`).
- [x] Removed GTK view_prelude branch from `leptos/src/view_prelude.rs`.
- [x] Removed `pub mod event_gtk` and `pub use event_gtk as event` from `tachys/src/html/mod.rs`.
- [x] GTK example Cargo.tomls + main.rs updated.

Verified: `cargo check -p leptos_gtk` passes on macOS (the crate body is `#![cfg(target_os = "linux")]` so it compiles to an empty rlib, but its manifest deps and lib.rs declarations resolve cleanly). Full validation requires running on Linux.

#### Phase 5e — drop tachys's remaining native baggage (partial)

The renderer adapters in `tachys/src/renderer/{cocoa,ios,gtk}.rs` stay in tachys for now — they're internal-only (the `Rndr` typealias used by tachys's generic machinery). Moving them out would require a `Renderer`-trait refactor that's out of scope for this pass.

Remaining cleanup that can happen now:

- [ ] Drop the cocoa-specific `IntoAttributeValue` impls in `tachys/src/html/attribute/value.rs` (Color, NSTextAlignment, FlexDirection, etc.). These are tied to the deferred Phase 6 — `into_attribute_value` refactor — and currently leak `cocoa_dom::*` / `ios_dom::*` types into tachys.
- [ ] Once Phase 6 lands, drop the `cocoa_dom`/`gtk_dom`/`ios_dom` optional deps from `tachys/Cargo.toml` and `leptos/Cargo.toml`.

#### Phase 3 fold-in (pending — happens after 5e)

Once tachys no longer carries any per-OS impls, the trait surgery from Phase 3 is straightforward:

- [ ] Split `IntoView`'s bound: `Render + RenderHtml + Send` on web, `Render + Send` on native.
- [ ] Cfg-gate `View<T>`'s `RenderHtml`/`ToTemplate`/`AddAnyAttr` impls behind `feature = "web"`.
- [ ] Cfg-gate the matching impls in `children.rs`/`attribute_interceptor.rs`/`suspense_component.rs`/`nonce.rs` and any `C::AsyncOutput: Send` bounds.
- [ ] Cfg-split `AddAnyAttr` so `Output: RenderHtml` is web-only.
- [ ] Delete `leptos_cocoa/src/render_html_stub.rs` (and ios/gtk equivalents) and the `*_stub_view_impls!` macros.
- [ ] Strip the `RenderHtml` emissions from `impl_typed_attrs_for!` and ad-hoc `RenderHtml for ...` blocks in the moved `elements.rs` files.
- [ ] Delete native `failed_to_cast_*` stubs in `tachys/src/hydration.rs`.

### Phase 6 (deferred) — `into_attribute_value` refactor

Out of scope per user direction.

## Verification (final, 2026-05-06)

All 8 verification points green:

1. `cargo check --workspace --features web` (in leptos-mac) — clean.
2. `cargo check --workspace` (in leptos-native) — clean.
3. wasm web example: `examples/counter` cross-compiles to `wasm32-unknown-unknown` — clean.
4. Larger wasm example: `examples/todomvc` cross-compiles — clean.
5. Cocoa native example: `examples_cocoa/counter` builds.
6. iOS sim example: `examples_ios/counter` cross-compiles to `aarch64-apple-ios-sim`.
7. Mutex check (both features active): `cargo check -p leptos --features web,native-ui` correctly errors with `compile_error!`.
8. Mutex check (no feature): `cargo check -p tachys --no-default-features` correctly errors with `compile_error!`.

Plus 7 cocoa examples and 5 iOS examples spot-checked: all build.
