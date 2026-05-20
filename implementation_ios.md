# Implementation log — Leptos iOS/UIKit port

A running record of design decisions made during the iOS/UIKit port,
especially the ones we deliberately deferred. Newest entries at the
top.

---

## 2026-05-19 — Direct typed attribute setters (port mirror)

Mirrored the cocoa attribute-setter cleanup. Removed `StringAttr`
and `BoolAttr` enums + their dispatch + the string-keyed
`set_attribute` / `remove_attribute`. Replaced with direct typed
methods on `Node`:

- `set_title(&str)` — UIButton.setTitle(Normal) / UILabel.setText.
- `set_value(&str)` — UITextField.setText / UITextView.setText.
- `set_placeholder(&str)` — UITextField.setPlaceholder.
- `set_hidden(bool)` — UIView.setHidden.
- `set_enabled(bool)` — UIView.setUserInteractionEnabled + UIControl.setEnabled.
- `set_checked(bool)` — UISwitch.setOn:animated:.

Builders in `leptos_uikit` and the bind layer migrated to direct
calls.

---

## 2026-05-19 — Typed element constructors + Element/Node unification (port mirror)

Mirrored two cocoa refactors landed earlier the same day:

1. Removed the `match tag { ... }` body in `Element::create_with` —
   each builder now calls a uniquely-named typed constructor
   (`Node::create_button`, `create_switch`, `create_label`,
   `create_scroll_view`, ...) from the new
   `uikit/dom/src/make_view.rs`. `Node::from_view` is the shared
   registration primitive. See cocoa's `implementation_log.md` entry
   for the full rationale.

2. Unified `Element` and `Node` into a single type. `pub type
   Element = Node;` aliased for backwards-compat; `as_node` /
   `into_node` / `from_node_unchecked` kept as identity methods so
   existing call sites work unchanged. `WeakElement` is now a type
   alias for `WeakNode`. `Mountable<Dom>` / `CastFrom<Node>` /
   `LayoutElement` / `UniversalElement` impls collapsed to single
   `Node` impls.

iOS-specific note: `<scroll_view>` allocates its content UIView as
the first subview of the UIScrollView (same as the pre-refactor
state). The cocoa-style `is_scroll_view` meta flag + internal Taffy
wrapper is set up by `Node::create_scroll_view`.

`AppDelegate` switched from
`Element::create_with(&tree, "vstack", mtm)` to
`Element::create_container_with(&tree, mtm)` + explicit
`set_flex_direction(Column)` — the typed constructor doesn't preset
direction.

`Renderer::create_element(tag, namespace)` was unused inside the
workspace and got deleted in step 1.

---

## 2026-05-19 — Node refactor part 2 (port mirror)

Mirrored the cocoa changes; see `implementation_log.md` for the
full rationale. iOS specifics:

- `IosBackend::Handlers = IosNodeHandlers` (set in the 2026-05-18
  pass). The text-view-delegate explicit-drop ordering fix in
  `IosNodeHandlers::Drop` is unchanged.
- Node = `Rc<NodeInner { tree, id, kind, view: Retained<UIView>, is_borrowed }>`
  with no state enum.
- `Element::create(tree, "foo")` eagerly allocates into the arena.
- `WeakElement` / `WeakNode` / `WeakText` / `WeakPlaceholder` added
  in cocoa's image. Same upgrade-on-fire pattern for closure-back-
  into-node use cases.

20/20 lifecycle tests pass on iPhone 17 Pro simulator. All 9
iOS examples build clean.

---

## 2026-05-18 — Node ownership refactor (port mirror)

Mirrored the cocoa Node refactor; see `implementation_log.md` for
the full rationale. iOS specifics:

- `IosBackend::Handlers = IosNodeHandlers` (was `()`). The
  `IosNodeHandlers` struct holds the same shape as cocoa's
  (`action_targets: Vec`, `text_view_delegate: Option`,
  `gesture_targets: Vec`, plus the new `view: Option<…>`
  back-ref).
- Same `NodeHandlers::Drop` text-view-delegate workaround as
  cocoa: the UITextView delegate `Retained` must drop explicitly
  before `disconnect_view_handlers` runs `setDelegate(None)`.
  UIKit's text-system pins an extra retain otherwise. See
  `uikit/dom/src/event.rs::IosNodeHandlers::drop`.
- `IosNodeHandlersBundle` deleted; `with_handlers_mut(|h| ...)`
  replaces `handlers().borrow_mut()`.
- Same Node accessor surface as cocoa.

---

## 2026-05-15 — Async runtime integration (port mirror)

Mirrors the cocoa-side async work (see top entry in
`implementation_log.md`) onto iOS. Two examples ported:
`uikit/examples/ipify` and `uikit/examples/async_patterns`.

Same `on_main` helper from `apple_shared` — libdispatch's
`DispatchQueue::main()` is identical on macOS and iOS, so the
port-shared module needed no iOS-specific code at all.

`set_image_view_bytes` setter added to `ios_dom::Element` via
`UIImage::imageWithData:`, plus the matching `.bytes(Option<Vec<u8>>)`
builder on the leptos_uikit `ImageView` and `Option<Vec<u8>>` in
the `impl_pair!` macro in `attr.rs`. Mechanical mirror of the
cocoa changes.

The iOS `<label>` builder doesn't have a `bold` attribute (cocoa
does); both examples render labels in the default UIKit weight.
Worth adding eventually but out of scope for this work.

Examples are excluded from the workspace (iOS examples aren't
workspace members; cargo can't conditionalise on target). Each
ships a `run_ios.sh` derived from `counter/run_ios.sh` via sed —
bundle name, binary path, bundle ID, and process-name predicate
adjusted; everything else identical. Verified each launches into
the simulator and runs without panicking.

The pattern-4 thread_local workaround (see SIGNAL_MT.md) is the
same on iOS — no difference in the cross-thread signal story
between the two Apple ports.

---

## Dark mode — adaptive colors via UIKit's named system colors

`Color` is an enum: `Rgba {…}` for fixed sRGB, `System(SystemColor)`
for one of UIKit's named adaptive colors (`labelColor`,
`systemBackgroundColor`, `systemBlueColor`, …). The system
variants return *dynamic* `UIColor`s that re-resolve on every
draw against the surrounding view's
`traitCollection.userInterfaceStyle`. UIKit handles the redraw on
`traitCollectionDidChange:` automatically — our reactive effects
don't need to re-fire because `[UILabel setTextColor:]` stores
the dynamic `UIColor` ref, not the resolved colour, and re-asks
on every redraw.

So dark-mode adaptation is hands-off as long as the builder
defaults (which use UIKit's own defaults — `labelColor` etc.) are
left alone, AND any explicit colors users set go through
`Color::System(...)` rather than `Color::Rgba {...}`.

The macro tripwire: `view!{}` wraps non-literal attribute values
through `IntoAttributeValue::into_attribute_value(expr)` (see
`leptos_macro/src/view/mod.rs`). For `text_color=Color::SYSTEM_BLUE`
to compile, `Color: IntoAttributeValue<Output=Color>` is needed —
added in `tachys/src/html/attribute/value.rs` as a native escape
hatch (mirroring the existing `Vec<&'static str>` /
`Vec<String>` impls).

Apple's full taxonomy of adaptive colors:
<https://developer.apple.com/design/human-interface-guidelines/foundations/color>

---

## Tap gestures — `userInteractionEnabled` defaults bite

`Element::on_click` now installs a `UITapGestureRecognizer` for
non-UIControl views (UIView, UILabel, UIImageView, container
stacks). The recognizer's target is an `ActionTarget` (same shared
class as the UIControl path), retained in the per-view
`HANDLER_STORE`.

Gotcha: `UILabel` and `UIImageView` default to
`userInteractionEnabled = NO`. Attaching a recognizer to a label
without flipping that flag silently swallows every tap. We force
it to `true` in `on_tap_gesture` before adding the recognizer.

The recognizer is retained by the view (`addGestureRecognizer:`
keeps a strong ref). The view holds a *weak* ref to its target —
the same shape as UIControl target/action — so the `ActionTarget`
must live in the handler store independently. Same leak shape
(entries cleared in `Node::teardown`).

`UITapGestureRecognizer::initWithTarget_action` takes
`Option<Sel>`, not `Sel` directly — small papercut to remember
when porting selectors that aren't optional in the docs.

---

## Modern scene delegate — programmatic UISceneConfiguration

iOS 13+ wants window creation through a `UISceneDelegate`, not the
legacy `UIApplicationDelegate.window` path. The shape is now:

1. **Info.plist** declares scene support via
   `UIApplicationSceneManifest = { UIApplicationSupportsMultipleScenes = false }`,
   with **no** `UISceneConfigurations` entry.
2. **AppDelegate** implements
   `application:configurationForConnectingSceneSession:options:`
   and returns a programmatic `UISceneConfiguration` whose
   `delegateClass` is `SceneDelegate::class()` — that lets us point
   UIKit at our objc2-mangled class name without baking it into
   Info.plist (which is read before our code can run).
3. **SceneDelegate** implements `UIWindowSceneDelegate`. Its
   `scene:willConnectToSession:options:` does the work that used to
   live in AppDelegate's `didFinishLaunchingWithOptions`: alloc the
   `UIWindow` via `init(windowScene:)`, set up the content root +
   Taffy tree + `RootViewController`, run the user's stored view
   builder closure, `makeKeyAndVisible`.

Two objc2 gotchas this surfaces:

- **`scene:willConnectToSession:options:` belongs to `UISceneDelegate`**,
  not `UIWindowSceneDelegate` (which inherits from it). The override
  must live in the `unsafe impl UISceneDelegate` block — putting it
  on the `UIWindowSceneDelegate` impl makes objc2 panic at startup
  with "failed overriding protocol method ... method not found"
  because the selector isn't on that specific protocol's method
  list.
- The programmatic-config method returns
  `Retained<UISceneConfiguration>`. That requires
  `#[unsafe(method_id(...))]` (not `#[unsafe(method(...))]`) so
  objc2 emits the autorelease bridging. Plain `method` rejects
  `Retained<T>` returns with `EncodeReturn` errors.

Also: register the SceneDelegate class eagerly at startup
(`SceneDelegate::class()` or a throwaway alloc) — objc2 registers
classes lazily, and waiting for first method dispatch is too late
because UIKit is already trying to look up the class by name from
the config we returned.

The deprecated `UIWindow::initWithFrame` and
`UIScreen::mainScreen` calls are gone; the
`#[allow(deprecated)]` guards with them.

---

## `UILaunchScreen` is required, or iOS runs at 320×480

Without `UILaunchScreen` (or `UILaunchStoryboardName`) in
`Info.plist`, iOS runs the app in legacy **320×480 compatibility
scaling mode** — the original iPhone 1 screen size — regardless of
the actual device. On a modern iPhone the app then renders into a
centered ~70% card, which looks like a sheet/modal but isn't.

Symptom: `view.bounds` reads `(0,0 320x480)` even on iPhone 16
(should be `393x852`). Window bounds match.

Fix: add an empty dict for `UILaunchScreen` to the bundled
Info.plist:
```xml
<key>UILaunchScreen</key>
<dict/>
```
Empty dict = "I support modern device sizes, no custom launch
UI." Required even when the app has no real launch screen
artwork.

Also add `MinimumOSVersion` (Apple-required for App Store
submissions). Both keys are now in every `examples_ios/*/run_ios.sh`.

---

## Keyboard avoidance — guard against unresolved `keyboardLayoutGuide`

`RootViewController::viewDidLayoutSubviews` reads
`view.keyboardLayoutGuide().layoutFrame()` to compute how much the
on-screen keyboard intrudes into the safe area, then adds that to
the content root's bottom padding so input fields stay visible
above the keyboard.

On the very first layout pass, UIKit hasn't resolved the keyboard
layout guide's constraints yet — `layoutFrame` returns
`CGRect.zero`. Without a guard, that gives
`raw_kb_bottom = bounds.height - 0 = bounds.height`, which gets
applied as bottom padding and crushes the entire app into a tiny
strip at the top. (Symptom: app appears "letterboxed into a small
square in the centre" on first display.)

Guard: if the resolved frame's `size.width <= 0`, treat the
keyboard as hidden (extra inset = 0). A resolved guide always has
the view's full width; an unresolved one has zeros.

Notification-based approaches (observing
`UIKeyboardWillShowNotification` etc.) avoid this entirely, but
require pulling `NSNotification` / `NSDictionary` features back
into `objc2-foundation` plus an Objective-C selector for the
observer. The guide-based path is much smaller; keep it with the
guard.

---

## Builders ported in cocoa-style — keep the macros DRY

`tachys/src/ios/element.rs` mirrors the cocoa structure: each
builder defines a struct with all attributes as fields, plus
`apply_universal` / `apply_text_attrs` helpers + the macros
`impl_universal_attrs!` / `impl_text_attrs!` /
`impl_typed_attrs_for!`. The first builder is slow; subsequent ones
fall to mostly-mechanical content.

iOS-specific deltas vs cocoa:
- No `tool_tip` (macOS hover concept). `apply_universal` is just
  `alpha`.
- `Switch` (UISwitch) replaces cocoa's `Checkbox` (NSButton-as-switch
  bezel). UISwitch has no title and no text styling — just on/off.
- `ProgressIndicator` is named for cocoa cross-port parity but is
  determinate-only (UIProgressView). Indeterminate spinners would be
  a separate `UIActivityIndicatorView` builder; deferred.
- No `PopUpButton` — UIMenu / UIPickerView are quite different from
  NSPopUpButton. Deferred.
- No `ColorWell` — UIColorPickerViewController is a modal sheet, not
  inline. Deferred.

---

## `<switch>` is an SVG-list tag in the macro — `r#switch` raw ident

leptos_macro's `is_svg_element` includes "switch" (it's a real SVG
tag in the web spec). So the macro emits `tachys::svg::switch()`
for `<switch>` — but `switch` is a Rust keyword. Solution: define
`pub fn r#switch()` in `tachys/src/svg_ios.rs`, delegating to
`tachys::ios::element::switch_()`. Same trick the web port already
uses for `r#use` / `<use>`.

---

## `objc2`'s `define_class!` mangles class names; UIKit allocs need init

`UIApplicationMain` looks up the AppDelegate by its registered
ObjC class name. With objc2's `define_class!`, the actual name is
mangled (something like `ios_dom_app_AppDelegate$$...`). Hard-coding
`"AppDelegate"` makes UIKit raise
`NSInternalInconsistencyException` before launch.

Fix: pass `AppDelegate::class().name()` (a `&CStr` from
`ClassType`) — that's the runtime-registered name. See
`uiapplication_main` in `ios_dom/src/app.rs`.

The trick has a sibling: when UIKit allocates the AppDelegate via
`[Class alloc] init]`, our Rust ivars come back uninitialised.
First `self.ivars()` access then panics with "tried to access
uninitialized instance variable". Fix: define an `-init` method
inside `define_class!` that calls `set_ivars` before forwarding to
super's init. The same pattern is needed for `RootViewController`
(also instantiable from outside Rust if iOS ever decides to).

For the macOS sibling port (which this one mirrors), see
[`implementation_log.md`](./implementation_log.md). The macOS log is
the canonical reference for Taffy bridging, event-handler storage,
the `mount_before` synthetic-parent dance, and other cross-cutting
concerns. This log covers only iOS-specific departures.

---

## Architecture overview

The iOS port follows the same three-layer architecture as the macOS
and GTK ports:

1. **`ios_dom/`** — DOM-shaped façade over UIKit. Provides `Node`,
   `Element`, `Text`, `Placeholder` backed by `UIView` and UIKit
   subclasses. Contains Taffy layout, event wiring, spawner, storage,
   interval, window/app management.

2. **`tachys/src/ios/`** — Element builders (Button, TextField, Label,
   Slider, Switch, ScrollView, etc.) that implement tachys' `Render`
   trait. Plus `attr.rs` (reactive attribute helpers), `bind.rs`
   (two-way binding), `node_ref.rs`, `window.rs`.

3. **`tachys/src/html/element_ios.rs` + `event_ios.rs`** — Facade
   modules so the `view!{}` macro's auto-generated paths resolve on
   iOS. Plus `tachys/src/svg_ios.rs` for the `<view>` tag re-route.

4. **`leptos/src/mount_ios.rs`** — Entry points: `run(closure)` and
   `mount_to_window(title, size, closure)`.

### Why not share code with cocoa_dom?

UIKit and AppKit share Foundation types (NSString, NSTimer,
NSUserDefaults, NSDate, NSNotification) through `objc2-foundation`,
but the UI layer is entirely different:

- **View classes**: Each AppKit class has a renamed UIKit analogue
  (NSView→UIView, NSButton→UIButton, NSTextField→UITextField,
  NSScrollView→UIScrollView, etc.). The method signatures are similar
  but the types are distinct Rust types in `objc2-ui-kit` vs
  `objc2-app-kit`.
- **Window/app model**: iOS has UISceneDelegate (modern) /
  UIApplicationDelegate, no NSWindow with title bars, no menu bar.
  Multi-windowing on iPad goes through UISceneSession, not arbitrary
  NSWindow creation.
- **Coordinate system**: UIKit uses top-left origin by default
  (unlike AppKit's bottom-left), so we don't need `FlippedView`.
- **Layout**: Taffy still works, but the intrinsic-size measurement
  closure calls UIKit methods on UIView/UIControl instead of AppKit
  ones on NSView/NSControl.
- **Safe areas**: iOS has `safeAreaInsets` that must be respected;
  macOS doesn't.
- **Hardware keyboard**: iOS supports Bluetooth/USB keyboards, but
  the event pipeline is different (UIKeyCommand / `pressesBegan:` vs
  NSResponder chain).

Separate crates keep the UIKit/AppKit types from colliding in the
same compilation unit. The shared Foundation types (NSDate, NSTimer,
etc.) are re-exported through `objc2-foundation` which is a common
dependency of both `objc2-app-kit` and `objc2-ui-kit`.

### What IS shared (no duplication)

- Taffy layout engine — identical dependency, same measure closure
  pattern (just calls UIKit intrinsicContentSize instead of AppKit's)
- `dispatch2` spawner — identical (same DispatchQueue::main())
- NSUserDefaults storage — identical code, different crate
- NSTimer intervals — identical code, different crate
- NSDate, NSNotification — shared via Foundation
- The tachys `Render`/`Mountable`/`CastFrom` trait impls — identical
  shape, same generic machinery
- The `view!{}` macro — unchanged; facade modules route its emissions
- reactive_graph, any_spawner — untouched

---

## UIKit vs AppKit: key differences

### View hierarchy

| macOS (AppKit)      | iOS (UIKit)        | Notes |
|---------------------|---------------------|-------|
| NSView              | UIView              | UIKit has top-left coords by default |
| NSControl           | UIControl           | Same target/action pattern |
| NSButton            | UIButton            | UIButton has `buttonWithType:` not `buttonWithTitle:...` |
| NSTextField         | UITextField         | UITextFieldDelegate has different method names |
| NSSecureTextField   | UITextField + secureTextEntry | Not a separate class |
| NSSlider            | UISlider            | Similar API |
| NSScrollView        | UIScrollView        | Similar pattern (documentView → contentView + contentSize) |
| NSTextView          | UITextView          | UITextView is NOT wrapped in a scroll view by default |
| NSPopUpButton       | —                   | No direct analogue; use UIMenu with UIButton or UIPickerView |
| NSProgressIndicator | UIProgressView       | Bar style; UIActivityIndicatorView for spinner |
| NSColorWell         | —                   | No UIKit analogue (use UIColorPickerViewController) |
| NSDatePicker        | UIDatePicker        | Different styles available |
| NSStepper           | UIStepper           | Similar API |
| NSSegmentedControl  | UISegmentedControl  | Similar API |
| NSImageView         | UIImageView         | Similar API |
| NSStackView         | UIStackView         | But we use Taffy; UIView containers suffice |
| NSWindow            | UIWindow            | iOS: one window fills screen; no title bar |
| NSMenu / NSMenuItem | —                   | No menu bar on iOS |
| FlippedView         | (not needed)        | UIKit defaults to top-left |
| NSColor             | UIColor             | Different class, same concept |
| NSFont              | UIFont              | Different class |
| NSEvent             | UIEvent             | Different dispatch pipeline |

### Application lifecycle

macOS: NSApplication → NSApplicationDelegate → NSWindow → NSView tree
iOS:   UIApplication → UIApplicationDelegate → UISceneDelegate → UIWindow → UIView tree

Modern iOS (13+) requires UISceneDelegate for window management.
We'll target iOS 15+ (the minimum for `objc2-ui-kit` 0.3).

### Threading

Same main-thread-only contract as cocoa_dom. UIKit calls must happen
on the main thread. We use the same `SendWrapper` + runtime panic
pattern.

### Event handling

UIControl target/action works the same as NSControl — we can port the
ActionTarget class and handler store pattern almost verbatim.

UITextFieldDelegate has different method names:
- `textFieldDidBeginEditing:` (vs `controlTextDidBeginEditing:`)
- `textFieldDidEndEditing:` (vs `controlTextDidEndEditing:`)
- `textFieldDidChangeSelection:` or `textField:shouldChangeCharactersIn:replacementString:` — but for `on:input` we'll use the `editingChanged` UIControl event (which fires on every keystroke via target/action, simpler than the delegate path)

UITextViewDelegate has different method names:
- `textViewDidChange:` (same as NSTextDelegate, conveniently)

---

## Implementation stages

### Stage 1 — `ios_dom/` crate (UIKit façade)

Create `ios_dom/` with these modules, porting from cocoa_dom:

| Module | Source | Changes needed |
|--------|--------|-----------------|
| `lib.rs` | cocoa_dom/src/lib.rs | `#![cfg(target_os = "ios")]`, UIKit re-exports |
| `node.rs` | cocoa_dom/src/node.rs | UIView instead of NSView; UIButton, UITextField, UISlider, UISwitch, UIScrollView, UIDatePicker, UIStepper, UIProgressView, UIImageView, UISegmentedControl instead of AppKit classes; no secure_text_field as separate class (use UITextField with secureTextEntry); no pop_up_button (no UIKit analogue in v1); no color_well (use UIColorPickerViewController in a later stage); no stack_view (just UIView containers) |
| `layout.rs` | cocoa_dom/src/layout.rs | UIView intrinsicContentSize instead of NSView; no FlippedView needed; safe area insets consideration; UIScrollView contentSize instead of NSScrollView documentView frame |
| `event.rs` | cocoa_dom/src/event.rs | UIControl target/action (same pattern); UITextFieldDelegate (different protocol); UITextViewDelegate; keep the thread-local handler store pattern |
| `spawner.rs` | cocoa_dom/src/spawner.rs | Identical — same dispatch2 pattern |
| `window.rs` | cocoa_dom/src/window.rs | UIWindow + UISceneDelegate; no title bar; safe area; single-window initially |
| `app.rs` | cocoa_dom/src/app.rs | UIApplication + UIApplicationDelegate; UISceneDelegate for window creation; no menu bar |
| `renderer.rs` | cocoa_dom/src/renderer.rs | Same shape, ios_dom types |
| `storage.rs` | cocoa_dom/src/storage.rs | Identical — NSUserDefaults is shared |
| `interval.rs` | cocoa_dom/src/interval.rs | Identical — NSTimer is shared |
| `color.rs` | cocoa_dom/src/color.rs | UIColor instead of NSColor |
| `date.rs` | cocoa_dom/src/date.rs | Identical — NSDate is shared |
| `key_event.rs` | cocoa_dom/src/key_event.rs | Deferred to later stage (UIKeyCommand for hardware keyboard) |

### Stage 2 — tachys renderer bridge

Add `tachys/src/renderer/ios.rs` — the `Dom` unit struct that
delegates to `ios_dom::Renderer`, with `Mountable` and `CastFrom`
impls. Direct port of `tachys/src/renderer/cocoa.rs`.

Wire into `tachys/src/renderer/mod.rs` with `cfg(target_os = "ios")`.

### Stage 3 — tachys element builders

Add `tachys/src/ios/` with these files:

| File | Source | Changes |
|------|--------|---------|
| `mod.rs` | tachys/src/cocoa/mod.rs | Re-exports |
| `element.rs` | tachys/src/cocoa/element.rs | UIKit-flavoured builders: Button, Label, TextField, Slider, Switch (replaces Checkbox), ScrollView, TextView, ImageView, DatePicker, Stepper, ProgressView, SegmentedControl, View, HStack, VStack; no PopUpButton (v1), no ColorWell (v1) |
| `attr.rs` | tachys/src/cocoa/attr.rs | Identical pattern |
| `bind.rs` | tachys/src/cocoa/bind.rs | UIKit-flavoured two-way binding |
| `directives.rs` | tachys/src/cocoa/directives.rs | Identical pattern |
| `node_ref.rs` | tachys/src/cocoa/node_ref.rs | Identical pattern |
| `render_html_stub.rs` | tachys/src/cocoa/render_html_stub.rs | Same stub pattern |
| `window.rs` | tachys/src/cocoa/window.rs | iOS-flavoured Window builder |

### Stage 4 — tachys facade modules

- `tachys/src/html/element_ios.rs` — Re-exports from `tachys::ios::element`
- `tachys/src/html/event_ios.rs` — Event descriptors (Click, Input, Change, Focus, Blur, KeyDown, KeyUp)
- `tachys/src/svg_ios.rs` — `view()` re-export for `<view>` tag
- Wire `tachys/src/html/mod.rs` for the element/event facades
- Wire `tachys/src/lib.rs` for the `svg` alias and `pub mod ios`

### Stage 5 — leptos mount entry point

- `leptos/src/mount_ios.rs` — `run()` and `mount_to_window()`
- Wire into `leptos/src/lib.rs` prelude and module declarations

### Stage 6 — workspace integration

- Add `ios_dom` to workspace `Cargo.toml` members
- Add `ios_dom` dep to `tachys/Cargo.toml` (optional, target_os = "ios")
- Add `ios_dom` dep to `leptos/Cargo.toml` (optional, target_os = "ios")
- Build scripts: ensure `leptos_native` cfg is set on iOS when `native-ui` feature is on (currently requires checking the build scripts in `tachys/build.rs`, `leptos/build.rs`, etc.)

### Stage 7 — examples

- `examples_ios/counter/Cargo.toml` + `src/main.rs`
- `examples_ios/counters/Cargo.toml` + `src/main.rs`
- `examples_ios/greeter/Cargo.toml` + `src/main.rs`
- `examples_ios/checkbox/Cargo.toml` + `src/main.rs`

### Stage 8 — deferred / follow-up

- Hardware keyboard events (UIKeyCommand, `pressesBegan:`)
- PopUpButton / PickerView
- ColorWell / UIColorPickerViewController
- iPad multi-windowing (UISceneSession multiple scenes)
- Navigation controller / tab bar controller integration
- Dark mode / trait collection reactive updates
- Dynamic Type (scaled fonts)
- Accessibility (VoiceOver, Dynamic Type)
- iOS-specific controls: Switch (UISwitch), ActivityIndicator, PageControl
- XCTest-based UI test harness (like the macOS XCUITest plan in `tests.md`)

---

## Key design decisions

### Decision 1: UISceneDelegate for window management

Modern iOS (13+) requires apps to use UISceneDelegate for window
creation. Older-style `UIApplicationDelegate.window` property still
works but is deprecated. We'll implement both:
- UISceneDelegate for iOS 13+ (the primary path)
- UIApplicationDelegate window property as fallback

The UISceneDelegate's `scene:willConnectToSession:options:` method
creates the UIWindow and sets its rootViewController. Our content is
mounted as a subview of the rootViewController's view.

### Decision 2: UIView as container instead of FlippedView

UIKit uses top-left coordinates by default, so we don't need a
flipped view subclass. All container views are plain UIView instances.

### Decision 3: UIScrollView content sizing via Taffy

Same two-pass approach as macOS NSScrollView (see
`cocoa_dom/src/layout.rs:relayout_scroll_views`). First pass lays out
within the viewport; second pass re-lays out the scroll view's
subtree with MaxContent height so children take natural sizes. Then
we set `contentSize` on the scroll view to match.

iOS difference: UIScrollView doesn't have a `documentView` property
like NSScrollView. Instead, children are added directly to the scroll
view, and the scrollable area is set via `contentSize`. We add an
intermediate content UIView as the single child of UIScrollView, and
mount user children inside that — this mirrors NSScrollView's
documentView pattern and lets Taffy lay out the content view
naturally.

### Decision 4: Secure text field as UITextField property

NSSecureTextField is a separate AppKit class, but iOS uses
`UITextField.isSecureTextEntry = true`. Our `<secure_text_field>` tag
creates a regular UITextField with `setSecureTextEntry(true)`.

### Decision 5: Switch instead of Checkbox

iOS uses UISwitch for boolean toggles (not NSButton with checkbox
bezel). We'll call the tag `<switch>` and provide a Switch builder
with `bind:checked` support. The UISwitch `.isOn` property maps to
the same "checked" semantics.

For cross-platform code, we could alias `<checkbox>` to `<switch>` on
iOS in the facade, but that's a later concern — v1 uses `<switch>`
explicitly.

### Decision 6: No menu bar

iOS has no menu bar. The `app.rs` module doesn't create menus (unlike
cocoa_dom's `app.rs` which installs a default menu). The
`UIApplicationDelegate` just handles scene lifecycle.

### Decision 7: Safe area insets

iOS devices have safe area insets (status bar, notch, home indicator).
We expose a `safe_area_insets` property on the root view and apply
them as padding on the content root, so Taffy layout stays within the
safe area. Users can opt out with `.ignore_safe_area(true)` on the
Window builder.

### Decision 8: `target_os = "ios"` cfg gate

Rust/Cargo uses `target_os = "ios"` for iOS targets (both device and
simulator). This is the cfg we use to disambiguate the iOS path from
macOS (`target_os = "macos"`) and Linux (`target_os = "linux"`).

---

## Open items

- **UISwitch initial size**: UISwitch has a fixed intrinsic size (51×31
  points). Need to test that Taffy doesn't try to stretch it.
- **UITextView scrolling**: Unlike NSTextView (which must be wrapped in
  NSScrollView), UITextView IS a UIScrollView subclass. So `<text_view>`
  on iOS uses UITextView directly, not a wrapper. The scroll behavior
  is built in.
- **Status bar style**: iOS status bar can be light or dark content.
  We should expose this via an app-level config, likely deferred.
- **Orientation changes**: iOS rotates. Taffy recompute on
  `viewWillTransitionToSize:` (called on the root view controller).
  We'll wire this into the Window delegate.
- **Multi-window on iPad**: UISceneDelegate's
  `scene:willConnectToSession:options:` is called once per scene.
  Our `run()` creates one window per call. Multi-window would need
  the user to manually manage multiple scene sessions — deferred.

## 2026-05-20 — TLS node store + `Copy` `NodeId` (see implementation_log.md)

The cross-cutting `Node`-becomes-`NodeId`-over-a-thread-local-store
refactor landed on this port too, mirroring cocoa one-for-one. Full
rationale + the shared design is in the top entry of
`implementation_log.md` (2026-05-20). Port-local notes: same
`LayoutBackend::with_tree` + `thread_local!` store, `Node` is a `Copy`
id (no `Rc`/`SendWrapper`/refcount), explicit teardown+cascade
lifecycle, and the walk-up-to-root relayout scheduler.
