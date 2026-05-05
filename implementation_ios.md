# Implementation log — Leptos iOS/UIKit port

A running record of design decisions made during the iOS/UIKit port,
especially the ones we deliberately deferred. Newest entries at the
top.

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
