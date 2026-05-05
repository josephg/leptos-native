# iOS port — outstanding work

Priority-ordered. Top of each section is the next concrete unit of
work; lower items are more speculative or larger in scope.

References:
- [`audit_ios.md`](./audit_ios.md) — original audit + status table
- [`implementation_ios.md`](./implementation_ios.md) — design log
- [`tasks_ios.md`](./tasks_ios.md) — original stage-by-stage plan
- [`tachys/src/cocoa/element.rs`](./tachys/src/cocoa/element.rs) — the
  reference port; iOS builders mirror its shape
- [`tachys/src/cocoa/bind.rs`](./tachys/src/cocoa/bind.rs) — bind impls
  reference

---

## ~~P1 — Finish the §1c builder port~~ ✅ DONE

All builders ported: `Stepper`, `SegmentedControl`, `DatePicker`,
`ProgressIndicator` (UIProgressView under the hood, named for cocoa
parity), `ImageView`, `ScrollView`, `TextView` — plus TextField /
SecureTextField / Switch / Slider / Button / Label / View from
earlier passes. `bind:value` works for TextField/Slider/Stepper/
DatePicker/TextView/Label, `bind:checked` for Switch,
`bind:selection` for SegmentedControl.

Examples added: `examples_ios/controls` exercises every builder.

Skipped (no native UIKit equivalent):
- **PopUpButton** — iOS uses `UIMenu` on a button or
  `UIPickerView`, neither is a 1:1 with `NSPopUpButton`. Could
  build a `<menu_button>` wrapper later.
- **ColorWell** — iOS has `UIColorPickerViewController` (a full
  modal sheet, not an inline well). Different UX.

---

## ~~P2 — Event routing for non-text controls~~ — match-cocoa, no work

Audit §2 was harsher than warranted. Cocoa has the same
`Change/Focus/Blur` text-field-only routing — the convention is
that non-text controls use `on:click` (which now correctly fans out
to `valueChanged` for any UIControl after the recent fix) or
`bind:` for value-change semantics. Source-compatible with cocoa.

If a future use case demands typed events per control, audit §2
sketches the design space. Until then: status quo.

---

## P3 — iOS essentials any real app needs

### ~~Keyboard avoidance~~ ✅ DONE
`RootViewController::viewDidLayoutSubviews` now reads
`view.keyboardLayoutGuide().layoutFrame()` on every layout pass and
adds the keyboard's intrusion (over and above the safe-area
bottom) to the content root's Taffy padding. UIKit fires
`viewDidLayoutSubviews` automatically when the keyboard shows /
hides, so the relayout cascade is hands-off. Verify with the
`greeter` example: focus the text field, watch the label stay
above the keyboard.

### ~~Modern scene delegate (audit §3a + §6a + §6d)~~ ✅ DONE
- `UIApplicationSceneManifest` declared in Info.plist
  (no static `UISceneConfigurations` — config is programmatic).
- `AppDelegate` is now slim: spawner init +
  `application:configurationForConnectingSceneSession:options:` that
  hands UIKit a programmatic `UISceneConfiguration` naming the
  `SceneDelegate` class.
- `SceneDelegate` implements `UIWindowSceneDelegate` /
  `UISceneDelegate`; its `scene:willConnectToSession:options:` does
  what `didFinishLaunchingWithOptions` used to: alloc the UIWindow
  via `init(windowScene:)`, set up content_root + Taffy tree +
  `RootViewController`, run the stored view-builder closure,
  `makeKeyAndVisible`.
- Deprecated `UIWindow::initWithFrame` + `UIScreen::mainScreen`
  calls and their `#[allow(deprecated)]` guards are gone.
- Required Cargo features added: `UIWindowScene`, `UIScene`,
  `UISceneSession`, `UISceneConfiguration`, `UISceneOptions`.

### ~~Run script polish (§6b, §6c, §6d)~~ ✅ DONE
- `run_ios.sh` (all five examples) now terminates any prior
  instance before install/launch.
- Empty `[build-dependencies]` section dropped from
  `examples_ios/counter/Cargo.toml`.
- Info.plist now declares `MinimumOSVersion=15.0` and (critically)
  `UILaunchScreen=<dict/>` — without it iOS runs every app in
  legacy 320×480 compatibility scaling, which letterboxes the UI
  into a centered card on modern devices.

---

## P4 — Documentation & examples

### Docs
- [x] **`README_ios.md`** — user-facing overview matching
  `README_gtk.md` / `README_macos.md`. ✅
- [x] **`CLAUDE.md`** updates — iOS port section added: build
  commands, architecture summary, conventions vs cocoa, gotchas
  (the `define_class!`-mangled-class-name + `[alloc init]`
  ivar trap, the `<switch>` SVG-list raw-identifier dance, the
  `keyboardLayoutGuide` unresolved-frame guard). ✅
- [x] **`implementation_ios.md`** — keyboard guard, builder
  cocoa-style port, `<switch>` raw-ident, define_class init
  entries added. ✅
- [ ] **`tests_ios.md`** — port `tests.md`'s test plan to iOS
  (`XCUITest` harness shape; deferred work but worth a written
  plan).

### Examples (mirror `examples_cocoa/`)
- [x] **counter** — basic counter. ✅
- [x] **greeter** — text field bind:value, reactive label
  echoes input. ✅
- [x] **switch_demo** — switch + slider together. ✅
- [x] **controls** — full showcase: every supported control
  inside a scroll_view. ✅
- [x] **counters** — dynamic list with `<For>`, add/remove —
  exercises Mountable::unmount + mount_before paths. ✅
- [x] **checkbox** — `<switch>` + text field with on:input/
  on:change/bind:value all coexisting on one field. ✅
- [x] **login_form** — text field + secure text field +
  bind:value + Memo-gated submit button. Exercises keyboard
  avoidance. ✅
- [x] **settings** — slider/switch/segmented_control with
  derived `enabled=` (mute disables slider). ✅
- [x] **timer** — `set_interval_with_handle` + a `use_interval`
  hook that re-schedules when its interval signal changes.
  Stepper drives the interval (instant + iOS-native, vs
  text-input where dismissing the keyboard is awkward). ✅
- [x] **todomvc** — full TodoMVC. `<For>` keyed iteration,
  per-row mount/unmount cycles, persistence via `local_storage`
  (NSUserDefaults), `node_ref` + `on_load` autofocus. iOS UX
  deltas vs cocoa: explicit "+" button instead of Return-to-add
  (no `on:keydown` yet), `<switch>` instead of `<checkbox>`,
  commit-on-blur (no Escape-to-cancel). ✅

---

## P5 — Beyond v1

### Hardware keyboard support (§4j tail; original Stage 8)
- [ ] Wire `pressesBegan:` / `pressesEnded:` on a custom
  `UIResponder` subclass. Translate `UIPress` → `KeyEvent`.
- [ ] Implement `Element::on_text_keydown` /
  `on_text_keyup` (currently no-op stubs in
  `ios_dom/src/node.rs`).
- [ ] `UIKeyCommand` for menu-style shortcuts (Cmd-S etc.).

### Accessibility
- [ ] Auto-set `accessibilityLabel` on controls from their title
  /value/text. Default behaviour is okay for most cases; this
  would polish it.
- [ ] Dynamic Type — let `font_size` defer to
  `UIFont.preferredFont(forTextStyle:)` when not explicitly set.
  iOS users expect text to scale with their system setting.
- [ ] VoiceOver gestures (rotor, hints) — usually free with
  default UIKit but worth verifying with VoiceOver in the
  simulator.

### Dark mode / appearance reactive updates
- [ ] Subscribe to `traitCollectionDidChange:` on the root view
  controller. Re-run any color-dependent reactive effects.
- [ ] Provide a dark/light-aware `Color` constructor that wraps
  `UIColor.dynamicProvider` so colours adapt automatically.

### Navigation & lists
- [ ] `<navigation>` / `<navigation_view>` builder around
  `UINavigationController`. Push/pop pages.
- [ ] `<tab_view>` builder around `UITabBarController`.
- [ ] `<list>` / `<table>` builder around `UICollectionView` (the
  modern equivalent of UITableView). Big effort — likely a
  separate stage.

### Gestures
- [ ] `on:tap` / `on:long_press` / `on:swipe` / `on:pan` via
  `UIGestureRecognizer`. Currently we only have UIControl
  target/action.

### iPad multi-window
- [ ] `Scene` builder integrated with `UISceneDelegate`. Allows
  programmatic new-scene activation
  (`UIApplication.requestSceneSessionActivation:`).
- [ ] State-restoration support so each scene gets the right
  view tree.

### Tests
- [ ] XCUITest harness — same shape as the macOS plan in
  `tests.md`. Counter / greeter / switch_demo as the first
  three subjects.
- [ ] In-process Rust tests for layout / event-store leak / etc.
  where they can be isolated from UIKit.

---

## P6 — Smaller polish & known issues

- [ ] **Audit §4d** — `<scroll_view>` content view is found via
  `subviews[0]` (`ios_dom/src/node.rs::Element::subview_parent`).
  After the user scrolls, UIKit can insert private indicator
  subviews. They land at the end so we're probably fine, but
  tagging the content view (custom subclass or `setTag(...)`)
  is more robust.
- [ ] **Audit §4i** — `event::keep_target_alive` entries are never
  removed when an `Element` is reused or its action target is
  replaced. Same leak as cocoa (which acknowledges it). Bound,
  not unbounded — tracked.
- [ ] **Audit §5d** — several `Element` setters in
  `ios_dom/src/node.rs` are unreferenced
  (`set_image_view_path`, `set_progress_indeterminate`,
  `set_slider_vertical`, `configure_stepper`, etc.). They get
  wired up as the P1 builders land. After P1 closes, sweep for
  any still-unused.
- [ ] **Workspace-level build** — `cargo build` from the repo
  root errors on `ios_dom` resolution because the leptos lib's
  `cfg(target_os = "ios")` re-exports aren't gated on
  `leptos_native`. Same pre-existing pattern as macOS. Fix by
  changing the cfg in `leptos/src/lib.rs:199-211` to
  `cfg(all(target_os = "ios", leptos_native))` (and matching
  macOS: `cfg(all(target_os = "macos", leptos_native))`).
- [ ] **`Owner` leak in `mount_ios::run`** — same pattern as
  cocoa. Run loop never returns; OS reclaims everything. Fine,
  but document explicitly.
- [ ] **`LocalRwSignal` support** — `mount_ios::run`'s closure
  bound is now `'static` (no `Send`), but reactive_graph's
  storage still wants Send for some signal types. Verify
  `LocalRwSignal<Rc<...>>` works end-to-end.
- [ ] **`set_attribute` cycle protection** — already done in
  cocoa parity (`set_string_attribute` / `set_bool_attribute`
  diff before mutating). Verify it covers the new builders too.
