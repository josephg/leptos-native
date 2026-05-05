# Audit: iOS / UIKit port

> **Status:** §1c (all builders) complete. `bind:value`,
> `bind:checked`, `bind:selection` all work. NodeRef is reactive.
> Audit-fixed items: 1a, 1b, 3b, 3c, 3d, 4a, 4b, 4f, 4g, 4j, 4k,
> 4l, 5b, 5c, 5g, 6b, 6c, 6d and the Button/Label `child` routing
> bug. Keyboard avoidance done via `keyboardLayoutGuide`.
> **Letterboxing**: Info.plist now declares `UILaunchScreen=<dict/>`
> — without it iOS runs apps at 320×480 compatibility scaling.
> Examples shipped: counter, greeter, switch_demo, controls,
> counters, checkbox, login_form, settings, timer, todomvc — all
> 10 of the cocoa examples that have iOS analogues. README_ios.md, CLAUDE.md and implementation_ios
> all updated. §2 closed as match-cocoa behaviour. Still pending:
> 3a (modern scene delegate), todomvc port, hardware keyboard /
> accessibility / dark mode (P5 in TODO_ios.md).

I built the counter example (compiles cleanly with warnings), then ran it on
the booted simulator. It crashes immediately at launch with:

```
*** Terminating app due to uncaught exception 'NSInternalInconsistencyException',
reason: 'Unable to instantiate the UIApplication delegate instance.
No class named AppDelegate is loaded.'
```

Below: the crash root cause, then the broader design issues. The agent built
the scaffolding for stages 1–6 but cut a lot of corners in the tachys layer;
several controls advertised in `tasks_ios.md` are unimplemented stubs.

---

## 1. Critical — what's making it crash

### 1a. `UIApplicationMain` can't find `AppDelegate` by name
`ios_dom/src/app.rs:160` passes the literal string `"AppDelegate"` as
`delegate_class_name`. But `objc2`'s `define_class!` macro registers classes
under a *mangled* name (something like `ios_dom_app_AppDelegate$$...`), not
the Rust struct name. The runtime lookup that `UIApplicationMain` does
therefore fails before any of our code runs.

**Fix:** pass the real registered name.
```rust
use objc2::ClassType;
let cls_name_cstr = AppDelegate::class().name();
let delegate_name = NSString::from_str(&cls_name_cstr.to_string_lossy());
```
The "force registration" stunt at `app.rs:145-149` (`AppDelegate::new` +
`mem::forget`) is necessary for this — keep it, but use the looked-up name.

### 1b. `mount_ios::run` requires `Send` on the user closure
`leptos/src/mount_ios.rs:30` constrains `F: FnOnce() -> V + Send + 'static`.
The closure executes on the main thread inside
`application:didFinishLaunchingWithOptions:`, exactly like cocoa. The cocoa
equivalent has `F: FnOnce() -> V + 'static` with no `Send`. Drop the bound;
otherwise users can't capture `Rc`/non-Send signals (an `RwSignal<i32>`
happens to be `Send`, but a `LocalRwSignal` won't be, and any
scope-captured `Rc` will fail).

### 1c. `tachys/src/ios/element.rs` is missing most builders — the example only happens to compile because it uses `vstack`/`hstack`/`label`/`button` exclusively
- Line 358: `pub use label as text_field;` — `<text_field>` builds a
  `UILabel`, not `UITextField`. The DOM layer creates a real text field
  from `Element::create("text_field")`, but no view-tree code ever takes
  that path because nothing routes through `text_field()`.
- Line 359: `pub fn switch_() -> View<(), ()> { view() }` — `<switch>`
  builds a generic UIView container, not a `UISwitch`.
- No builders at all for: `Slider`, `Switch` (real one), `TextField`,
  `SecureTextField`, `DatePicker`, `Stepper`, `ProgressView`, `ImageView`,
  `SegmentedControl`, `ScrollView`, `TextView`. Compare to
  `tachys/src/cocoa/element.rs` which has all of them.

These have to be ported one-for-one from `tachys/src/cocoa/element.rs`
(~3300 lines). Until that's done, the only working tags are
`view`/`vstack`/`hstack`/`label`/`button`.

### 1d. `tachys/src/ios/bind.rs` is a stub (23 lines vs cocoa's 512)
Only contains a marker `BindAttribute` trait; no `BindAttribute<Value, Sig>`
impls for any control, no `IntoSignal` impls for `RwSignal`/`(Get, Set)`
tuples. `bind:value=signal` and `bind:checked=signal` won't typecheck
against any real builder. Not exercised by the counter example, but
advertised throughout the plan and re-exported in
`leptos/src/lib.rs:262`.

---

## 2. Event routing is wrong for non-text controls

`tachys/src/html/event_ios.rs:136-138` — the `PendingHandler::apply_to`
routes:
- `Change` → `el.on_text_end_editing(cb)`
- `Focus` → `el.on_text_focus(cb)`
- `Blur` → `el.on_text_blur(cb)`

All three implementations in `ios_dom/src/node.rs:666-693` are guarded by
`if let Some(field) = downcast::<UITextField>(...)` — so `on:change` on a
`UISwitch`, `UISlider`, `UISegmentedControl`, `UIDatePicker`, or `UIStepper`
is silently dropped. That's everywhere `on:change` would normally fire
`ValueChanged` on macOS. The dispatcher needs a control-type fallback to
`on_action` (which already wires `ValueChanged`).

The `Change` event also unconditionally hands a `String` payload, which
makes no sense for a switch (bool) or slider (f64). Either generalise
`EventType` per control (the typed approach the cocoa port uses) or expose
a separate `Action`/`ValueChanged` event whose payload is `()`.

---

## 3. Window / scene model

### 3a. Deprecated `UIWindow::initWithFrame`
`app.rs:75-79` and `window.rs:31-35` use
`UIWindow::initWithFrame(UIScreen::mainScreen(...).bounds())`. Both are
deprecated; the modern path is a `UIWindowScene` with
`UIWindow::initWithWindowScene:`. iOS still accepts the deprecated path on
iOS 15/16/17/18, so this is not the cause of the crash, but it does mean:
- The app doesn't get a proper scene attachment, which breaks size-class
  transitions, multi-window on iPad, and several iOS 17+ layout
  guarantees.
- Deprecation warnings will eventually become errors.

The right shape is a `UISceneDelegate` registered via Info.plist
`UIApplicationSceneManifest` (or programmatically with
`UISceneConfiguration`), which receives
`scene:willConnectToSession:options:` and creates the window from the
connecting scene. The Info.plist `run_ios.sh` writes is missing the scene
manifest.

### 3b. `window.rs` is dead code
`ios_dom::window::open_window` and `OpenedWindow` are defined but never
called — `app.rs::did_finish_launching` does its own window setup inline.
Pick one path.

### 3c. Safe area not applied
Decision 7 in `implementation_ios.md` says safe-area insets get applied as
padding on the content root. Nowhere in the code does this happen — the
content view sits at `(0,0,fullScreenW,fullScreenH)` and content lands
under the status bar / notch / home indicator.

### 3d. No `viewDidLayoutSubviews` / rotation handling
`compute_layout` runs once at launch (`mount_ios.rs:46`). After that, only
style mutations re-trigger layout via `schedule_relayout`. Device
rotation, keyboard appearance, split-view resize on iPad — none of those
re-run layout. Need a custom `UIViewController` that overrides
`viewDidLayoutSubviews` (or
`viewWillTransitionToSize:withTransitionCoordinator:`) to call
`compute_layout` with the new size.

---

## 4. Smaller correctness issues

### 4a. `set_font_size` calls `schedule_relayout` for `UITextView` only
`ios_dom/src/node.rs:874` — the `schedule_relayout` line is inside the
function but at the bottom, after all the `return`s. Each branch returns
early, so the relayout is only scheduled for `UITextView` (and only as a
side-effect of falling through to the bottom). Move the call to before
the early returns or to every branch.

### 4b. `set_autohides_scrollers` does the wrong thing
`node.rs:946-953` toggles both indicators based on the parameter — but
the macOS analogue toggles whether they auto-hide vs always-show. iOS's
actual analogue would be `flashScrollIndicators` and the indicator-style
settings; a simpler matching behaviour is
`setShowsVerticalScrollIndicator(!autohides)` etc. Either way the
current implementation fights the more specific
`set_has_horizontal_scroller` / `set_has_vertical_scroller` calls.

### 4c. `clear_children` doesn't drop layout/handler entries
`node.rs:533-540` calls `removeFromSuperview()` on each subview but
doesn't call `crate::layout::drop_node` or
`crate::event::drop_handlers_for`. Children leak Taffy entries and any
retained event targets. Compare with `Node::teardown` (which does both).

### 4d. `<scroll_view>` content view is fragile
`node.rs:464-479` `subview_parent` finds the content view as the *first
subview* of the `UIScrollView`. After scrolling, UIKit can insert its own
scroll-indicator subviews (`_UIScrollViewScrollIndicator`) into the
subview list. They're documented to land at the end, so this *probably*
still works, but it's worth tagging the content view (e.g. via
`setTag(...)` or a private subclass) instead of indexing.

### 4e. `attach_child` no-ops if parent isn't registered yet
`layout.rs:201-220` — if the parent has no layout handle, the child is
silently *not* registered. This matches the cocoa pattern
(cascade-on-mount), but the iOS `Element::insert_node`
(`node.rs:481-510`) inserts into the UIView tree *and* calls
`attach_child` immediately. So when a builder calls
`parent.insert_node(child, None)` before the parent is in any tree, the
child UIView is parented but never gets a Taffy node. In the cocoa
cascade, `ElementState::mount` is what does the insertion; here too, but
worth verifying every builder defers child mounting until its own
`mount()` runs.

### 4f. `app.rs::did_finish_launching` redundantly sets `Column` flex direction
Line 86-89 sets flex direction Column, but
`Element::create_with("vstack", mtm)` already sets that. Harmless, but
reads as a copy-paste artefact.

### 4g. `app.rs` leaks the layout tree with `mem::forget` (line 107)
The tree is kept alive only because clones inside `LayoutHandle.tree`
exist — but if no node is ever registered, the `Rc` would drop. Storing
the tree on the `AppDelegateState` (alongside `window` and
`content_root`) is cleaner than `mem::forget`.

### 4h. `ios_dom::Renderer::try_insert_node` is incorrect
`renderer.rs:125-132` calls `parent.insert_node(...)` and returns `true`
unconditionally. Real semantics ought to be: succeed iff parent is
currently a real parent of `new_child`'s superview. Compare to
`Dom::try_mount_before` in `tachys/src/renderer/ios.rs:147-158` which
does the right thing.

### 4i. `event.rs::keep_target_alive` and friends never expire
Same leak the cocoa port has, but at least cocoa documents it. The iOS
doc comment on `keep_target_alive` is silent.

### 4j. `key_event.rs` is dead code
`from_command_selector` is unused (compiler warns). Either wire it up
via `pressesBegan:` / `UIKeyCommand` or delete it. Currently
`on_text_keydown` / `on_text_keyup` in `node.rs:696-708` are bodyless
stubs.

### 4k. Stray `unsafe` blocks
`tachys/src/renderer/ios.rs:141, 151` — `before.ui_view().superview()`
is safe; the `unsafe` block draws a warning. Drop them.

### 4l. Unused imports
- `ios_dom/src/event.rs:18` — `Bool` unused.
- `tachys/src/ios/bind.rs:6,8-9` — almost the entire file's imports are
  unused (it's a stub).
- `tachys/src/ios/element.rs:11` — `BoolAttr` unused in this trimmed
  builder set.
- `tachys/src/ios/window.rs:10` — `MainThreadMarker` unused.

These are the warnings already emitted on build; cleaning them up makes
real warnings stand out.

---

## 5. Design issues / "dog's breakfast"

### 5a. Builder code is a fraction of cocoa's, with formatting deliberately compressed
`tachys/src/ios/element.rs` is 360 lines for 3 builders
(View/Button/Label) with one statement per line. The cocoa equivalent is
~3300 lines for ~14 builders. The agent appears to have ported only the
bare minimum to make the counter build, then stuffed the rest behind
aliases (`text_field = label`, `switch_ = view`). I'd treat what's there
as a sketch — a clean port from `cocoa/element.rs`, including the
`apply_universal` / `apply_text_attrs` helpers and
`impl_universal_attrs!` / `impl_text_attrs!` macros, will be a much
better starting point than extending the current draft.

### 5b. `Window` builder in tachys/src/ios/window.rs is vestigial
It exists for "API parity" but doesn't open a window — the `AppDelegate`
does that. So `mount_to_window` can't actually take title/size and have
them mean anything. Either delete the type and have `mount_to_window`
only forward to `run` (drop the `_title`/`_size` parameters), or
actually wire it through.

### 5c. `mount_to_window` is just `run` with discarded args
`mount_ios.rs:55-62` ignores both `title` and `size`. Cleaner to either
drop `mount_to_window` or have it set the initial window frame (as a
hint for future iPad multi-window) and the navigation title.

### 5d. `Element` API has grown unrelated to the macro
Several setter methods (`set_image_view_path`,
`set_progress_indeterminate`, `set_slider_vertical`,
`configure_stepper`, etc.) exist in `node.rs` but no
`tachys/src/ios/element.rs` builder calls them. Either delete or wire
up.

### 5e. `define_class!` of `AppDelegate` should not also implement `UIApplicationDelegate`'s deprecated `application:openURL:options:` etc.
Currently it implements only `application:didFinishLaunchingWithOptions:`
which is correct. Just flagging that — don't add the deprecated ones;
route through scene delegate methods instead when 3a is fixed.

### 5f. `dispatch2`/`spawner.rs` is fine
`spawn` (Send) and `spawn_local` both go through `spawn_main` — same as
cocoa. OK.

### 5g. `objc2-foundation` dep features include `NSCalendar`, `NSLocale`, `NSTimeZone`, `NSGeometry` etc.
None of these are used directly by `ios_dom` source. Probably copy/paste
from cocoa_dom; trim to what's actually used.

---

## 6. Build / packaging

### 6a. `run_ios.sh` Info.plist is missing `UIApplicationSceneManifest`
For a single-scene iOS 13+ app, the file works because UIKit falls back
to the AppDelegate's `application:didFinishLaunchingWithOptions:` window
creation. Once 3a is addressed, the manifest is required.

### 6b. `run_ios.sh` doesn't kill a previously running instance before launching
If you're iterating, you'll see stale builds run. Add
`xcrun simctl terminate "$DEVICE_ID" "$BUNDLE_ID" 2>/dev/null || true`
before install.

### 6c. `examples_ios/counter/Cargo.toml` has empty `[build-dependencies]`
Section header with no entries — minor noise, delete.

### 6d. The example doesn't set `MinimumOSVersion` in Info.plist
Apple requires it. Add
`<key>MinimumOSVersion</key><string>15.0</string>` (or whatever your
`objc2-ui-kit` 0.3 minimum is).

---

## Suggested fix order

1. **Unblock launch (1a, 1b)** — pass the mangled class name to
   `UIApplicationMain`, drop the `Send` bound on `mount_ios::run`. After
   this the counter should actually launch on the simulator.
2. **Verify counter renders** — only `vstack`/`hstack`/`label`/`button`
   are exercised; that should work end-to-end once it launches.
3. **Apply safe area + rotation handling (3c, 3d)** — otherwise content
   sits under the notch/home indicator.
4. **Port `tachys/src/cocoa/element.rs` properly (1c)** — replace the
   stub `text_field`/`switch_` aliases with real builders, port
   `Slider`/`TextField`/`Switch`/`SegmentedControl`/etc. one for one,
   including the `apply_universal` / `impl_text_attrs!` shape.
5. **Port `tachys/src/cocoa/bind.rs` (1d)** so `bind:value` /
   `bind:checked` actually work.
6. **Fix event routing (2)** — Change/Focus/Blur should fan out by
   control type, not assume `UITextField`.
7. **Modern scene delegate path (3a)** — add
   `UIApplicationSceneManifest` to the Info.plist, register a
   `UISceneDelegate`, move the window creation there. Remove the
   deprecated initWithFrame path.
8. **Consolidate `window.rs` vs `app.rs` (3b)** — delete the unused
   `open_window` or have `app.rs` call it.
9. Cleanup: 4a–4l, 5a–5g.
