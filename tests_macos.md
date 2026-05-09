# Test plan — Leptos macOS port

A running checklist of everything in the macOS port that needs automated
test coverage. Written early so we don't ship a feature without
remembering to come back. Items are grouped by module (rough Stage
order). Crosshair `□` items are not yet covered; `■` are.

## How to read this

- **Type** indicates the cheapest test that would catch regressions:
  - *unit* — pure-Rust assertion, no run loop, no window. Just builds
    structures and reads state.
  - *action* — programmatically invoke an NSControl's target/action via
    `[target performSelector:action]`. Exercises handlers without a
    window or event loop.
  - *layout* — build a tree, run Taffy compute, assert frames.
  - *integration* — spin up a hidden NSApplication on a worker run loop,
    post events, assert. Higher cost; reserve for things the cheaper
    tiers can't catch.
  - *XCUIAutomation* — Swift XCTest target, fully end-to-end. Defer
    until after Stage 5 (API churn).
- **Why it matters** is included when it's not obvious. If a regression
  here lands silently, what's the symptom?

## 0. Test infrastructure to build first

Before writing the per-module tests below, we need a small set of
helpers. These are "Stage 0" of the testing work.

- ■ **MainThreadMarker for tests.** AppKit calls require an `mtm`. In
  unit tests we're already on the test runner's main thread; provide a
  `test_mtm()` helper that grabs one (or skips with a clear error).
- ■ **`fire_action(&button)`.** Reads `target` and `action` off an
  NSControl, calls `performSelector:withObject:` with the control as
  the sender. The "synthetic click" used by the *action* tier.
- ■ **`fire_control_text_did_change(&field)`.** Same but posts the
  notification NSTextField listens for. Needed for `on:input` tests.
- ■ **Layout helper.** `compute_layout(root, available_size)` that
  walks the cocoa_dom tree, syncs to a Taffy tree, computes, writes
  back frames. Will fall out of Stage 4 anyway.
- ■ **Frame assertion helpers.** `assert_frame_eq!(view, x, y, w, h)`
  with f64 tolerance.
- ■ **Subtree pretty-printer.** For diagnostics on assertion failure —
  print the NSView tree with tags, frames, and text. Saves debugging
  time on every later test.
- ■ **Reactive-system scope.** Tests that touch signals need an Owner
  set on the current thread. A `with_reactive_scope(|| { ... })`
  helper that creates and forgets an Owner, runs the test body, then
  drops the Owner cleanly.
- □ **Handler registry reset.** `clear_handler_store()` between tests
  so leaks from one test don't pollute the next.
- □ **CI policy decision.** Which tests can run on a headless macOS
  runner without a window server? AppKit run loop tests often can't.
  Mark each integration-tier test with whether it needs a Window
  Server session.

## 1. cocoa_dom::node

### Element creation
- ■ *unit* `Element::create("view")` produces an NSView (not a subclass).
- ■ *unit* `Element::create("button")` produces an NSButton with the
  rounded push-button bezel (constructed via
  `buttonWithTitle:target:action:`, not `initWithFrame:`).
- ■ *unit* `Element::create("checkbox")` produces an NSButton in the
  switch/checkbox style (constructed via
  `checkboxWithTitle:target:action:`).
- ■ *unit* `Element::create("label")` produces a non-editable,
  non-bordered NSTextField (the `labelWithString:` configuration).
- ■ *unit* `Element::create("text_field")` produces an editable
  NSTextField.
- ■ *unit* `Element::create("secure_text_field")` produces an
  NSSecureTextField (downcasts as NSTextField too — NSSecureTextField
  IS-A NSTextField).
- ■ *unit* `Element::create("slider")` produces an NSSlider with
  `isContinuous = true`.
- ■ *unit* `Element::create("pop_up_button")` produces an NSPopUpButton
  in pull-up (not pull-down) mode.
- ■ *unit* `Element::create("stack_view")` produces a flipped layout
  container with `flex_direction: Column` default.
- ■ *unit* `Element::create("unknown_xyz")` falls back to a flipped
  NSView container.
- ■ *unit* The element's `kind()` is always `NodeKind::Element`.
- □ *unit* Off-main: `Element::create` from a non-main thread panics
  via the `MainThreadMarker::new()` failure or SendWrapper guard.

### Text & Placeholder creation
- ■ *unit* `Text::create("hello")` makes a label-style NSTextField with
  the given string and `kind() == NodeKind::Text`.
- ■ *unit* `Text::create("")` works — empty string is valid.
- ■ *unit* Multi-line text: newlines preserved in the field's value.
- ■ *unit* `Placeholder::create()` produces a hidden, zero-size NSView
  with `kind() == NodeKind::Placeholder`.
- □ *unit* Placeholders don't intercept hit-testing (mouse passes
  through to siblings/parents).

### Round-trip & casts
- ■ *unit* `Element::into_node()` then `Element::from_node_unchecked()`
  yields equivalent Element (same NSView pointer).
- □ *unit* `CastFrom<Node>::cast_from(node)` returns Some only when
  kind matches. Element rejected to Text returns None and vice versa.
- □ *unit* `AsRef<Node>` for Element/Text/Placeholder borrows the
  inner Node without copying.
- ■ *unit* `Node::ptr_eq` is true for clones of the same Node, false
  for differently-constructed ones with different NSViews.

### Tree mutation
- ■ *unit* `insert_node(child, None)` appends to subviews.
- ■ *unit* `insert_node(child, Some(marker))` places child immediately
  before the marker; subviews-array order is preserved.
- □ *unit* Inserting before a marker that's not actually a child
  silently appends (matches web DOM behaviour).
- ■ *unit* `insert_node` of a child that's already mounted somewhere
  else moves it (NSView semantics — a view can have only one parent).
- ■ *unit* `remove_child(child)` returns Some(node) and removes the
  view; calling `remove_child` for a non-child returns None and
  doesn't disturb the tree.
- ■ *unit* `clear_children()` removes all subviews; calling on an
  already-empty parent is a no-op.
- □ *unit* `clear_children` on a parent that's a layout container
  (NSStackView) doesn't leave dangling stack arrangements.

### Attribute setters — typed (StringAttr, BoolAttr)
- ■ *unit* `set_string_attribute(StringAttr::Title, "X")` on a button
  updates `[button title]`.
- ■ *unit* `set_string_attribute(StringAttr::Value, "X")` on an
  editable text_field updates `stringValue`.
- ■ *unit* `set_string_attribute(StringAttr::Placeholder, "X")` on a
  text_field updates `placeholderString`.
- ■ *unit* `set_string_attribute` is a same-value no-op (skips the
  AppKit setter when the current value already matches — this is what
  prevents the focus-ring flash on `bind:` cycles).
- ■ *unit* `set_string_attribute(StringAttr::Title, "X")` schedules a
  relayout (intrinsic content size may have changed).
- ■ *unit* `set_string_attribute(StringAttr::Placeholder, "X")` also
  schedules a relayout (placeholder width contributes to intrinsic
  size when the field is empty).
- ■ *unit* `set_bool_attribute(BoolAttr::Hidden, true)` and `false`
  toggle the view's hidden flag.
- ■ *unit* `set_bool_attribute(BoolAttr::Enabled, true/false)` toggles
  `NSControl::isEnabled`. No-op on non-NSControl views.
- ■ *unit* `set_bool_attribute(BoolAttr::Checked, true/false)` toggles
  `NSButton::state` (NSControlStateValueOn / Off). No-op on
  non-button views.
- ■ *unit* `set_bool_attribute` is a same-value no-op for each variant.

### Attribute removal
- ■ *unit* `remove_string_attribute(StringAttr::Title)` clears the
  button title to "".
- ■ *unit* `remove_string_attribute(StringAttr::Value)` clears
  `stringValue` to "".
- ■ *unit* `remove_string_attribute(StringAttr::Placeholder)` clears
  the placeholder.
- ■ *unit* `remove_bool_attribute(BoolAttr::Hidden)` sets hidden to
  false (visible again).
- ■ *unit* `remove_bool_attribute(BoolAttr::Enabled)` sets enabled to
  true (NSControl's default).
- ■ *unit* `remove_bool_attribute(BoolAttr::Checked)` sets state to
  Off.

### Attribute name lookup (typed enum ↔ string)
- ■ *unit* `StringAttr::from_name("title")` → `Some(Title)`;
  `from_name("xyz")` → `None`.
- ■ *unit* `BoolAttr::from_name("enabled")` → `Some(Enabled)`;
  unknown names → `None`.
- ■ *unit* `StringAttr::name()` and `BoolAttr::name()` round-trip
  with `from_name` for every variant.

### Stringly-typed `Rndr` trait surface
- ■ *unit* `Element::set_attribute("title", "X")` (the `&str`
  trait-side entry point) routes through `StringAttr::from_name` and
  updates the button.
- ■ *unit* `Element::set_attribute` with an unknown name is a silent
  no-op.
- ■ *unit* `Element::set_attribute("enabled", "true")` does NOT toggle
  enabled — bool attrs are deliberately not parsed from the string
  entry. (Builders use the typed setter directly.)
- ■ *unit* `Element::remove_attribute("title")` clears the title;
  `remove_attribute("hidden")` resets to visible.

### Text node mutation
- ■ *unit* `Text::set_text` updates the displayed string and
  schedules a relayout.

### Element control wiring
- ■ *unit* `Element::on_click(cb)` on a non-button is a silent no-op
  (no panic, registry untouched).
- ■ *action* `Element::on_action(cb)` on a slider — `fire_action`
  fires the closure (NSSlider extends NSControl directly, NOT
  NSButton, so `on_click` would have silently dropped — see
  implementation log slider regression).
- ■ *action* `Element::on_action(cb)` on a popup fires.
- □ *action* `Element::on_action(cb)` on a button fires (button is
  NSControl too — `on_action` is the strict generalization).
- ■ *unit* `Element::on_action` on a non-NSControl (label, view) is a
  silent no-op.
- ■ *action* `Element::on_text_change(cb)` on a text_field fires when
  `controlTextDidChange:` is posted.
- ■ *unit* `Element::on_text_change` on a non-text_field is a silent
  no-op.
- ■ *action* `Element::on_text_end_editing(cb)` fires when
  `controlTextDidEndEditing:` is posted.

### Element value getters
- ■ *unit* `Element::checked()` reads NSButton state; returns false
  for non-button.
- ■ *unit* `Element::double_value()` reads `NSControl::doubleValue`;
  returns 0.0 for non-control.
- ■ *unit* `Element::set_double_value(v)` writes
  `NSControl::doubleValue`; same-value writes diff and skip.
- ■ *unit* `Element::set_slider_min/max` set the slider's bounds.
- ■ *unit* `Element::popup_selection()` reads
  `indexOfSelectedItem`; returns -1 for non-popup.
- ■ *unit* `Element::set_popup_selection(idx)` writes
  `selectItemAtIndex:`; same-value writes diff and skip.
- ■ *unit* `Element::set_popup_items(&[...])` populates the popup
  via `addItemWithTitle:`; resets selection per AppKit default.

### Threading
- □ *integration* Calling any method on a Node from a non-main thread
  panics from the SendWrapper guard. (Hard to write reliably without
  a real second thread; mark as low-priority.)

## 2. tachys::renderer::cocoa

### Forwarders
- □ *unit* Every `Dom::*` method calls into `cocoa_dom::Renderer::*`
  with the same arguments and return type. (Smoke tests, one per
  method.)

### `Mountable` impls
- □ *unit* `Element::mount(parent, None)` adds Element as last child.
- □ *unit* `Element::mount(parent, Some(marker))` places Element
  before marker.
- □ *unit* `Element::unmount()` removes from superview.
- □ *unit* `Text::mount` / `unmount` same.
- □ *unit* `Placeholder::mount` / `unmount` same.
- □ *unit* `Node::elements()` returns empty vec (we don't recover the
  typed Element from a generic Node).
- □ *unit* `Element::elements()` returns Vec containing self.
- □ *unit* `insert_before_this` returns false (currently unimplemented
  on native — make sure the false return doesn't break dynamic children
  during Stage 4+).

### `mount_before` / `try_mount_before`
- □ *unit* `mount_before` mounts a new child as preceding sibling of
  `before`.
- □ *unit* `mount_before` panics when `before` has no superview.
- □ *unit* `try_mount_before` returns true on success, mounts.
- □ *unit* `try_mount_before` returns false when no superview, leaves
  child unmounted.

### `CastFrom`
- □ *unit* `CastFrom<Node>::cast_from(elem_node)` returns Some(Element).
- □ *unit* Cross-kind casts return None.
- □ *unit* `CastFrom<Element> for Element` is the identity.

### Hydration stubs
- □ *unit* `Renderer::get_parent` panics with the expected error
  message containing "hydration is not supported on the native target".
- □ *unit* Same for `first_child`, `next_sibling`.
- □ *unit* `failed_to_cast_text_node` / `_marker_node` /
  `_element` panic with the expected diagnostic.

## 3. cocoa_dom::spawner

### Lifecycle
- □ *unit* `spawner::init()` succeeds the first time.
- □ *unit* Second call returns `Err(ExecutorError::AlreadySet)` and
  doesn't disrupt state.
- □ *unit* After init, `Executor::spawn_local(future)` runs the future
  on the main queue.

### Future polling
- □ *integration* `spawn_local(async { /* ready immediately */ })`
  completes on the next dispatch tick.
- □ *integration* `spawn_local` of a future that yields once
  (Pending → wake → Ready) completes after two dispatch ticks.
- □ *integration* `spawn(send_future)` works the same as `spawn_local`
  (since we're single-threaded).
- □ *integration* Many small futures spawned concurrently all
  complete; none are dropped.
- □ *integration* A future that never completes (just yields forever)
  doesn't leak / spin the CPU between ticks.

### Waker behaviour
- □ *integration* Calling `wake()` between two polls causes exactly
  one re-poll (coalescing).
- □ *integration* Calling `wake()` from within the future's `poll`
  itself enqueues a follow-up poll (the `queued.store(false)` ordering
  contract).
- □ *integration* `wake_by_ref` works the same as `wake`.
- □ *integration* Dropping the Waker doesn't crash (Arc count
  decrement only).

### Reactive integration
- □ *integration* `Effect::new(|_| count.get())` actually fires after
  `count.set(...)`. This is the smoke test that proves spawner ↔
  reactive_graph wiring is alive.
- □ *integration* Effect bodies that read multiple signals
  re-subscribe each run.
- □ *integration* `Effect::new` declared inside a component is dropped
  when the component is unmounted (no zombie effects firing on
  detached views).

## 4. cocoa_dom::event

### `ActionTarget`
- ■ *unit* Constructing an `ActionTarget` with a closure works.
- □ *action* `actionFired:` invokes the stored closure exactly once
  per call.
- □ *action* The closure can mutate captured state across multiple
  invocations (it's `FnMut`).
- □ *unit* Reentrant call (closure A's body invokes another action
  that calls into the *same* ActionTarget) doesn't deadlock the
  RefCell — current behaviour is "skip with eprintln". Test that we
  observe exactly one fire and a log line, not a panic.

### `on_control_action` (covers buttons, sliders, popups)
- ■ *action* After `on_control_action(button, cb)`,
  `fire_action(&button)` calls `cb`.
- ■ *unit* The control's `target` is the ActionTarget, `action` is
  `actionFired:`.
- ■ *action* Calling `on_control_action` twice on the same control
  with different closures: only the second one fires (NSControl
  single-target/action). The first stays in the retain-store as a
  leak. When we add fan-out, test both fire.
- □ *action* Closure that captures and mutates a signal updates the
  signal correctly.
- ■ *action* `on_control_action` on an NSSlider (not NSButton) wires
  correctly — regression guard for the original "slider doesn't
  respond" bug where `on_click` downcasted to NSButton.

### `TextFieldDelegate` (fan-out for `controlTextDidChange:` and
### `controlTextDidEndEditing:`)
- ■ *unit* First `on_text_field_change(field, cb)` constructs a
  `TextFieldDelegate` and installs it on the field via `setDelegate:`.
- ■ *unit* Second `on_text_field_change` on the same field appends to
  the existing handler list — does NOT replace the delegate.
- ■ *action* All installed `on_input` callbacks fire (in install order)
  on `controlTextDidChange:`.
- ■ *action* All installed `on_change` callbacks fire on
  `controlTextDidEndEditing:`.
- ■ *action* `on_input` and `on_change` handlers coexist on the same
  field (e.g. `bind:value` + `on:input` + `on:change`).
- □ *action* Reentrant call from inside a callback skips with a debug
  warning rather than panicking on the RefCell borrow.
- □ *unit* `drop_handlers_for(field.as_view())` removes the
  `TextFieldDelegate` from the registry; field's delegate slot may
  retain it weakly until the field deallocates.

### Handler registry
- □ *unit* `keep_target_alive(view, target)` increments the registry's
  entry count.
- □ *unit* `drop_handlers_for(view)` removes the entry.
- □ *unit* Multiple handlers on the same view all retained
  (Vec<Retained<...>>).
- □ *integration* Handlers retained across many GC cycles don't get
  dropped early. (Hard to construct meaningfully — mostly a Stage-3
  TODO that goes away when Mountable::unmount cleans up properly.)

### `Element::on_click`
- □ *action* Wired correctly on a button (delegates to `on_button_click`).
- □ *unit* Called on a non-button (e.g. label) is a silent no-op,
  doesn't panic.

## 5. cocoa_dom::app

- □ *integration* `run_app` initialises spawner before invoking the
  build closure (test: spawn a future inside the build closure; verify
  it runs).
- □ *integration* The window has the title and size passed in.
- □ *integration* The build closure's returned Element is the window's
  contentView.
- □ *integration* `app.run()` blocks until termination (need to send a
  programmatic terminate from the test).

## 6. Stage 4 — Taffy layout (planned)

### Style attribute mapping
For each supported CSS property, test that the cocoa_dom attribute
setter translates to the right Taffy `Style` field:
- □ *unit* `width=N` (px) → `Style.size.width = Length(N)`.
- □ *unit* `width=N%` → `Style.size.width = Percent(N/100)`.
- □ *unit* `width=auto` → `Style.size.width = Auto`.
- □ *unit* same for `height`, `min_width`, `max_width`, `min_height`,
  `max_height`.
- □ *unit* `padding` (1, 2, or 4 args).
- □ *unit* `margin` (same).
- □ *unit* `flex_direction=row` / `column` / `row_reverse` /
  `column_reverse`.
- □ *unit* `flex_wrap=wrap` / `nowrap`.
- □ *unit* `justify_content` for each value.
- □ *unit* `align_items` for each value.
- □ *unit* `gap=N`.
- □ *unit* `flex_grow=N`, `flex_shrink=N`, `flex_basis=...`.
- □ *unit* `position=absolute` / `relative`, `top` / `left` / etc.
- □ *unit* Setting an unrecognised CSS-like attribute is silent.

### Layout computation
- ■ *layout* Single-element root: frame matches available size.
- ■ *layout* Two children, row direction: frames placed
  side-by-side, total width matches.
- ■ *layout* Two children, column direction: frames stacked, total
  height matches.
- □ *layout* `justify_content=space_between` distributes correctly.
- ■ *layout* Padding shrinks children's frames inward by the right
  amount.
- ■ *layout* `gap` separates children correctly.
- ■ *layout* Nested containers: parent's frame contains all children;
  children of inner container fit within inner's frame.
- ■ *layout* `flex_grow` distributes leftover space proportionally.

### Reflow on tree mutation
- □ *layout* Inserting a child triggers a re-layout; existing siblings
  reposition correctly.
- □ *layout* Removing a child reflows.
- □ *layout* Reordering children reflows.
- □ *layout* Setting an attribute that affects layout (e.g. width)
  reflows.
- □ *layout* Setting a non-layout attribute (e.g. title) does not
  reflow (perf — verify via a counter).

### Window resize
- □ *integration* Resizing the host window triggers a re-layout
  before the next paint.
- □ *integration* Layout invariants hold after resize (no overlapping
  children, no negative frames).

### Edge cases
- ■ *layout* Zero-child container: parent frame is correct size, no
  panics.
- □ *layout* Container with all-hidden children: same.
- ■ *layout* Negative or zero available size: layout produces zero
  frames, no panics.
- □ *layout* Very deep nesting (100 levels): no stack overflow.

## 7. Stage 5 — Element builders + view! macro (planned)

### Element builders
For each element type (`view`, `button`, `label`, `text_field`,
`secure_text_field`, `checkbox`, `radio_button`, `slider`, `stepper`,
`pop_up_button`, `combo_box`, `segmented_control`, `date_picker`,
`color_well`, `image_view`, `stack_view`, `scroll_view`):
- □ *unit* Builder fn returns an HtmlElement-shaped wrapper around the
  right NSView subclass.
- □ *unit* Each typed attribute method (e.g. `.title("...")`,
  `.min_value(0.0)`) sets the right NSControl property.
- □ *unit* Calling `.child(...)` adds a subview.
- □ *unit* `.children([...])` adds multiple in order.
- □ *unit* `.on(ev::click, |_| ...)` wires an event handler.
- □ *unit* Builder produces a `Render` impl whose `build()` and
  `rebuild()` work as expected.

### `view!` macro
- □ *unit* Lowercase tag `<button>` resolves to the `button()` builder.
- □ *unit* Snake_case tag `<text_field>` resolves to `text_field()`.
- □ *unit* PascalCase tag `<MyComponent />` resolves to component
  invocation.
- □ *unit* Children are added in source order.
- □ *unit* Inline expressions `{expr}` produce dynamic text nodes.
- □ *unit* `on:click=closure` wires the right event.
- □ *unit* Dynamic attributes `class:foo=signal` (if we keep the
  class infra) toggle correctly.
- □ *unit* `bind:value=signal` works (after Stage 5+ — see below).
- □ *unit* Attribute spreading (if supported).
- □ *unit* Conditional children via `<Show when=...>`.
- □ *unit* Iteration via `<For each=...>`.

### Components & props
- □ *unit* Component fn called once per use site.
- □ *unit* Props arrive correctly typed.
- □ *unit* `children` prop (the `Children` type) renders correctly.
- □ *unit* Component cleanup: signals/effects created inside are
  dropped on unmount.
- □ *unit* Nested components: parent's rebuild doesn't recreate
  children unnecessarily.

### `mount_to_window`
- □ *integration* Returns an `UnmountHandle`.
- □ *integration* Dropping the handle unmounts the view tree and
  cancels effects.
- □ *integration* `forget()` keeps the tree mounted.
- □ *integration* Multiple windows can be mounted independently
  (eventual goal).

### `IntoView`
- □ *unit* `&str` → text node.
- □ *unit* `String` → text node.
- □ *unit* Primitives (`i32`, `f64`, `bool`, …) → text nodes with
  default formatting.
- □ *unit* Tuples `(a, b, c)` → fragment in order.
- □ *unit* `Option<T: IntoView>` → renders T or nothing.
- □ *unit* `Result<T, E>` → renders T or hits ErrorBoundary.
- □ *unit* `Vec<T: IntoView>` → fragment, but warns / errors if not
  keyed (HTML-style guidance).

## 8. Stage 5+ — `bind:` rebuild (planned)

For each NSControl-derived element with a `BindValue<V>` impl:

### Common contract
- □ *integration* Initial signal value populates the control before
  first paint.
- □ *integration* User input on the control updates the signal.
- □ *integration* `signal.set(...)` updates the control.
- □ *integration* Re-entrant updates don't cause infinite loops
  (signal change firing in response to its own set).
- □ *integration* Effect cleanup on unmount: programmatically setting
  the signal after unmount does NOT mutate the (now detached) view.
- □ *integration* Two `bind:` to the same signal stay in sync with
  each other.

### Per control type
- □ `text_field bind:value: String` — debounce on rapid typing? decide.
- □ `secure_text_field bind:value: String` — same.
- □ `checkbox bind:state: bool` — toggle propagates.
- □ `radio_button bind:state: bool` — group exclusivity? (separate
  binding for the group state.)
- □ `slider bind:value: f64` — continuous mode events fire; respect
  min/max.
- □ `stepper bind:value: f64` — integer-mode rounding.
- □ `pop_up_button bind:selection: usize` (or generic `T`).
- □ `combo_box bind:value: String`.
- □ `segmented_control bind:selection: usize`.
- □ `date_picker bind:value: NSDate / chrono / icu?` (decide type).
- □ `color_well bind:color: NSColor`.

## 9. Stage 6 — Real example tests (planned)

### `counters` example
- ■ *integration* Initial render: zero counters, "Add Counter" button
  visible.
- ■ *integration* Click "Add Counter": new row appears with its own
  counter at 0.
- ■ *integration* Each row's `+` and `−` buttons mutate only that
  row's count.
- □ *integration* "Remove" on a row removes only that row.
- □ *integration* Removing a middle row keeps the others in order with
  their state intact (keyed iteration test).
- □ *integration* Adding 1000 rows doesn't choke the run loop
  (perf smoke test).

### `todomvc`-style example (if added)
- Add/edit/delete items, filter by state, keyed reorder, persistence.

## 10. Cross-cutting

### Memory
- □ *unit* `Element` clone semantics: cheap retain, both clones point
  at the same NSView (verify with `ptr_eq`).
- □ *unit* Dropping all clones of an Element drops the NSView's
  refcount to its expected value (it may still be retained by AppKit,
  e.g. as a subview).
- □ *integration* Mounting + unmounting a view tree N times doesn't
  grow allocations linearly (no leak detector built-in; eyeball with
  `leaks(1)` or instruments).
- □ *integration* Closing a window with active effects/signals drops
  them cleanly (no use-after-free, no zombie effect fires).
- □ *integration* Handler registry size returns to baseline after
  unmount (after we wire `drop_handlers_for` into Mountable::unmount).

### Threading
- □ *integration* Touching a Node from a worker thread panics from
  SendWrapper.
- □ *integration* `dispatch_async` from a worker thread to schedule
  a future works (the spawner contract).
- □ *integration* Calling `Executor::spawn` from a worker thread (the
  Send-future path) is safe.

### Performance smoke
- □ *integration* 10k signal updates per second don't stall the run
  loop (effects coalesce).
- □ *integration* Large list (1k items) renders within a budget
  (define one).
- □ *integration* Re-layout after insert/remove on a 1k-item list is
  bounded.

### Error paths
- □ *unit* Passing an invalid frame size (NaN, infinity) doesn't crash
  AppKit — test our `set_frame` boundary.
- □ *unit* Hydration stubs panic with messages that mention
  `implementation_log.md`.
- □ *unit* `mount_before` on an orphan Node panics with a clear error.

## 11. XCUIAutomation tier (deferred to after Stage 5)

When we eventually add this:

### Setup
- □ Wrap the binary as a `.app` bundle (Info.plist with
  `LSUIElement=NO`, accessibility entitlements).
- □ Add a small Xcode project / Swift test target.
- □ Wire `xcodebuild test` into CI as a separate macOS-runners job.

### What XCUIAutomation tests would cover that lower tiers can't
- □ Real keyboard input into a text_field (typeText, deleteKey,
  modifier keys).
- □ Tab navigation between fields (NSWindow first-responder chain).
- □ Mouse-down / mouse-up / drag (sliders, selection).
- □ Right-click / context menus.
- □ Window-level events: resize, miniaturize, close, fullscreen.
- □ Multi-window behaviour: bringing window forward, key-window swap.
- □ Accessibility tree validity (VoiceOver labels).
- □ Screenshot-based visual regression on the demo apps.
- □ "Did NSApplication ever beep?" (modal alert, system error).

## 12. Test infrastructure decisions still open

- □ Where do tests live? `cocoa_dom/tests/`? Per-stage subdirectories?
  A separate `tests/` workspace member?
- □ Do we adopt `insta` for snapshot tests of NSView trees /
  layouts? It's already a workspace dep.
- □ How do we run the AppKit run loop in tests without blocking the
  test runner? `run_for(Duration)` helper that starts the run loop,
  posts events, and stops it after a timeout.
- □ CI: macOS-only runners for the AppKit-touching subset. What's the
  baseline test set that has to pass on every PR?
- □ Do we generate test coverage reports? Tarpaulin doesn't work great
  on macOS; investigate `cargo-llvm-cov`.


## 13. Events: `on:event=handler` (added since original plan)

### `EventDescriptor` + `PendingHandler`
- ■ *unit* `ClickEvent::into_pending(cb)` produces
  `PendingHandler::Click(_)`.
- ■ *unit* `InputEvent::into_pending(cb)` produces
  `PendingHandler::Input(_)`.
- ■ *unit* `ChangeEvent::into_pending(cb)` produces
  `PendingHandler::Change(_)`.
- ■ *unit* `PendingHandler::apply_to(el)` routes each variant to the
  correct cocoa_dom hook (Click → on_click, Input → on_text_change,
  Change → on_text_end_editing).

### `SupportsEvent<E>` compile-time pairing checks
- ■ *compile-pass* `<button on:click=...>` compiles.
- ■ *compile-pass* `<checkbox on:click=...>` compiles.
- ■ *compile-pass* `<text_field on:input=...>` compiles.
- ■ *compile-pass* `<text_field on:change=...>` compiles.
- ■ *compile-pass* `<secure_text_field on:input=...>` compiles.
- □ *compile-fail* `<button on:input=...>` fails with a trait-bound
  error pointing at `Button::on`.
- □ *compile-fail* `<text_field on:click=...>` fails.
- □ *compile-fail* `<slider on:click=...>` fails (slider currently
  has no SupportsEvent impls).
- □ *compile-fail* `<pop_up_button on:click=...>` fails.

(Use `trybuild` for compile-fail tests, gated on
`cfg(target_os = "macos")`.)

### Inline event installation flow
- ■ *action* Builder `.on(click, cb)` stashes a `PendingHandler`,
  then `Render::build` drains and installs against the constructed
  Element. Verify `cb` fires when the underlying NSControl's action
  is invoked.
- ■ *unit* Multiple `.on(...)` calls on a builder push multiple
  `PendingHandler`s; build installs all in order.

### Spread-attribute (`{..attr}`) path
- □ *action* `let attr = on(click, cb); view!{ <button {..attr}/> }`
  installs the click handler.
- □ *unit* `OnAttribute::take_pending` returns Some on first call,
  None thereafter.

### `on:input` semantics
- ■ *action* Typing into a text_field with `on:input=cb` fires `cb`
  with the new value on each keystroke.
- ■ *action* `on:input` coexists with `bind:value`: setter fires
  AND `cb` fires.

### `on:change` semantics
- ■ *action* Pressing return / blurring a text_field with
  `on:change=cb` fires `cb` with the committed value.
- □ *action* Programmatic value changes do NOT fire on:change
  (commit semantics — only user-initiated commits trigger it).

## 14. `bind:` (added since original plan)

### `IntoSignal<T>`
- ■ *unit* `RwSignal<T>` impls `IntoSignal<T>` for `T: Send + Sync + Clone`.
- ■ *unit* `(getter, setter)` tuple impls `IntoSignal<T>`.
- □ *unit* `into_get()` returns a `Fn() -> T` boxed closure that, when
  called inside an Effect, subscribes to the signal.
- □ *unit* `into_set()` returns a `FnMut(T)` boxed closure that
  updates the signal.

### TextField — `bind:value=String_signal`
- ■ *integration* Initial signal value populates the field.
- ■ *integration* User typing pushes new value into the signal
  (`on_text_change` → setter).
- ■ *integration* `signal.set("X")` updates the field's stringValue.
- ■ *integration* No focus-ring flash on first keystroke (the
  `set_string_attribute` diff guard prevents the redundant write
  loop).
- □ *integration* Two `bind:value` to the same signal stay in sync.

### Checkbox — `bind:checked=bool_signal`
- ■ *integration* Initial signal value sets button state.
- ■ *integration* User click toggles signal.
- ■ *integration* `signal.set(true)` checks the box.

### Slider — `bind:value=f64_signal`
- ■ *integration* Initial signal value sets slider position.
- ■ *integration* Drag fires action target/action via NSControl,
  pushes new doubleValue into signal.
- ■ *integration* Continuous drag generates many setter calls (one
  per AppKit drag step).
- ■ *integration* `signal.set(50.0)` repositions the slider.

### PopUpButton — `bind:selection=usize_signal`
- ■ *integration* Initial signal value selects the right item.
- ■ *integration* User picks a new item → signal updates.
- ■ *integration* `signal.set(2)` selects index 2.

### Effect lifecycle
- □ *integration* The `RenderEffect` returned by `install_*_bind`
  unsubscribes when the element's State drops.
- □ *integration* Programmatically setting the signal AFTER unmount
  doesn't mutate the (now-detached) view.

### `Selection` AttributeKey re-export
- □ *unit* `tachys::html::attribute::Selection` resolves (the macro
  emits this path for `bind:selection=...`).
- □ *unit* `cocoa_dom::cocoa::bind::Selection` and the re-export are
  the same type.

## 15. Element builders (post-Stage-5 additions)

### Button
- ■ *unit* `.title("X")` sets the title via
  `set_string_attribute(StringAttr::Title, "X")`.
- ■ *unit* `.enabled(true/false)` toggles `NSControl::isEnabled`.
- ■ *unit* `.enabled(closure)` installs an Effect that re-fires on
  signal change.
- ■ *unit* `.on_click(cb)` and `.on(click, cb)` are equivalent.

### Checkbox
- □ *unit* `.title("X")` sets the title.
- ■ *unit* `.checked(true)` (one-way, static) sets initial state.
- ■ *unit* `.checked(closure)` (one-way, reactive) drives state from
  signal — but does NOT push back.
- ■ *unit* `bind:checked=signal` is the two-way form.

### TextField / SecureTextField
- ■ *unit* `secure_text_field()` builds an NSSecureTextField; same
  bind / event plumbing works.
- ■ *unit* `.placeholder("X")` sets `setPlaceholderString:`.
- ■ *unit* `.value("X")` sets initial value (one-way).
- ■ *unit* `bind:value=signal` is the two-way form.

### Slider
- ■ *unit* `.min_value(N)` / `.max_value(N)` set bounds.
- ■ *unit* `.value(N)` sets initial position (one-way).
- ■ *unit* Slider's `Render::build` calls
  `el.set_slider_min/max` BEFORE installing the value
  Effect (so the initial setDoubleValue clamps correctly).

### PopUpButton
- ■ *unit* `.items(vec!["A", "B"])` populates via
  `set_popup_items(&[...])`.
- ■ *unit* `.items(vec![String])` (owned strings) also works.
- ■ *unit* `.selection(usize)` sets initial selection (one-way).
- ■ *unit* Build order: items installed before selection (selection
  is meaningless without items).

## 16. `cocoa_dom::app` Edit menu (added)

- ■ *integration* Main menu has both "App" and "Edit" submenus.
- ■ *integration* Edit menu items have correct selectors:
  Undo→`undo:`, Redo→`redo:`, Cut→`cut:`, Copy→`copy:`,
  Paste→`paste:`, Delete→`delete:`, Select All→`selectAll:`.
- ■ *integration* Each Edit item has `target == nil` (responder-chain
  dispatch).
- ■ *integration* "Redo" has Cmd-Shift-Z modifier; others have Cmd
  only.
- □ *XCUIAutomation* Cmd+A in a focused text_field selects all.
- □ *XCUIAutomation* Cmd+C / Cmd+V round-trips text via the system
  pasteboard.

## 17. Window cleanup (`windowWillClose:` → unmount)

- □ *unit* `WindowDelegate::install_close_handler(cb)` returns the
  previous closure (None on first install).
- □ *unit* Calling `install_close_handler` twice replaces but does
  NOT call the prior closure.
- □ *integration* Closing the window fires the close handler exactly
  once; subsequent close attempts are no-ops.
- □ *integration* `WindowState::build` moves children into a close
  handler that calls `children.unmount()`.
- □ *integration* After the close handler runs, the per-view handler
  registries (`HANDLER_STORE`, `TEXT_FIELD_STORE`) no longer contain
  entries for the window's content tree.
- □ *integration* `WindowState::unmount` calls
  `nswindow.close()` which fires the close handler — i.e. unmount
  via the AppKit code path is exercised.
