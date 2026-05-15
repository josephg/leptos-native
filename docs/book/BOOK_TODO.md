# Book follow-up work

Tracks the audit findings from the first review pass. Tag each
item as **CODE** (fix the library to match the docs), **DOCS**
(fix the docs to match the library), **EITHER** (design call),
or **VERIFY** (read source first).

Status markers: `[ ]` open, `[x]` done, `[?]` verifying / blocked.

---

## P0 — factual errors

- [ ] **#1** DOCS — `<Show>` fallback documented as missing.
      `common/leptos/src/show.rs:54` exposes the typed fallback.
      Rewrite `view/06_control_flow.md` and delete the
      migration-appendix note.
- [ ] **#2** CODE — iOS `<image_view>` only has `source`.
      Add `sf_symbol`, `tint` (and `content_mode` if cheap).
- [ ] **#3** CODE — iOS `<button>` has no `sf_symbol`.
      Add method with same signature as Cocoa's.
- [ ] **#4** CODE — `Color::YELLOW` missing on Cocoa.
      Add `YELLOW`, `ORANGE`, `PURPLE`, `GRAY`, `CYAN`,
      `MAGENTA` to `cocoa/dom/src/color.rs` for parity with iOS.
- [ ] **#5** CODE — `<toolbar_item>` lacks `sf_symbol=` shorthand.
      Add `.sf_symbol(name)` that wraps to `Icon::SfSymbol`.
- [ ] **#6** DOCS — `signal()` returns `(ReadSignal, WriteSignal)`,
      not `(Signal, SignalSetter)`. Scrub conflation.

## P1 — accuracy

- [ ] **#7** VERIFY — `.application(app)` on GTK window builder.
      Read `gtk/leptos_gtk/src/gtk/window.rs`.
- [ ] **#8** VERIFY — `TypedChildren::into_inner()()`. If awkward,
      add a `.render()` helper (CODE); otherwise clarify (DOCS).
- [ ] **#9** VERIFY → likely CODE — iOS controls' `on:click`.
      Audit which exposes which event. Standardise on
      `on:change` for value-bearing controls.
- [ ] **#10** VERIFY — GTK `<label>` `bind:value`.
      `gtk/leptos_gtk/src/gtk/bind.rs`.
- [ ] **#11** VERIFY — GTK `<menu_item>` event name.
- [ ] **#12** DOCS — Cocoa Button single-handler panic message
      wording: match the actual `panic!()` text.
- [ ] **#13** EITHER/DOCS — GTK `mount_to_window` `(i32, i32)`
      vs Cocoa `(f64, f64)`. Long-term: unified `WindowSize`.

## P2 — polish

- [ ] **#14** DOCS — "macOS" vs "Cocoa" terminology consistency
      pass.
- [ ] **#15** DOCS — example citation format consistency.
- [ ] **#16** DOCS — typo: `NSTextDelegate` → `NSTextViewDelegate`
      in `elements/text_view.md`.
- [ ] **#17** CODE — inconsistent value-change events. Expose
      `on:change` on every value-bearing control; reserve
      `on:click` for actual clicks.
- [ ] **#18** DOCS — `Dim` types inconsistently described in
      element-reference vs layout-attributes pages.
- [ ] **#19** VERIFY — `state.snapshot()` on `Store<T>`.
- [ ] **#20** VERIFY → CODE — iOS scroll-to-focused-on-keyboard.
      `UIScrollView.scrollRectToVisible` wiring.
- [ ] **#21** DOCS — dead "see below" reference in
      `elements/image_view.md` (resolves with P0#2).
- [ ] **#22** DOCS — mention `tachys::html::event` import path
      in `platform/gtk/menus.md`.

## P3 — broader gaps

- [ ] **#23** DOCS/EXAMPLE — multi-error ErrorBoundary case.
- [ ] **#24** DOCS — attribute spread `{..props}` — show or drop.
- [ ] **#25** CODE+DOCS — no theming chapter. Cocoa missing
      `Color::SYSTEM_*` enum (iOS has it).
- [ ] **#26** EXAMPLE — `AsyncDerived` end-to-end.
- [ ] **#27** Screenshots — already tracked in
      `SCREENSHOTS_TODO.md`.

---

## Cross-cutting API improvements (all CODE)

### Cargo / dependency UX

- [ ] **A1** GTK requires `features = ["gtk"]`. Make `gtk` a
      default feature of `leptos_gtk`. Keep `--no-default-features`
      working for the renderer-agnostic typecheck mode.
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

- [ ] `cocoa/examples/show_fallback` — `<Show fallback=...>`.
- [ ] `cocoa/examples/dark_mode` — `Color::SYSTEM_*` + decoration.
- [ ] `cocoa/examples/async_derived` — `AsyncDerived` driving UI.

P1:

- [ ] `cocoa/examples/bind_tuple_form` — `bind:value=(getter, setter)`.
- [ ] `uikit/examples/keyboard_avoidance` — long form, keyboard
      pushes content.
- [ ] `cocoa/examples/multi_error_boundary` — two errors at once.
- [ ] `gtk/examples/gsettings_persistence` — real `gio::Settings`
      round-trip.

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

1. **Doc-only patches** — P0#1, P0#6, P2#14–16, P2#21–22,
   P3#23–24.
2. **Easy code wins** — A1, P0#4, P0#5, P0#3, P0#2.
3. **Verify P1** — #7, #8, #9, #10, #11, #19, #20.
4. **Examples** — P0 examples first (show_fallback, dark_mode,
   async_derived), then P1.
5. **API symmetry** discussion — A4–A13 are real design calls.
