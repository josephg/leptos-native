# Book follow-up work

Tracks the audit findings from the first review pass. Tag each
item as **CODE** (fix the library to match the docs), **DOCS**
(fix the docs to match the library), **EITHER** (design call),
or **VERIFY** (read source first).

Status markers: `[ ]` open, `[x]` done, `[?]` verifying / blocked.

---

## P0 — factual errors

- [x] **#1** DOCS — `<Show>` fallback documented as missing.
      `common/leptos/src/show.rs:54` exposes the typed fallback.
      Rewrite `view/06_control_flow.md` and delete the
      migration-appendix note.
- [x] **#2** CODE — iOS `<image_view>` `sf_symbol` + `tint`
      added. `content_mode` deferred (UIKit-specific nicety).
- [x] **#3** CODE — iOS `<button>` `sf_symbol` added.
- [x] **#4** CODE — Cocoa colour constants added (YELLOW,
      ORANGE, PURPLE, GRAY, CYAN, MAGENTA, TRANSPARENT).
- [x] **#5** CODE — `<toolbar_item>` `.sf_symbol(name)`
      shorthand added.
- [x] **#6** DOCS — no-op. No `SignalSetter` conflation was
      actually present in the docs.

## P1 — accuracy

- [x] **#7** VERIFIED — `.application(app)` exists at
      `gtk/leptos_gtk/src/gtk/window.rs:34`. Docs correct.
- [x] **#8** VERIFIED — `TypedChildren::into_inner()` returns
      `FnOnce()` / `FnMut()` / `Fn()` depending on the variant.
      The `()()` call pattern in docs is correct.
- [x] **#9** VERIFIED — every iOS value-bearing control exposes
      `on:click` (fires on value change). Slider docs updated to
      match. The naming inconsistency (`on:click` for value
      change) remains as P2#17 / A8.
- [x] **#10** VERIFIED — GTK `<label>` `bind:value` exists in
      `gtk/leptos_gtk/src/gtk/bind.rs`. Docs correct.
- [x] **#11** VERIFIED — GTK `<menu_item>` uses `on:action`
      (matches Cocoa). `ActionEvent` defined at
      `gtk/leptos_gtk/src/event_gtk.rs:46`.
- [ ] **#12** DOCS — Cocoa Button single-handler panic message
      wording: match the actual `panic!()` text.
- [ ] **#13** EITHER/DOCS — GTK `mount_to_window` `(i32, i32)`
      vs Cocoa `(f64, f64)`. Long-term: unified `WindowSize`.

## P2 — polish

- [ ] **#14** DOCS — "macOS" vs "Cocoa" terminology consistency
      pass.
- [ ] **#15** DOCS — example citation format consistency.
- [x] **#16** DOCS — typo: `NSTextDelegate` → `NSTextViewDelegate`
      in `elements/text_view.md`.
- [ ] **#17** CODE — inconsistent value-change events. Expose
      `on:change` on every value-bearing control; reserve
      `on:click` for actual clicks.
- [ ] **#18** DOCS — `Dim` types inconsistently described in
      element-reference vs layout-attributes pages.
- [x] **#19** VERIFIED — `Store::snapshot()` does NOT exist.
      Docs updated to subscribe to fields individually instead.
- [x] **#20** VERIFIED — root view shifts; per-scroll-view
      auto-scroll-to-focused is NOT implemented. Docs updated to
      describe actual behaviour. Real fix tracked separately
      (would need `scrollRectToVisible` + focus observer).
- [x] **#21** DOCS — dead "see below" reference cleaned from
      `elements/image_view.md`. Table now anticipates P0#2.
- [x] **#22** DOCS — `tachys::html::event` import path mentioned
      in `platform/gtk/menus.md`.

## P3 — broader gaps

- [x] **#23** DOCS — multi-error ErrorBoundary snippet added to
      `view/07_errors.md`. (Example crate still wanted: see
      "missing examples" below.)
- [x] **#24** DOCS — components page corrected. `on:click` on a
      component invocation does work when the component's root
      is a leaf — wording softened from "you can't" to "works
      on leaf roots only."
- [ ] **#25** CODE+DOCS — no theming chapter. Cocoa missing
      `Color::SYSTEM_*` enum (iOS has it).
- [ ] **#26** EXAMPLE — `AsyncDerived` end-to-end.
- [ ] **#27** Screenshots — already tracked in
      `SCREENSHOTS_TODO.md`.

---

## Cross-cutting API improvements (all CODE)

### Cargo / dependency UX

- [x] **A1** `gtk` is now a default feature of `leptos_gtk`.
      `--no-default-features` still works for the contributor
      typecheck path. Example crates and docs updated.
- [ ] **A2** `cargo run-ios` extension. Wrap the `run_ios.sh`
      flow into a cargo subcommand: `cargo run-ios -p counter_ios`.
- [ ] **A3** GTK application ID could default from
      `CARGO_PKG_NAME` when caller passes `None`.

### Cross-port symmetry

- [ ] **A4** iOS missing `<color_well>` and `<pop_up_button>`.
      Add UIColorPickerViewController + UIMenu wrappers, even if
      stub-level.
- [ ] **A5** `<switch>` vs `<checkbox>`. Either alias `<checkbox>`
      on iOS to `<switch>`, or expose `<toggle>` everywhere.
- [ ] **A6** `should_quit_on_last_window_close` option for menu-bar
      apps on Cocoa.
- [ ] **A7** Unified `mount()` entry point per port (instead of
      `mount_to_window` / `mount_ios::run` split).

### Layout / widget polish

- [ ] **A8** Standardise on `on:change` for value-bearing
      controls. Same as P2#17.
- [ ] **A9** Reactive `<toolbar>` items (`<For>` inside `<toolbar>`).
- [ ] **A10** `<text_field>` intrinsic-width control — explicit
      opt-in instead of silent default.
- [ ] **A11** Cocoa system-color constants (`Color::LABEL`,
      `SYSTEM_BACKGROUND`, …) for dark-mode-aware UIs.
- [ ] **A12** `<scroll_view>` with unbounded parent: panic at
      build time per the failure-mode hierarchy.
- [ ] **A13** GTK styling attributes (`background_color`, …):
      either translate to GTK CSS or panic at build time.

### Reactive surface (already on the long-term list)

- [ ] **A14** `<Slots>` macro.
- [ ] **A15** `<Transition>` / `<AnimatedShow>`.
- [ ] **A16** `<label>` should accept `Result<String, _>` (and
      flow errors to boundary), not just `String`.

---

## Missing examples (separate work)

P0 (would directly support a chapter):

- [x] `cocoa/examples/show_fallback` — `<Show fallback=...>`.
      Needs `_marker=PhantomData::<()>` workaround; that's
      noted in docs and in the source comment of `show.rs`.
- [x] `cocoa/examples/dark_mode` — manual palette swap via
      reactive `background_color` / `text_color`. Uses the new
      colour constants from P0#4.
- [x] `cocoa/examples/async_derived` — `AsyncDerived` driving
      a `<label>`, with a tokio runtime for `sleep`.

P1:

- [x] `cocoa/examples/bind_tuple_form` — trim + lowercase email
      via `bind:value=(getter, setter)`.
- [x] `uikit/examples/keyboard_avoidance` — long form in a
      `<scroll_view>` demonstrating safe-area + keyboard insets.
- [x] `cocoa/examples/multi_error_boundary` — two parsings, one
      boundary; both errors render together.
- [x] `gtk/examples/persistent_settings` — JSON file persistence
      (renamed from `gsettings_persistence` since real GSettings
      needs schema infrastructure that's a separate concern).

P2:

- [ ] `cocoa/examples/tabs_via_segmented` — segmented driving
      Switch/Match.
- [ ] `cocoa/examples/node_ref_focus` — focus on mount.
- [ ] `cocoa/examples/window_handle_close` — programmatic close.
- [ ] `uikit/examples/image_view_basic` — bundled PNG.
- [ ] `cocoa/examples/grid_responsive` — reactive `columns=`.

P3 (additions to existing examples):

- [ ] `counters` — empty-state `<Switch>` branch.
- [ ] `showcase` — every documented element appears somewhere.
- [ ] Unify the three `counter` examples so the "same source
      across platforms" claim is literally true.

---

## Execution order

1. **Doc-only patches** — DONE
2. **Easy code wins (P0/A1)** — DONE
3. **Verify P1** — DONE
4. **Examples** — DONE
5. **API symmetry phase** — decisions made (see below), executing.

---

# API design decisions (session 2)

All decisions captured from user. Order of execution within each
phase chosen for logical grouping + low conflict.

## Phase A — Quick additive code wins ✅

- [x] **S4** Cocoa `Color` converted to enum mirroring iOS:
      `Rgba { r, g, b, a }` + `System(SystemColor)`. Constants
      added for parity (LABEL, SECONDARY_LABEL, SYSTEM_BACKGROUND,
      SYSTEM_RED/BLUE/etc., CONTROL_BACKGROUND, etc.). Showcase
      pattern-match updated; AppKit's dynamic NSColors handle
      light/dark mode automatically.
- [x] **R1** Removed `<Show>`'s unused `F` generic. Workaround
      removed from book and example.
- [x] **E1** `set_quit_on_last_window_close(bool)` added to
      `cocoa_dom::app`, re-exported via leptos prelude.
      Documented in `platform/cocoa/windows.md`.
- [x] **E3** GTK `application_id` is now
      `impl Into<Option<&'static str>>`; `None` resolves to
      `local.cargo.<CARGO_PKG_NAME>` (with character-safe
      coercion).
- [x] **R4** `<label>.try_text(closure)` added on Cocoa.
      Per-port duplication for GTK/iOS deferred to phase D as
      part of the cross-port symmetry pass.

## Phase B — Doc polish ✅

- [x] **NEW** Default column added to every element-reference
      attribute table plus the shared layout-attributes page.
      Read defaults from the per-port builder constructors and
      Default impls.
- [x] **N2** Light terminology pass; book is already largely
      consistent (macOS in headings, Cocoa in tables). Deeper
      sweep deferred until other phases settle.

## Phase C — Mechanical renames ✅

- [x] **N1** Done across all three ports. `ChangeEvent` is now
      `()` payload, universal for value-bearing controls
      (slider, stepper, segmented, date, colour well, popup,
      plus text field's `on:change` as a unit-payload fan-out).
      Text field's old `on:change(String)` migrated to new
      `on:commit(String)`. Macro's known-events list gained
      `"commit"`. Per-port `on_value_change` added to dom layer
      (routes to text-field delegate or NSControl/UIControl
      target/action as appropriate).
- [x] **S3** GTK `bind:selection` removed; `bind:value` now used
      for PopUpButton on both Cocoa and GTK. Examples and docs
      updated.
- [x] **R3b** No-op — `LocalResource` doesn't exist in this
      codebase yet. When the async-resource type is eventually
      built (under R3 / Phase F), it'll be named `Resource`
      from the start.

## Phase D — Cross-port symmetry ✅

- [x] **S2** `<toggle>` added on every port as the portable
      name for the boolean toggle widget. `<checkbox>` (Cocoa/
      GTK) and `<switch>` (iOS) remain as native aliases.
- [x] **S5** `<view>` removed from GTK and iOS macro facades.
      `<stack>` is now the portable no-direction container; iOS
      grew a `stack()` builder as an alias for the internal
      `view()`.
- [x] **L1** Shared `WindowSize` / `WindowPosition` newtypes in
      `common/renderer/src/window.rs`. `From<(i32, i32)>`,
      `From<(f64, f64)>`, `From<(u32, u32)>`. Cocoa re-exports
      them and the per-port mount/builders accept any
      `Into<WindowSize>`.
- [x] **E2** `mount(view_fn)` exposed on every port. Cocoa:
      defaults to "App" 640×480. GTK: same + auto application
      ID. iOS: alias for `run`. `mount_to_window` / `run` /
      `mount_to_split_window` remain for explicit control.

## Phase E — Tooling restructure ✅

- [x] **T2** iOS examples moved to their own inner workspace at
      `uikit/examples/`. `uikit/leptos_uikit` and `uikit/dom`
      stay in the top-level workspace (so `cargo check
      --workspace` still verifies them against `common/*`
      changes). Inner workspace defaults to
      `aarch64-apple-ios-sim` target and shares parent
      `target/` via `.cargo/config.toml`.
- [x] **T1** Per-example `run_ios.sh` scripts now 3-line shims
      that call `uikit/tools/run_ios.sh <example_dir>`. The
      shared script auto-derives package name (`<dir>_ios`),
      display name (PascalCase of dir), and bundle ID
      (`com.example.<dir>`).

## Phase F — Substantial new work

- [x] **R2** Slots / AnyView landed. `AnyView<R>` in
      `common/renderer/src/view/any_view.rs`; per-port
      `AnyView` + `ChildrenFn` aliases; `IntoAny::into_any()`
      extension trait. The previously-broken `slots_cocoa`
      example now builds and is back in the workspace.
- [x] **S1** iOS `<pop_up_button>` (UIButton + UIMenu, iOS 14+)
      and `<color_well>` (UIColorWell, iOS 14+) both wired up.
      `bind:value` works on both with the same API as Cocoa.
      Docs (element pages + iOS deltas) updated.
- [x] **P3** GTK port: new `gtk_dom::Color` shim type +
      `gtk::decoration::WithDecoration` trait providing
      warn-and-discard setters for `background_color` /
      `corner_radius` / `border_*` / `clip`. Trait impl'd on
      every GTK builder. First call per process logs an
      `eprintln!` explaining that GTK styling is meant to go
      through `gtk::CssProvider`. Portable user code now
      compiles cleanly across all three ports.
- [x] **L2** CLAUDE.md's failure-mode hierarchy refined with
      explicit "panic vs warn-and-degrade" criteria + four
      concrete examples (compile error via type system, mount-
      time panic for double on:click, warn-and-degrade for
      unbounded scroll_view parent, AddAnyAttr panic for
      branching wrappers).

## Phase G — Final polish ✅

- [x] Final mdbook build + workspace check (top-level + inner
      uikit workspace) all green.
- [x] CLAUDE.md updated: GTK no longer requires explicit
      feature flag, iOS examples now in inner workspace,
      `uikit/tools/run_ios.sh` shared script.
- [x] README_gtk.md / README_ios.md updated: drop `features =
      ["gtk"]` and `--features gtk`; reflect inner iOS
      workspace; point at the shared run_ios.sh shim.
- [x] common/leptos/src/lib.rs module docs updated: AnyView
      now described as "used sparingly" with concrete examples,
      not "doesn't exist"; Slots/Show fallback notes refreshed;
      For is keyed.
- [x] migration appendix in book: removed "no AnyView" claim,
      added concrete example of `into_any()`; removed stale
      Slots line in "not yet implemented" list.
- [x] Layout safety net mirrored to Cocoa: `cocoa_dom::layout`
      emits a once-per-process warning when a `<scroll_view>`
      ends up with zero-height viewport but non-empty content
      (mirrors the iOS warning landed earlier).

### Landed since the initial Phase G

- **P2** `<text_field>` `intrinsic_width` enum + builder method
  on Cocoa. Plumbs through `cocoa_dom::layout` via a
  thread-local NSView-pointer set. Default stays `FromParent`;
  `FromContent` opt-in restores natural NSTextField sizing.
- **`<Transition>` + `LocalResource` + `Suspend`** — minimal
  but working implementation. `LocalResource<T>` wraps
  `AsyncDerived<T, LocalStorage>` with relaxed `Send` bounds.
  `Suspend<F>` renders a future as a view (placeholder until
  ready, mounted state after). `<Transition>` is currently a
  passthrough; coordinated cross-suspend "shared loading"
  fallback is a future enhancement. The previously-broken
  `transition_cocoa` example builds and is back in the
  workspace.
- **P1** Reactive `<toolbar>` items via `ToolbarHandle::set_items`
  + `current_identifiers`. Imperative declarative-shaped API
  driven from an `Effect`. The macro-level `<For>` inside
  `<toolbar>` is still a future enhancement, but the new
  helpers let users get equivalent declarative behaviour via
  `Effect::new + toolbar.set_items(...)`.

### Still deferred

- **`<AnimatedShow>`** — needs CoreAnimation design thought.
- **`<For>` directly inside `<toolbar>`** — would need a
  ToolbarMountable-shaped reactive iteration adapter. The
  imperative `set_items` API covers the use case for now.
- **`<Transition>` cross-suspend coordination** — share a
  loading state across multiple `Suspend`s in a subtree so a
  single fallback can cover the whole region.
- **GTK widget parity** — `<scroll_view>`, `<image_view>`,
      `<progress_indicator>`, etc. The user marked these as
      "all eventually, low priority — do at the end."
