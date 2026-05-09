# Test plan — Leptos Linux/GTK port

A running checklist of everything in the GTK port that needs automated
test coverage. Point `□` items are not yet covered; `■` are.

For the macOS sibling plan see [`tests.md`](./tests.md). The GTK port
mirrors the macOS port's shape; this document calls out the differences
(no Taffy, signal-based events, glib spawner, etc.).

## How to read this

- **Type** indicates the cheapest test that would catch regressions:
  - *unit* — pure-Rust assertion, no GTK init, no window. Builds
    structures and reads state. Requires `gtk::init()` (or `gtk4`
    test harness equivalent) for gobject type system to be available.
  - *action* — programmatically emit a GTK signal (e.g.
    `button.emit_clicked()`) to exercise handlers without a window
    or event loop.
  - *integration* — spin up a `GtkApplication`, open a hidden window,
    mutate signals, assert widget state. Higher cost; reserve for
    things the cheaper tiers can't catch.
  - *layout* — build a tree, assert child ordering and widget
    properties (orientation, spacing, expand flags). GTK self-lays-
    out so we don't compute frames, but we do verify parent-child
    structure and Box properties.
- **Why it matters** is included when it's not obvious. If a regression
  here lands silently, what's the symptom?

## 0. Test infrastructure to build first

Before writing the per-module tests below, we need a small set of
helpers. These are "stage 0" of the testing work.

- □ **GTK init helper.** GTK class methods (`Button::new()`, etc.)
  require `gtk::init()` to have run. Provide a `test_init()` that
  calls `gtk::init()` once (idempotent) plus sets up the spawner
  via `gtk_dom::spawner::init()`. Tests run sequentially on the
  main thread (`--test-threads=1` + `#[test]` with an
  `AssertUnwindSafe` harness or the custom `run_tests!` macro
  pattern from cocoa_dom).
- □ **Custom main-thread harness.** Like cocoa_dom's `common/mod.rs`.
  GTK4 requires all widget manipulation on the main thread. We
  need: `test_init()`, a `run_tests(&[...])` entry point for
  `harness = false` test binaries, and signal-firing helpers.
- □ **Signal emitter helpers.** `fire_clicked(&button)` calls
  `button.emit_clicked()`. `fire_text_changed(&entry, "text")`
  sets text and emits the `changed` signal. `fire_activate(&entry)`
  emits `activate`. These exercise handlers without a window or
  event loop — equivalent to cocoa_dom's `fire_action` /
  `fire_text_did_change`.
- □ **Reactive scope helper.** `with_owner(|| { ... })` that creates
  and forgets an `Owner`, runs the test body, then drops the Owner
  cleanly. Needed for any test that touches signals or effects.
- □ **Run-loop pump.** `pump_gtk()` that iterates the GTK main
  context for a few cycles so any `spawn_local`-dispatched effects
  fire. Equivalent to cocoa_dom's `pump_run_loop`. On GTK this is
  `while g_main_context_pending() { g_main_context_iteration(...) }`
  or the glib-rs equivalent: `MainContext::default().iteration(false)`.
  Call this after `signal.set(...)` in integration tests to let the
  Effect chain catch up before assertions.
- □ **Headless GTK decision.** Can we run GTK tests without a
  display? `GTK4` + `GDK_BACKEND=x11` / `wayland` may require
  `xvfb-run` or `weston-headless`. Document the CI requirement.
  Alternatively use `GDK_DEBUG=no-portals` + `--headless` flags.

## 1. gtk_dom::node

### Element creation
- □ *unit* `Element::create("button")` produces a `gtk::Button`. Verify
  via `widget.downcast_ref::<gtk4::Button>().is_some()` and
  `kind() == NodeKind::Element`.
- □ *unit* `Element::create("checkbox")` produces a `gtk::CheckButton`.
- □ *unit* `Element::create("label")` produces a `gtk::Label` with no
  initial text (empty string).
- □ *unit* `Element::create("text_field")` produces an editable
  `gtk::Entry`.
- □ *unit* `Element::create("secure_text_field")` produces a
  `gtk::PasswordEntry`. Also verifies PasswordEntry IS-A Entry (IS-A
  Editable).
- □ *unit* `Element::create("slider")` produces a horizontal `gtk::Scale`.
- □ *unit* `Element::create("pop_up_button")` produces a `gtk::DropDown`.
- □ *unit* `Element::create("vstack")` produces a `gtk::Box` with
  `Orientation::Vertical`.
- □ *unit* `Element::create("hstack")` produces a `gtk::Box` with
  `Orientation::Horizontal`.
- □ *unit* `Element::create("view")` (and unknown tags) falls back to a
  vertical `gtk::Box` container. `kind()` is `NodeKind::Element`.
- □ *unit* Tag names are case-sensitive — `"VSTACK"` doesn't resolve
  (falls through to default vertical Box).

### Text & Placeholder creation
- □ *unit* `Text::create("hello")` makes a `gtk::Label` with the given
  string and `kind() == NodeKind::Text`.
- □ *unit* `Text::create("")` works — empty string label.
- □ *unit* `Placeholder::create()` produces a hidden `gtk::Label` (not
  a Box — the label cannot accept children, which surfaces bugs if
  tachys incorrectly mounts content under a placeholder). `kind()` is
  `NodeKind::Placeholder`.
- □ *unit* Placeholder's hidden label is `set_visible(false)` — it
  doesn't take space in layout.

### Round-trip & casts
- □ *unit* `Element::into_node()` then `Element::from_node_unchecked()`
  yields equivalent Element (same gobject pointer via `ptr_eq`).
- □ *unit* `CastFrom<Node>::cast_from(node)` returns Some only when
  kind matches. Element rejected to Text returns None.
- □ *unit* `AsRef<Node>` for Element / Text / Placeholder borrows the
  inner Node without copying.
- □ *unit* `Node::ptr_eq` is true for clones of the same Node, false
  for differently-constructed ones.
- □ *unit* `from_node_unchecked` with a mismatched kind panics (not
  just debug — release too, per the assert_eq! in the impl).

### Tree mutation
- □ *unit* `insert_node(child, None)` on a `gtk::Box` appends.
- □ *unit* `insert_node(child, Some(marker))` inserts child immediately
  before the marker via `insert_child_after(marker.prev_sibling())`.
  Verify child ordering by walking `box_.first_child()` →
  `next_sibling()`.
- □ *unit* Inserting before a marker that's not a child of this parent
  returns false from `try_insert_node`.
- □ *unit* Inserting child == marker is a permitted no-op (returns true
  from `try_insert_node` without changing the tree).
- □ *unit* `insert_node` of a child that's already parented elsewhere
  calls `teardown()` on the old parent and re-parents.
- □ *unit* `insert_node` with marker on a `Window` / `ApplicationWindow`
  returns false (windows only take a single root child via `set_child`).
- □ *unit* `remove_child(child)` returns `Some(Node)` and unparents the
  widget. Calling `remove_child` for a non-child returns `None`.
- □ *unit* `remove_child` works on `gtk::Box`, `gtk::Window`, and
  `gtk::ApplicationWindow` (and by extension, any container via the
  universal `unparent()` path).
- □ *unit* `clear_children()` removes all children by walking the
  first-child chain and unparenting. Calling on an empty parent is a
  no-op.
- □ *unit* `insert_node` of a child already in the same parent at a
  different position reorders via `reorder_child_after`. Verify the
  widget moves, not clones.

### Attribute setters — string
- □ *unit* `set_attribute("title", "X")` on a `gtk::Button` calls
  `set_label("X")`. Same for `gtk::CheckButton`.
- □ *unit* `set_attribute("title", "X")` on a `gtk::Label` also
  routes through `set_label("X")` (even though Label doesn't have a
  "title" — it's the only text-bearing method on Label).
- □ *unit* `set_attribute("value", "X")` on a `gtk::Entry` calls
  `set_text("X")`. On a `gtk::Label`, calls `set_label("X")`.
- □ *unit* `set_attribute("placeholder", "X")` on a `gtk::Entry` or
  `gtk::PasswordEntry` calls `set_placeholder_text(Some("X"))`.
- □ *unit* `set_attribute` is a same-value no-op for each variant
  (title, value, placeholder). The diff-first check reads the widget's
  current value and skips the setter if unchanged. This is what
  prevents `bind:` cycles.
- □ *unit* `set_attribute` with an unknown attribute name is a silent
  no-op (no panic).

### Attribute setters — bool
- □ *unit* `set_bool_attribute("enabled", false)` calls
  `widget.set_sensitive(false)`. `true` → `set_sensitive(true)`.
  Same-value diff checks first.
- □ *unit* `set_bool_attribute("hidden", true)` calls
  `widget.set_visible(false)` (inverted). Same-value diff checks
  first.
- □ *unit* `set_bool_attribute("checked", true)` on a
  `gtk::CheckButton` calls `set_active(true)`. No-op on a non-check
  widget.
- □ *unit* `set_bool_attribute` with an unknown name is a silent
  no-op.

### Attribute removal
- □ *unit* `remove_attribute("title")` clears button label to `""`.
- □ *unit* `remove_attribute("value")` clears entry text to `""` and
  label text to `""`.
- □ *unit* `remove_attribute("placeholder")` calls
  `set_placeholder_text(None)`.
- □ *unit* `remove_attribute("hidden")` delegates to
  `set_bool_attribute("hidden", false)` (visible again).
- □ *unit* `remove_attribute("enabled")` delegates to
  `set_bool_attribute("enabled", true)`.
- □ *unit* `remove_attribute("checked")` delegates to
  `set_bool_attribute("checked", false)`.

### Event wiring (currently inline on Element; Stage 3 event.rs extraction)
- □ *action* `Element::on_click(cb)` on a `gtk::Button` — calling
  `fire_clicked` invokes `cb`. Multiple `on_click` calls stack
  (each `connect_clicked` appends an additional handler — unlike
  cocoa's single-target/action).
- □ *unit* `Element::on_click` on a non-button widget (label, box)
  is a silent no-op.
- □ *action* `Element::on_text_change(cb)` on a `gtk::Entry` —
  setting text and emitting `changed` invokes `cb` with the new
  value. Multiple calls stack.
- □ *unit* `Element::on_text_change` on a non-entry is a silent
  no-op.
- □ *action* `Element::on_text_activate(cb)` on a `gtk::Entry` —
  emitting `activate` invokes `cb` with the current text. Multiple
  calls stack.
- □ *action* `Element::on_action(cb)` on a `gtk::Scale` —
  emitting `value-changed` invokes `cb`. (Verify this, since
  `Scale::value-changed` only fires on user interaction, not
  programmatic `set_value` — the test must use
  `adjustment.value_changed()` or manually emit the signal.)
- □ *action* `Element::on_action(cb)` on a `gtk::CheckButton` —
  toggling the check state invokes `cb`.
- □ *action* `Element::on_action(cb)` on a `gtk::DropDown` —
  selecting a new item invokes `cb`.
- □ *unit* `Element::on_action` on an unrecognised widget is a
  silent no-op.

### Element value getters/setters
- □ *unit* `checked()` reads `CheckButton::is_active()`; returns
  false for non-checkbutton.
- □ *unit* `double_value()` reads `Scale::value()`; returns 0.0 for
  non-scale.
- □ *unit* `set_double_value(v)` writes `Scale::set_value(v)`;
  same-value writes diff and skip.
- □ *unit* `set_slider_min(v)` / `set_slider_max(v)` set the
  associated `Adjustment`'s bounds. No-op on non-scale.
- □ *unit* `popup_selection()` reads `DropDown::selected()` (u32);
  returns 0 for non-dropdown.
- □ *unit* `set_popup_selection(idx)` writes `DropDown::set_selected(idx)`;
  same-value writes diff and skip.
- □ *unit* `set_popup_items(&["A", "B"])` creates a `gtk::StringList`
  model and calls `DropDown::set_model(...)`. No-op on non-dropdown.

### Threading
- □ *integration* Touching a `Node` from a spawned thread panics via
  the `SendWrapper` guard. (Need `std::thread::spawn` + a
  `catch_unwind` across the thread boundary.)

## 2. tachys::renderer::gtk

### Forwarders
- □ *unit* Every `Dom::*` method calls into `gtk_dom::Renderer::*`
  with the same arguments. (Smoke: one per method, verifies the
  delegation chain.)

### Mountable impls
- □ *unit* `Element::mount(parent, None)` adds Element as last child.
- □ *unit* `Element::mount(parent, Some(marker))` places Element
  before marker.
- □ *unit* `Element::unmount()` calls `teardown()` — unparents the
  widget.
- □ *unit* `Text::mount` / `unmount` same.
- □ *unit* `Placeholder::mount` / `unmount` same.
- □ *unit* `Node::elements()` returns empty vec.
- □ *unit* `Element::elements()` returns vec containing self.
- □ *unit* `insert_before_this` delegates to
  `Dom::try_mount_before(child, self)` and returns the bool
  result. Verify child appears before self in parent's ordering.

### mount_before / try_mount_before
- □ *unit* `mount_before` mounts a new child as preceding sibling
  of `before`. Verify child ordering.
- □ *unit* `mount_before` panics when `before` has no parent widget
  (via the `.expect()` in `synthesise_parent_element`).
- □ *unit* `try_mount_before` returns true on success, mounts.
- □ *unit* `try_mount_before` returns false when no parent, leaves
  child unmounted.

### CastFrom
- □ *unit* `CastFrom<Node>::cast_from(elem_node)` returns
  `Some(Element)`.
- □ *unit* Cross-kind casts return `None`.
- □ *unit* `CastFrom<Element> for Element` is the identity.

### Hydration stubs
- □ *unit* `Renderer::get_parent` panics with the expected error
  message.
- □ *unit* Same for `first_child`, `next_sibling`.
- □ *unit* `get_template` / `clone_template` panic with "web-only
  optimization" message.

## 3. gtk_dom::spawner

### Lifecycle
- □ *unit* `spawner::init()` succeeds the first time.
- □ *unit* Second call returns `Err(ExecutorError::AlreadySet)` and
  doesn't disrupt state.
- □ *unit* After init, `Executor::spawn_local(future)` runs the
  future on the GTK main context.
- □ *integration* `spawn_local(async { ... })` inside
  `connect_activate` during `app.run()` runs the future.

### Future polling
- □ *integration* `spawn_local(async { /* ready immediately */ })`
  completes on the next main-loop iteration (after `pump_gtk()`).
- □ *integration* `spawn_local` of a future that yields once
  (Pending → wake → Ready) completes after two pump cycles.
- □ *integration* Many small futures spawned concurrently all
  complete; none are silently dropped.
- □ *integration* A future that never completes (just yields forever)
  doesn't leak — verify no unbounded accumulation of `JoinHandle`s
  or GSource objects.

### Waker behaviour
- □ *integration* Calling `wake()` between two polls causes exactly
  one re-poll (coalescing — the `glib::spawn_local` waker
  integration handles this).
- □ *integration* Dropping the Waker doesn't crash (Arc count
  decrement only).

### Reactive integration
- □ *integration* `Effect::new(|_| count.get())` actually fires after
  `count.set(...)` + `pump_gtk()`. This is the smoke test that
  proves spawner ↔ reactive_graph wiring is alive.
- □ *integration* `RenderEffect::new(|_| { apply(signal.get()); })`
  calls apply with the initial value synchronously (inside the
  constructor), then on every signal change after `pump_gtk()`.
- □ *integration* Effect bodies that read multiple signals
  re-subscribe each run — changing any signal re-fires.
- □ *integration* Dropping the `RenderEffect` stops it from firing
  on subsequent signal changes (the inner channel closes, the
  spawned future exits).

## 4. gtk_dom::app

- □ *integration* `init_app(id)` returns a `gtk::Application` with
  the given ID.
- □ *integration* The spawner is initialised before
  `connect_activate` fires — `Effect::new` works inside the
  callback.
- □ *integration* `open_window(app, title, size)` creates an
  `ApplicationWindow` with the given title and default size. The
  content root is a vertical `gtk::Box` installed as the window's
  child.
- □ *integration* `OpenedWindow::show()` calls `present()` and the
  window becomes visible.
- □ *integration* `OpenedWindow::close()` calls `gtk_window.close()`
  and triggers the GTK close lifecycle.
- □ *integration* `run_loop(&app)` blocks until `app.quit()` or the
  last window closes.

## 5. Layout via GTK natives (Stage 4 — mostly implicit now)

### Box properties from builder
- □ *unit* `Element::create("vstack")` → vertical Box. `.padding(16.0)`
  → `set_margin_{top,bottom,start,end}(16)`. `.gap(12.0)` →
  `set_spacing(12)`.
- □ *unit* `Element::create("hstack")` same, for horizontal.
- □ *unit* `flex_grow(1.0)` on any container calls
  `set_hexpand(true)` and `set_vexpand(true)`. `flex_grow(0.0)`
  is a no-op (GTK expand is binary, not weighted — the builder
  API accepts `f32` for cocoa parity but only truthiness matters).
- □ *unit* `flex_grow` on a leaf control (button, label) also
  applies expand flags.

### Child ordering after insert
- □ *unit* Inserting children A, B, C in order produces
  A → B → C in the Box child chain (`first_child()` →
  `next_sibling()`).
- □ *unit* Inserting C before B produces A → C → B.
- □ *unit* `clear_children` removes all children; iterating after
  shows zero children.

## 6. Stage 5 — Element builders + view! macro (implemented, not tested)

For each element type (`view`, `button`, `label`, `text_field`,
`secure_text_field`, `checkbox`, `slider`, `pop_up_button`,
`vstack`, `hstack`):

### Element builders
- □ *unit* Builder fn returns the struct with correct defaults
  (empty strings, false for bools, etc.).
- □ *unit* Each typed attribute method (`.title("X")`,
  `.checked(true)`, `.min_value(0.0)`, `.items(vec!["A"])`, etc.)
  sets the right field on the builder.
- □ *unit* `.child(value)` sets the text/title field (for leaf
  controls: button, label, checkbox). Calling repeatedly replaces
  (last-wins).
- □ *unit* `.child(node)` on a `View` container adds the child to
  the tuple chain.
- □ *unit* `.on(ev::click, cb)` pushes a `PendingHandler` onto the
  handler vec.
- □ *unit* `.add_any_attr(OnAttribute)` pushes the handler.
- □ *unit* Builder's `Render::build` creates the right
  `gtk_dom::Element` tag and installs all effects.
- □ *unit* `vstack()` / `hstack()` / `view()` produce Views with
  the correct `tag` field (`"vstack"` / `"hstack"` / `"view"`).
- □ *unit* `flex_grow` on button / slider / popup routes to
  `set_hexpand` / `set_vexpand` at build time.

### view! macro (end-to-end)
- □ *unit* Lowercase `<button>` resolves to `button()` builder.
- □ *unit* Snake_case `<text_field>` resolves to `text_field()`.
- □ *unit* PascalCase `<MyComponent />` resolves to component
  invocation.
- □ *unit* Children are added in source order.
- □ *unit* Inline expressions `{expr}` produce dynamic text nodes.
- □ *unit* `on:click=closure` wires the right event handler.
- □ *unit* `bind:value=signal` works (see section 7).
- □ *unit* `<hstack>` produces a horizontal box.
- □ *unit* `<vstack>` produces a vertical box.
- □ *unit* `<view>` as an SVG tag (`tachys::svg::view`) resolves
  to the GTK container via `svg_gtk.rs`.
- □ *unit* Attribute spreading `{..attr}` (if supported).

### Components & props
- □ *unit* `#[component]` fn called once per use site.
- □ *unit* Props arrive correctly typed.
- □ *unit* Component cleanup: signals/effects created inside are
  dropped on unmount (when `ElementState::unmount` is called).

### mount_to_window
- □ *integration* `mount_to_window(app_id, title, size, closure)`
  opens a window with the given title and size.
- □ *integration* The closure's returned view tree is mounted as the
  window's content.
- □ *integration* Clicking a button wired via `on:click` mutates a
  reactive signal.
- □ *integration* `RwSignal` changes propagate to label text
  (reactive update via `RenderEffect`).

### IntoView
- □ *unit* `&str` → text node.
- □ *unit* `String` → text node.
- □ *unit* Primitives (`i32`, `f64`, `bool`) → text nodes with
  default formatting.
- □ *unit* Tuples `(a, b, c)` → fragment in order.
- □ *unit* `Option<T: IntoView>` → renders T or nothing.
- □ *unit* `Vec<T: IntoView>` → fragment with items in order.

## 7. bind: (implemented, not tested)

For each widget with a `BindAttribute` impl:

### Common contract
- □ *integration* Initial signal value populates the control before
  first paint.
- □ *integration* User interaction on the control updates the signal.
- □ *integration* `signal.set(...)` updates the control.
- □ *integration* Re-entrant updates don't cause infinite loops
  (GTK4's programmatic-write-doesn't-fire-signal behavior, plus
  diff-first guards, prevent this).
- □ *integration* Effect cleanup on unmount: programmatically setting
  the signal after unmount does NOT mutate the (now detached)
  widget.
- □ *integration* Two controls bound to the same signal stay in
  sync with each other.

### Per control type
- □ `TextField bind:value: String` — user typing pushes to signal;
  `signal.set("X")` updates entry text. Diff-first prevents focus
  ring / cursor position flash on same-value writes.
- □ `Checkbox bind:checked: bool` — toggle propagates to signal;
  `signal.set(true)` checks the box.
- □ `Slider bind:value: f64` — drag updates signal continuously;
  `signal.set(0.5)` repositions slider.
- □ `PopUpButton bind:selection: usize` — pick updates signal;
  `signal.set(2)` selects index 2.
- □ `Label bind:value: String` — read-only sink; `signal.set("X")`
  updates label text. No outgoing leg.

### GTK-specific properties
- □ *integration* `Scale::value-changed` fires on user drag but NOT
  on programmatic `set_value` — no block/unblock needed for slider
  bind.
- □ *integration* `Entry::changed` fires on user keystrokes but NOT
  on programmatic `set_text` — no block/unblock needed for
  text_field bind.
- □ *integration* `CheckButton::toggled` fires on user click but
  NOT on programmatic `set_active` — same.
- □ *integration* `DropDown::notify::selected` — verify behavior
  on programmatic `set_selected` vs user pick. If it fires on both,
  check that the diff-first guard in `set_popup_selection` prevents
  cycles (this was flagged as a potential issue during review).

### IntoSignal<T>
- □ *unit* `RwSignal<T>` impls `IntoSignal<T>` for
  `T: Send + Sync + Clone`.
- □ *unit* `(getter, setter)` tuple impls `IntoSignal<T>`.
- □ *unit* `into_get()` returns a closure that, when called inside
  an Effect, subscribes to the signal.
- □ *unit* `into_set()` returns a closure that updates the signal.

### Selection AttributeKey
- □ *unit* `Selection::KEY` is `"selection"`.
- □ *unit* `tachys::html::attribute::Selection` resolves via the
  re-export in `tachys/src/html/attribute/mod.rs`.

## 8. Stage 6 — Dynamic children & real examples

### counters example
- □ *integration* Initial render: zero counters, "Add Counter" button
  visible.
- □ *integration* Click "Add Counter": new row appears with counter
  at 0. Each row has `+`/`-` buttons.
- □ *integration* Each row's buttons mutate only that row's count.
- □ *integration* "Remove" removes only that row; order preserved.
- □ *integration* Removing a middle row keeps others in order with
  state intact (keyed iteration via `<For>`).
- □ *integration* Adding 1000 rows doesn't stall the UI (perf smoke).

### Other examples
- □ *integration* `counter_gtk` — counter with view!{} + #[component]
  works as an integration test.
- □ *integration* `checkbox_gtk` — checkbox toggles, bind:checked
  propagates.
- □ *integration* `login_form_gtk` — text fields + bind:value, form
  submit.
- □ *integration* `settings_gtk` — slider + popup + bind:
  selection/value.

## 9. Cross-cutting

### Memory
- □ *unit* `Element` clone semantics: cheap gobject ref bump; both
  clones point at the same widget (verify with `ptr_eq`).
- □ *unit* Dropping all clones of an `Element` doesn't immediately
  destroy the widget if it's still parented (gobject ref-counting).
- □ *integration* Mounting + unmounting a view tree N times doesn't
  grow allocations linearly (rough check via object count or
  `valgrind`-style tool).
- □ *integration* Closing a window with active effects/signals drops
  them cleanly — verify no zombie effect fires after close.
- □ *integration* `Effect` / `RenderEffect` tasks are cleaned up
  when the element state is dropped (via `Mountable::unmount`).

### Threading
- □ *integration* Touching a `Node` from a worker thread panics via
  the `SendWrapper` guard.
- □ *integration* Calling `Executor::spawn` from a worker thread
  (Send-future path) works — the future runs on the main thread.

### Error paths
- □ *unit* Hydration stubs panic with messages mentioning "hydration
  is not supported on the native target".
- □ *unit* `mount_before` on an orphan Node panics with a clear
  error ("node has no parent").
- □ *unit* Creating an Element with an unknown tag produces a default
  vertical Box — no panic.

### Performance smoke
- □ *integration* 100 signal updates + pump each one — no stalls.
- □ *integration* Large widget tree (1000 nodes) builds within a
  reasonable budget.
- □ *integration* Insert/remove many children in a loop — no
  performance cliff.

## 10. Tests for features not yet implemented

These tests apply to planned work and should be filled in as each
feature lands. They're listed here so we remember to write tests
before closing the feature.

### Window lifecycle (Stage 5 extension)
- □ *integration* `Window::build` used inside `mount_gtk::run`
  creates a real `GtkApplicationWindow` — unify the two code
  paths (currently `mount_to_window` bypasses `Window::build`).
- □ *integration* Multi-window: two windows can be mounted
  independently, each with its own content root and reactive
  scope.
- □ *integration* Closing the last window quits the app (GTK default
  behavior via `GtkApplication`).

### Scroll views
- □ *unit* `<scroll_view>` tag maps to `gtk::ScrolledWindow`.
- □ *unit* Child of a scroll view is set via
  `ScrolledWindow::set_child`.
- □ *unit* Vertical/horizontal scrollbar policies are controllable
  via attributes.

### Grid layout
- □ *unit* `<grid>` tag maps to `gtk::Grid`. Children have
  row/column/spacing attributes.

### Text view (multi-line)
- □ *unit* `<text>` or `<text_view>` tag maps to `gtk::TextView`
  wrapped in a `gtk::ScrolledWindow`.

### Additional events
- □ *action* `on:focus` / `on:blur` events via GTK focus
  controllers.
- □ *action* `on:keydown` / `on:keyup` via `gtk::EventControllerKey`.
- □ *action* Keyboard shortcuts (accelerators) wired via
  `gtk::ShortcutController`.

### Accessibility
- □ *unit* All elements have accessible names/roles set by default
  (button = "push button", etc.). Verify via
  `widget.accessible_role()`.

### CSS styling
- □ *unit* `set_css_property(...)` routes to `gtk::StyleContext` or
  `gtk::CssProvider` (to be designed).

### Window menu bar (Stage 6+)
- □ *integration* GTK's GMenu / GMenuModel integration for
  application-wide menus. (GTK4 doesn't have a single global menu
  bar per se — this needs design.)

### Drag-and-drop
- □ *integration* `gtk::DragSource` / `gtk::DropTarget` wiring
  (likely deferred until post-GA).

## 11. Test infrastructure decisions still open

- □ Where do tests live? `gtk_dom/tests/`? A separate workspace
  member?
- □ Do we adopt the custom `run_tests!` harness from `cocoa_dom`
  (main-thread sequential runner with panicking test bodies) or
  use something simpler? GTK4's `#[test]` with `--test-threads=1`
  might suffice if we call `gtk::init()` once.
- □ Do we use `insta` for snapshot tests of widget trees?
- □ Headless GTK: `xvfb-run` / `weston-headless` / `gtk::init()`
  with `GDK_BACKEND=x11` in CI. Which approach is most CI-friendly?
- □ CI: Linux runners with GTK4 dev libs installed. What's the
  baseline test set that has to pass on every PR?
- □ Coverage: `cargo-llvm-cov` or `tarpaulin` on Linux.
- □ Can we write `trybuild` compile-fail tests for `view!{}` +
  `SupportsEvent<E>` the way `tests.md` suggests (e.g.
  `<button on:input=...>` should fail at compile time)? Gated on
  `cfg(target_os = "linux")`.

---

## Added 2026-05-09 — review pass

Gaps surfaced during the post-native-pivot codebase review.

### Status: gtk/leptos_gtk not in workspace
- The crate exists on disk but isn't a workspace member; `cargo
  check --workspace` does not touch it. **First step**: add to
  `workspace.members`, then the test plan below becomes runnable.

### Macro / build-time (shared with cocoa)
- □ *compile_fail* `#[island]` and `#[lazy]` macros are gone.

### Signal handler stacking (GTK-specific)
- □ Multiple `on:click` on one button: GTK *stacks* handlers (unlike
  cocoa's target/action which overwrites). Confirm the documented
  behaviour and add a test that fires one click and verifies all
  closures ran in registration order.

### Widget lifetime / closure drop
- □ Drop a `<button>` with a captured `RwSignal`. Verify the
  closure (and the `Rc` to the signal) is dropped (assert via
  `Weak::upgrade` on the signal, or via a `Drop`-guarded sentinel).
  Cocoa-side has the leak; GTK side claims to drop with the widget
  — pin that down with a regression test.

### gtk::Box layout assumptions
- □ `<view>` defaults to vertical orientation (cocoa is row).
  Document + test.
- □ `flex_grow` is binary on GTK — `0.0` -> no expand, anything
  positive -> `set_hexpand(true)`. Test both arms.

### Reactivity through glib main loop
- □ `RwSignal::set` triggers a `gtk::Widget` update on the next idle
  tick, not synchronously (depending on spawner config).
