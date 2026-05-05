# iOS port — task list

Each stage is sequenced; within a stage, tasks are roughly in
dependency order. All tasks assume the `native-ui` feature flag
pattern established by the macOS and GTK ports.

---

## Stage 1 — `ios_dom/` crate (UIKit façade)

- [ ] **1.1** Create `ios_dom/Cargo.toml` — set up dependencies on `objc2`, `objc2-foundation`, `objc2-ui-kit`, `dispatch2`, `any_spawner`, `taffy`, `send_wrapper`, `futures`. Mirror `cocoa_dom/Cargo.toml` structure but with UIKit feature flags instead of AppKit ones. Gate all deps on `cfg(target_os = "ios")`.

- [ ] **1.2** Create `ios_dom/src/lib.rs` — `#![cfg(target_os = "ios")]`, declare modules (`app`, `color`, `date`, `event`, `layout`, `node`, `renderer`, `spawner`, `storage`, `interval`, `window`), re-export key types (`Node`, `Element`, `Text`, `Placeholder`, `Color`, `Date`, `Storage`, `Renderer`, etc.), re-export `objc2` and `objc2_ui_kit` types (`Retained`, `MainThreadMarker`, `UIView`).

- [ ] **1.3** Create `ios_dom/src/node.rs` — Port `Element::create_with` from cocoa_dom. Map tags to UIKit classes:
  - `"button"` → `UIButton::buttonWithType(UIButtonTypeSystem)`
  - `"label"` → `UILabel` (not `NSTextField` with label style)
  - `"text_field"` → `UITextField`
  - `"secure_text_field"` → `UITextField` with `isSecureTextEntry = true`
  - `"slider"` → `UISlider` (continuous = true)
  - `"switch"` → `UISwitch` (new; iOS-native toggle)
  - `"date_picker"` → `UIDatePicker`
  - `"stepper"` → `UIStepper`
  - `"progress_indicator"` → `UIProgressView` (determinate bar)
  - `"image_view"` → `UIImageView`
  - `"segmented_control"` → `UISegmentedControl`
  - `"scroll_view"` → `UIScrollView` with content UIView child
  - `"text_view"` → `UITextView` (native scroll view subclass)
  - `"view"` → plain `UIView`
  - `"stack_view"` → plain `UIView` with Column flex default
  - unknown → plain `UIView`
  - Port `Node`, `Text`, `Placeholder` structs (same shape, UIView-backed)
  - Port `StringAttr`, `BoolAttr` enums
  - Port attribute setters (`set_string_attribute`, `set_bool_attribute`, `set_double_value`, etc.) adapted for UIKit APIs
  - Port event wiring stubs (`on_click`, `on_text_change`, `on_action`, etc.)
  - Port `insert_node`, `remove_child`, `clear_children`, `subview_parent`
  - Port `teardown`
  - Port `focus()`, `blur()`
  - No FlippedView — UIView containers are plain `UIView::initWithFrame`

- [ ] **1.4** Create `ios_dom/src/layout.rs` — Port Taffy integration from `cocoa_dom/src/layout.rs`:
  - Same `LayoutTree`, `TreeRef`, `NodeLayout`, `LayoutHandle` types
  - Same `register_in_tree`, `drop_node`, `attach_child`, `detach_child`, `insert_child_at`
  - Same `schedule_relayout` / `schedule_relayout_for_tree` (dispatch2-based dedup)
  - `compute_layout` — root fill, `compute_layout_with_measure`, scroll view second pass
  - `measure_leaf` — call `UIView.intrinsicContentSize` (or `sizeToFit` then read frame) instead of NSView's. UIControl `sizeToFit` → read frame for button/label/field sizing. Editable UITextField width=0 (same as macOS).
  - `apply_layout` — walk Taffy tree, set UIView frames. UIScrollView: set `contentSize` on the scroll view from the union of children's rects.
  - Style setters: `set_width`, `set_height`, `set_flex_direction`, `set_padding`, `set_gap`, `set_flex_grow`, `set_justify_content`, `set_margin`
  - No `FlippedView` needed — UIKit is already top-left

- [ ] **1.5** Create `ios_dom/src/event.rs` — Port event wiring:
  - `ActionTarget` ObjC class (same pattern: holds `Box<dyn FnMut()>`, exposes `actionFired:`)
  - `HANDLER_STORE` thread-local (same pattern)
  - `on_control_action` for UIControl target/action
  - `TextFieldDelegate` — implements `UITextFieldDelegate`:
    - `textFieldDidBeginEditing:` → focus callbacks
    - `textFieldDidEndEditing:` → change + blur callbacks
    - For input (every keystroke): use `editingChanged` UIControl event via target/action (simpler than `textField:shouldChangeCharactersIn:replacementString:`)
  - `TextViewDelegate` — implements `UITextViewDelegate`:
    - `textViewDidChange:` → change callbacks
  - Port all `on_text_field_*` and `on_text_view_*` helpers
  - Port `keep_target_alive`, `drop_handlers_for`

- [ ] **1.6** Create `ios_dom/src/spawner.rs` — Identical to `cocoa_dom/src/spawner.rs` (dispatch2 main queue executor)

- [ ] **1.7** Create `ios_dom/src/window.rs` — iOS window management:
  - `UIWindow` creation (fills screen; iOS windows don't have arbitrary position/size)
  - Content root: plain UIView registered in a fresh Taffy tree
  - UISceneDelegate for window lifecycle (resize → layout recompute, window close → teardown)
  - Safe area inset handling: apply as padding on content root
  - `OpenedWindow` struct with UIWindow, content_root Element, tree, delegate
  - Initial layout pass before making window visible

- [ ] **1.8** Create `ios_dom/src/app.rs` — UIApplication setup:
  - `init_app()` — create UIApplicationDelegate, register spawner
  - `run_loop()` — call `UIApplicationMain` (or equivalent via objc2)
  - UISceneDelegate registration (via Info.plist or programmatic)
  - No menu bar (unlike macOS)

- [ ] **1.9** Create `ios_dom/src/renderer.rs` — Port from cocoa_dom (same shape, ios_dom types). Stub `ClassList`, `CssStyleDeclaration`, `Event`, `TemplateElement`.

- [ ] **1.10** Create `ios_dom/src/storage.rs` — Identical to cocoa_dom (NSUserDefaults via objc2-foundation)

- [ ] **1.11** Create `ios_dom/src/interval.rs` — Identical to cocoa_dom (NSTimer via objc2-foundation)

- [ ] **1.12** Create `ios_dom/src/color.rs` — UIColor wrapper (port from cocoa_dom Color, using UIColor instead of NSColor)

- [ ] **1.13** Create `ios_dom/src/date.rs` — Identical to cocoa_dom (NSDate via objc2-foundation)

- [ ] **1.14** Create `ios_dom/src/key_event.rs` — Stub for v1 (UIKeyCommand for hardware keyboard deferred)

- [ ] **1.15** Verify `ios_dom` compiles: `cargo build -p ios_dom` (on macOS, this compiles to empty since `#![cfg(target_os = "ios")]`; real check needs an iOS target or `cargo check --target aarch64-apple-ios`)

---

## Stage 2 — tachys renderer bridge

- [ ] **2.1** Create `tachys/src/renderer/ios.rs` — `Dom` unit struct delegating to `ios_dom::Renderer`. Port `Mountable` impls for `Node`, `Element`, `Text`, `Placeholder`. Port `CastFrom` impls. Port `synthesise_parent_element` for `mount_before`.

- [ ] **2.2** Wire `tachys/src/renderer/mod.rs` — Add `pub mod ios` gated on `cfg(all(target_os = "ios", leptos_native))`. Add `Rndr` alias and `types` re-exports for iOS.

- [ ] **2.3** Wire `tachys/src/lib.rs` — Add `pub mod ios` gated on `cfg(all(target_os = "ios", leptos_native, feature = "reactive_graph"))`. Add prelude `Dom` re-export for iOS.

---

## Stage 3 — tachys element builders

- [ ] **3.1** Create `tachys/src/ios/mod.rs` — Module declarations and re-exports. Port `FlexDirection`, `JustifyContent` passthrough.

- [ ] **3.2** Create `tachys/src/ios/element.rs` — UIKit-flavoured builder structs:
  - `View<Ch, At>` — generic UIView container (like cocoa's `View`)
  - `Button<Ch, At>` — UIButton builder (`.title()`, `.on(click, ...)`)
  - `Label<At>` — UILabel builder (`.text()`)
  - `TextField<At>` — UITextField builder (`.value()`, `.placeholder()`, `.on(input, ...)`, `.bind(value, signal)`)
  - `SecureTextField<At>` — same as TextField with secureTextEntry
  - `Switch<At>` — UISwitch builder (`.checked()`, `.bind(checked, signal)`)
  - `Slider<At>` — UISlider builder (`.value()`, `.min()`, `.max()`, `.bind(value, signal)`)
  - `DatePicker<At>` — UIDatePicker builder
  - `Stepper<At>` — UIStepper builder
  - `ProgressView<At>` — UIProgressView builder
  - `ImageView<At>` — UIImageView builder
  - `SegmentedControl<At>` — UISegmentedControl builder
  - `ScrollView<Ch, At>` — UIScrollView builder
  - `TextView<At>` — UITextView builder
  - `HStack<Ch, At>`, `VStack<Ch, At>` — convenience aliases for View with Row/Column
  - Constructor functions: `view()`, `button()`, `label()`, `text_field()`, `secure_text_field()`, `switch_()`, `slider()`, `date_picker()`, `stepper()`, `progress_view()`, `image_view()`, `segmented_control()`, `scroll_view()`, `text_view()`, `hstack()`, `vstack()`
  - Universal attrs: `alpha()`, `tool_tip()` (tooltip doesn't exist on iOS — no-op or map to accessibilityLabel)
  - Text attrs: `text_color()`, `alignment()`, `font_size()`

- [ ] **3.3** Create `tachys/src/ios/attr.rs` — Port `MaybeReactive<T>`, `IntoMaybeReactive<T>`, `install` helper (identical pattern)

- [ ] **3.4** Create `tachys/src/ios/bind.rs` — Port `IntoSignal<T>`, `BindAttribute` with UIKit-specific two-way binding impls:
  - `bind:value` on UITextField
  - `bind:checked` on UISwitch
  - `bind:value` on UISlider
  - `bind:value` on UIDatePicker
  - `bind:value` on UIStepper
  - `bind:value` on UISegmentedControl
  - `bind:value` on UITextView

- [ ] **3.5** Create `tachys/src/ios/directives.rs` — Port directive helpers (same pattern as cocoa)

- [ ] **3.6** Create `tachys/src/ios/node_ref.rs` — Port `NodeRef` (monomorphic over `ios_dom::Element`)

- [ ] **3.7** Create `tachys/src/ios/render_html_stub.rs` — Stub `RenderHtml`/`AddAnyAttr` impls via macro

- [ ] **3.8** Create `tachys/src/ios/window.rs` — `Window` builder struct + `window()` constructor, iOS-flavoured (wraps UISceneDelegate / UIWindow creation)

---

## Stage 4 — tachys facade modules

- [ ] **4.1** Create `tachys/src/html/element_ios.rs` — Re-export all builders from `tachys::ios::element`. Also re-export `view as div`.

- [ ] **4.2** Create `tachys/src/html/event_ios.rs` — Event descriptors and `PendingHandler` enum for iOS. Port `ClickEvent`, `InputEvent`, `ChangeEvent`, `FocusEvent`, `BlurEvent`, `KeyDownEvent`, `KeyUpEvent`. Port `SupportsEvent` impls for each builder. Port `on()` free-standing function and `OnAttribute`.

- [ ] **4.3** Create `tachys/src/svg_ios.rs` — `pub use crate::ios::element::view;` for the `<view>` SVG tag re-route.

- [ ] **4.4** Wire `tachys/src/html/mod.rs` — Add `pub mod element_ios` and `pub mod event_ios` gated on `cfg(all(target_os = "ios", leptos_native))`. Add `pub use element_ios as element` and `pub use event_ios as event` for iOS.

- [ ] **4.5** Wire `tachys/src/lib.rs` — Add `svg_ios` module + `pub use svg_ios as svg` for iOS. Ensure all `cfg` gates are correct.

---

## Stage 5 — leptos mount entry point

- [ ] **5.1** Create `leptos/src/mount_ios.rs` — `run(closure)` and `mount_to_window(title, size, closure)` entry points. iOS-flavoured: calls `ios_dom::app::init_app()`, creates Owner, builds view, runs UIApplicationMain loop.

- [ ] **5.2** Wire `leptos/src/lib.rs` — Add `pub mod mount_ios` gated on `cfg(all(target_os = "ios", leptos_native))`. Add prelude re-exports: `pub use crate::mount_ios::*`, `pub use ios_dom::*` (storage, interval), `pub use tachys::ios::NodeRef`, `pub use tachys::ios::BindAttribute`. Add `pub use tachys::ios as ios` (analogue of `pub use tachys::cocoa as cocoa`).

---

## Stage 6 — workspace integration

- [ ] **6.1** Add `"ios_dom"` to workspace `Cargo.toml` members list

- [ ] **6.2** Add `ios_dom` optional dep to `tachys/Cargo.toml` under `[target.'cfg(target_os = "ios")'.dependencies]`: `ios_dom = { path = "../ios_dom", version = "0.1.0", optional = true }`. Add `"dep:ios_dom"` to the `native-ui` feature.

- [ ] **6.3** Add `ios_dom` optional dep to `leptos/Cargo.toml` under `[target.'cfg(target_os = "ios")'.dependencies]`: `ios_dom = { path = "../ios_dom", version = "0.1.0", optional = true }`. Add `"dep:ios_dom"` to the `native-ui` feature.

- [ ] **6.4** Check build scripts — Verify that `tachys/build.rs`, `leptos/build.rs`, `leptos_dom/build.rs` (and any others that set `leptos_native` cfg) include `target_os = "ios"` in their auto-detection or that the feature-flag path already covers it.

- [ ] **6.5** Verify compilation — On macOS: `cargo build --workspace` (should still work, ios_dom compiles to empty rlib). With iOS target installed: `cargo check -p ios_dom --target aarch64-apple-ios`, `cargo check -p tachys --features native-ui --target aarch64-apple-ios`, `cargo check -p leptos --features native-ui --target aarch64-apple-ios`.

---

## Stage 7 — examples

- [ ] **7.1** Create `examples_ios/counter/` — Basic counter app (button + label). `Cargo.toml` with `leptos = { features = ["native-ui"] }`, `src/main.rs` with `#[component] fn App() -> impl IntoView { ... }` and `mount_to_window("Counter", (375.0, 667.0), || view! { <App/> })`.

- [ ] **7.2** Create `examples_ios/counters/` — Dynamic list with `<For>`, add/remove counters.

- [ ] **7.3** Create `examples_ios/greeter/` — Text field + label, two-way binding demo.

- [ ] **7.4** Create `examples_ios/checkbox/` — Switch (UISwitch) with `bind:checked`, reactive label showing state.

- [ ] **7.5** Create `examples_ios/controls/` — Showcase of available controls: slider, date picker, stepper, segmented control, progress view, scroll view, text view, image view, secure text field.

---

## Stage 8 — deferred / follow-up

- [ ] **8.1** Hardware keyboard events — `UIKeyCommand` registration, `pressesBegan:` handler, port `KeyEvent` from cocoa_dom.

- [ ] **8.2** PopUpButton analogue — `UIMenu` with `UIButton` (iOS 14+), or `UIPickerView` for a scrolling picker.

- [ ] **8.3** ColorWell analogue — `UIColorPickerViewController` integration.

- [ ] **8.4** Activity indicator — `UIActivityIndicatorView` as a separate `<activity_indicator>` tag or a `.style()` variant on ProgressView.

- [ ] **8.5** iPad multi-windowing — Support multiple `UISceneSession`s; allow `run()` to handle multiple scenes.

- [ ] **8.6** Orientation change relayout — Wire `viewWillTransitionToSize:withTransitionCoordinator:` on the root view controller to trigger Taffy recompute.

- [ ] **8.7** Safe area live updates — `viewSafeAreaInsetsDidChange` → update root padding → relayout.

- [ ] **8.8** Dark mode / trait collection — `traitCollectionDidChange:` → update reactive `ColorScheme` signal.

- [ ] **8.9** Dynamic Type — `UIContentSizeCategoryDidChange` notification → reactive font size scaling.

- [ ] **8.10** Accessibility — Expose `accessibilityLabel`, `accessibilityHint`, `isAccessibilityElement` as attributes on builders.

- [ ] **8.11** Navigation controller integration — Allow mounting into a `UINavigationController` stack.

- [ ] **8.12** Tab bar controller integration — `UITabBarController` support.

- [ ] **8.13** Modal presentation — `UIViewController.present` for sheets/alerts.

- [ ] **8.14** XCTest UI test harness — See `tests.md` for the macOS XCUITest plan; adapt for iOS.
