# Codebase review (post-native pivot)

Sweep of the leptos-mac fork after the recent native-pivot refactor.
Companion to `REFACTOR.md`. Items split into:

1. **Drive-by fixes already applied** in this pass.
2. **Vestigial / dead code** still present — safe deletes the next time
   someone touches the relevant file.
3. **Bugs and suspicious code** that need a real look.
4. **Cleanup / refactor opportunities** larger than a drive-by.
5. **Crate-structure observations** — combine? split? rename?
6. **Test gaps** — see also the per-port `tests_*.md` files (appended).

---

## 1. Drive-by fixes applied in this review

### Session 2

- **DRY apple-port code (REVIEW §4.2)** — extracted the truly-shared
  helpers between `cocoa/leptos_cocoa` and `uikit/leptos_uikit` into a
  new workspace crate `apple_shared`:
  - `attr_keys` module: `AttributeKey` trait + `Value` and `Checked`
    marker structs.
  - `directive` module: `IntoDirective<E, T, P>` trait (now generic
    over the `Element` type), plus the `pack` and `run_all` helpers.
  - The two leptos_* crates' `keys.rs`, `directive.rs`, and inner
    `directives.rs` are now thin re-exporting shims (~10 lines each).
  - The `IntoDirective<T, P>` bound in cocoa's `element.rs` was
    rewritten to `IntoDirective<cocoa_dom::Element, T, P>` at all 13
    call sites (uikit had none — `node_ref` / `directive` plumbing
    isn't wired through there yet).
  - Net: ~50 LOC of duplication removed; future fixes to the
    directive trait shape now happen in one place.
- **Split `common/leptos_macro/src/component.rs` (REVIEW §4.3)** —
  the 1094-line monolith became `component/{mod,prop,docs,util}.rs`
  (413/359/201/196 LOC respectively). `mod.rs` keeps `Model` /
  `DummyModel` and the codegen; `prop.rs` owns the prop-builder
  pipeline; `docs.rs` owns `Docs` / `UnknownAttrs`; `util.rs` owns
  the type-level helpers (`is_option`, `unwrap_option`,
  `convert_from_snake_case`, etc.). External re-exports are
  preserved at `crate::component::*` so `slot.rs` and `lib.rs`
  didn't change.
- **`RenderHtml` cleanup (REVIEW §4.1)** — the supertrait removal
  was already in place (`IntoView<R>` requires only `Render<R> +
  AddAnyAttr<R> + Send`). Cocoa and iOS use proper
  `AddAnyAttr<Dom>` impls, not stubs, so there was nothing to
  delete in those ports. Cleaned up stale doc references in
  `common/leptos/src/{lib,children,error_boundary}.rs` and
  `CLAUDE.md`. (GTK's `render_html_stub.rs` will be retired with
  the rest of the GTK Stage 5 work, per the user's request to
  leave the GTK crate alone for now.)
- **Trace features documented in REFACTOR.md** — `trace-components`
  is intact; `trace-component-props` is currently broken
  (references `leptos_dom`, which doesn't exist in this fork).
  Note added with the steps to fix and a stretch-goal pointer
  toward a `trace-component-render` debug overlay.

### Session 1

- **`Cargo.toml` (workspace root):** removed the vestigial `web-sys`
  dep and the stale `[workspace.metadata.cargo-all-features]
  skip_feature_sets = [["csr","ssr"], ["csr","hydrate"], …]` block.
  None of those features exist in this fork.
- **`common/reactive_graph/Cargo.toml`:** removed the
  `cfg(target_arch = "wasm32")` `web-sys` target dep.
- **`common/reactive_graph/src/lib.rs`:**
  - `log_warning()` no longer has a wasm32 arm; just `eprintln!` (or
    no-op when `tracing` is on).
  - `spawn()` no longer branches on `target_family = "wasm"` — always
    `Executor::spawn`.
  - "TODO remove this" comment dropped — the function is the canonical
    warning hook, not a dev placeholder.
- **`uikit/leptos_uikit/src/lib.rs`:** deleted `pub use
  leptos::serde_json;` — `leptos` does not re-export `serde_json`,
  so this was a dead/broken re-export. The `todomvc` example
  declares `serde_json` directly in its own `Cargo.toml`.
- **`common/leptos_macro`** — removed the `#[island]` and `#[lazy]`
  proc macros entirely:
  - Deleted `lazy.rs`, removed `mod lazy;` from `lib.rs`.
  - Deleted `pub fn island` and `pub fn lazy` macro entry points and
    their docstrings (~250 lines of `lib.rs`).
  - Stripped all island-conditional code paths from `component.rs`:
    `is_lazy`, `island: Option<String>`, `is_island_with_children`,
    `is_island_with_other_props`, `props_serializer`,
    `island_serialize_props` / `island_serialized_props`,
    `hydrate_fn_name`, `with_no_hydration`, the
    `tachys::html::islands::*` wrapper, `wasm_bindgen` /
    `web_sys::HtmlElement` hydrate functions, `prop_serializer_fields`,
    `to_serde_tokens`, the `is_lazy` and `with_island` builder
    methods on `Model`, the `_serialize` derive on prop builders.
  - `prop_builder_fields()` lost its `is_island_with_other_props`
    parameter and its `#[serde(skip)]` injection.
  - `cargo check --workspace` and `cargo test --workspace --no-run`
    (excluding iOS-target crates) are green.

These changes have no user-visible API impact — the `#[island]` and
`#[lazy]` macros were not used by any example, and the only reachable
output of the old `#[component]` macro for `island.is_some() == false`
was identical to what the new code emits.

---

## 2. Vestigial / dead code still present

- **`common/leptos_macro/Cargo.toml`** has features
  `trace-components` and `trace-component-props`. They're referenced
  via `cfg!(feature = "trace-components")` in
  `component.rs`, but the *only* effect is to emit a
  `::leptos::leptos_dom::tracing_props![...]` call — and
  `leptos_dom` doesn't exist in this fork (the `leptos` crate is the
  whole thing now). If anyone enabled the feature, the macro would
  emit a path that doesn't resolve. **Action**: remove both feature
  flags + the `cfg!` branches in `component.rs:259–284`.
- **`common/leptos_macro/src/component.rs:483`** still has a commented
  block `// TODO restore dyn attrs / impl ... DynAttrs { ... }`.
  Either restore or delete — it's been there long enough that "TODO
  restore" reads as "we don't need this anymore."
- **`common/reactive_stores_macro/src/lib.rs:688`** — `// TODO:
  support enums later on`. Genuine feature gap, not vestigial; leave.
- **Doc references to removed traits**: `common/leptos/src/lib.rs:84`
  and `common/renderer/src/view/add_any_attr.rs:18` mention
  `RenderHtml` (removed). Tidy the prose.
- **Workspace `serde_json` dep**: only the two `todomvc` examples use
  it, and they declare it locally (`= "1.0"`). The workspace-level
  declaration is unused. **Action**: remove from
  `[workspace.dependencies]`.
- **`gtk/leptos_gtk` is not a workspace member** but lives in the
  source tree. `gtk/examples/*` Cargo.toml files reference it via
  relative path. Either add it to `workspace.members` or move the
  examples under `gtk/leptos_gtk/examples/`. The current state
  silently bit-rots; `cargo check --workspace` doesn't touch it.
- **`uikit/dom/tests/layout.rs`** doesn't compile under
  `cargo test --workspace`: it lacks a `main()`. Either
  `#![cfg(target_os = "ios")]` it (so default-target `cargo test`
  skips it) or move it under a `#[cfg(target_os = "ios")] mod {}`.

---

## 3. Bugs and suspicious code

(Spotted by Explore agents; not all verified end-to-end.)

- **`common/leptos_macro/src/component.rs:391`** (in the old island
  branch — now deleted) called
  `Owner::current_shared_context().unwrap()`. After the
  island removal in this pass, this code path is gone. *No-op now —
  noted for posterity.*
- **Cocoa / iOS handler-store leak**: `cocoa_dom/src/event.rs`
  `HANDLER_STORE`, `TEXT_FIELD_STORE`, `TEXT_VIEW_STORE` — entries
  removed from `drop_handlers_for`, but `drop_handlers_for` is only
  called from `Mountable::unmount`. If a view is dropped without
  unmount (which happens on window close — currently the `Owner` is
  `mem::forget`'d in `mount_macos.rs::run`), every handler leaks for
  the lifetime of the process. CLAUDE.md acknowledges this as
  Phase 3; flag it for the multi-window cleanup story.
- **`cocoa_dom/src/app.rs:39`**: `std::mem::forget(delegate)` is
  intentional but pairs with the leaks above — once `Owner::run` has
  a real `UnmountHandle`, audit both at once.
- **`cocoa_dom/src/layout.rs:616`**: `.unwrap()` on `constraint_w` is
  guarded by an `is_some()` check on `known.height`, which feels
  fragile if anyone refactors the surrounding match. Rewrite as
  pattern destructure or `if let Some(...)`.
- **iOS `directives` / `node_ref` not wired**:
  `uikit/leptos_uikit/src/ios/element.rs` doesn't expose the same
  `node_ref` / `use:` directive surface as the Cocoa builders. Either
  port it over or document the gap (failure-mode hierarchy says
  "compile error or runtime panic, not silent no-op").
- **`uikit/leptos_uikit` lacks `render_html_stub.rs`** while
  `cocoa/leptos_cocoa` and `gtk/leptos_gtk` have one. Per CLAUDE.md
  these stubs satisfy the `IntoView` supertrait bound for
  `RenderHtml`. If the iOS port compiles today, maybe it doesn't need
  one — but check whether the supertrait bound was already removed
  there. If yes, the cocoa/gtk stubs can also go away (along with the
  `IntoView: RenderHtml` requirement); if no, iOS needs the stub.
- **`cocoa/leptos_cocoa/src/cocoa/element.rs:3644-3666`**
  (`impl_add_any_attr_for_leaf!` macro) duplicates the GTK
  `gtk_stub_view_impls!` pattern but per-builder rather than via a
  single macro. Refactor opportunity — see §4.

---

## 4. Cleanup / refactor opportunities (bigger than drive-by)

1. **Remove the `RenderHtml` supertrait from `IntoView`.** CLAUDE.md
   already calls this out as the long-term plan. Once gone, the
   `cocoa_stub_view_impls!` and `gtk_stub_view_impls!` macros — and
   their per-builder boilerplate — disappear. This is the single
   biggest dead-weight reduction available.

2. **DRY between `cocoa/leptos_cocoa` and `uikit/leptos_uikit`.**
   The two crates are ~10 K and ~4.8 K LOC respectively, with the iOS
   port deliberately mirroring the Cocoa port. Most of `bind.rs`,
   `attr.rs`, `directive.rs`, `node_ref.rs`, `mount.rs`, `keys.rs` is
   identical or near-identical. Options:
   - **Shared `apple_dom` / `apple_leptos` crate** wrapping the parts
     that depend on `objc2` + target/action + Taffy. Risk: leaky
     abstraction over NSView/UIView differences.
   - **Generic over a `ControlBackend` trait** where the only
     differences (NSButton vs UIButton, etc.) live behind associated
     types. Risk: trait soup, slow compile.
   - Given the divergence at the *control* level (NSPopUpButton vs
     UIPickerView, no menu bar, scene-based windows), a hybrid is
     probably right: shared helpers for the bits that really are
     identical (`apply_universal`, `apply_text_attrs`, `BoundValue`
     payloads, key-event encoding), separate builders.

3. **`leptos_macro/src/component.rs` is still 1100+ lines after this
   pass.** Worth splitting into `component/{model.rs, props.rs,
   builder.rs, codegen.rs}` for navigability. Unsexy but useful.

4. **`tachys` is gone but the namespace still echoes everywhere.**
   `leptos::tachys::cocoa::*`, `leptos::tachys::ios::*`,
   `leptos::tachys::view`, etc. — these paths exist for backwards
   compat with the `view!{}` macro's emitted output. Worth a pass to
   rename emit sites + collapse the re-export shim, since `tachys`
   no longer corresponds to a real crate boundary.

5. **`#[derive(reactive_stores::Patch)]`** lives in a separate
   `reactive_stores_macro` crate that's just one file (~870 LOC, but
   one logical macro). Could fold into `reactive_stores` with
   `proc-macro = true`. Caveat: proc-macro crates can't be normal
   library crates, so either move *all* of `reactive_stores` into
   `leptos_macro` (bad) or keep the split (current state).
   **Verdict: leave.**

6. **`common/leptos_macro` and `common/leptos`**: similar split
   reasoning to (5). Leave.

7. **`gtk/leptos_gtk` is unbuilt** in the workspace right now (not in
   `workspace.members`). Either commit to it (add) or remove it
   (the GTK port is documented but barely shipped — Stage 5). Right
   now it's schrödinger-alive.

8. **Examples sprawl.** `cocoa/examples/*` has 22 directories. Each
   has its own `Cargo.toml`. Possibly fine, but consider whether some
   pairs could be merged (`counter` + `counter_without_macros` could
   be one crate with two `[[bin]]` targets, sharing deps).

---

## 5. Crate-structure observations

The current breakdown (modulo the `gtk/leptos_gtk` ambiguity):

```
common/
  reactive_graph        — keep, well-bounded
  reactive_stores       — keep
  reactive_stores_macro — keep (proc-macro split is necessary)
  renderer              — keep (was tachys; the renderer trait + view types)
  leptos_macro          — keep (proc-macro split is necessary)
  leptos                — keep (user-facing API, prelude, components)
{cocoa,uikit,gtk}/
  dom                   — platform-specific NSView/UIView/GtkWidget façade
  leptos_{cocoa,uikit,gtk} — tachys-renderer + macro-facade for that platform
```

This is sane. The main question is whether `dom` and `leptos_*` per
port should merge:

- **Pros of merging** (per port): one crate per platform; the only
  consumers of `cocoa_dom` are `leptos_cocoa` + the `cocoa_dom`
  examples (which mostly demonstrate the low-level API).
- **Cons**: `dom` is a clean "AppKit DSL" you could use without
  Leptos's reactivity (e.g. for a UI-test harness, or hand-rolled
  windows). Worth keeping that boundary intact.

**Recommendation**: keep `dom` / `leptos_*` split for cocoa, ios, and
gtk. Reconsider once `leptos_*` boilerplate is DRY'd (§4.2) — at that
point the layering may want renegotiation.

Things that *could* be removed entirely: `gtk/dom/examples/` files
once `gtk/leptos_gtk` is real; the `nightly` feature on the various
crates if it's not actively used; the `subsecond` feature on
`reactive_graph` (hot-reload) if it's not used in the native ports.

---

## 6. Test gaps

See the additions appended to `tests_macos.md`, `tests_gtk.md`, and
`tests_ios.md` (the latter is partially covered by `audit_ios.md` /
`TODO_ios.md` already). High-leverage gaps:

- **Macro-level**: `compile_fail` tests for `#[component]` — we just
  removed `#[island]` and `#[lazy]`; a regression test that
  `#[island]` fails to compile (with a clear error or a "no such
  attribute" message) is cheap and catches accidental restore.
- **Cross-port parity**: a test matrix that exercises the same view
  graph against cocoa, ios, and gtk and asserts identical layout
  outputs (where applicable). Today regressions in iOS-only or
  GTK-only paths are caught only by example smoke-running.
- **Handler-store leak detection**: spin up + tear down 1000
  buttons, assert that `HANDLER_STORE.with(|s| s.borrow().len())` is
  back near zero. Today nothing exercises the cleanup path.
- **Two-handler-on-one-control panic**: cocoa has the documented
  build-time panic; assert with a `#[should_panic]` test.
- **`set_attribute` no-op-on-equal**: test that setting an attribute
  to its current value doesn't trigger a relayout (matters for the
  bind-cycle protection CLAUDE.md calls out).
- **Cocoa `<scroll_view>` parent-bounding**: today's "must wrap in
  `flex_grow=1.0`" gotcha is documented in CLAUDE.md but untested.
- **`TextView` delegate ↔ `TextField` delegate** correctness: their
  fan-out behavior is subtle; both ports could regress invisibly.
- **Owner cleanup on window close** (multi-window): two-windows
  example covers manual click-through; no automated coverage.
- **Spawner main-thread enforcement**: panic tests that spawning
  off-main fails loudly.

---

## TL;DR action items, prioritized

1. ✅ Removed `#[island]`, `#[lazy]`, vestigial wasm/web-sys deps,
   broken `serde_json` re-export, stale cargo-all-features block.
2. **Decide what to do with `gtk/leptos_gtk`** — add to workspace or
   delete. Right now nothing checks it.
3. **Remove `RenderHtml` from `IntoView`'s supertrait list** —
   biggest single dead-weight reduction. Lets ~3 stub modules go.
4. **Remove `trace-components` / `trace-component-props` features**
   from `leptos_macro` (broken — references nonexistent `leptos_dom`).
5. **Drop the `tachys::*` re-export shim names** once `IntoView`
   refactor lands — they're vestigial naming.
6. **DRY apple-port code** between `cocoa/leptos_cocoa` and
   `uikit/leptos_uikit` (a shared helper module is enough; no need
   for full crate sharing).
7. **iOS port**: add `node_ref` + `directive` surface, write basic
   tests (currently 1 file vs cocoa's 9).
8. **Ergonomic**: split `component.rs` into smaller modules; tidy
   stale doc references to `RenderHtml`.

