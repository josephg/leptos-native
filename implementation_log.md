# Implementation log — Leptos macOS port

A running record of design decisions made during the macOS/Cocoa port,
especially the ones we deliberately deferred. Newest entries at the top.

---

## 2026-05-03 — Stage 6: dynamic children (counters example)

`examples/counters_macos` works end-to-end: Add/Clear buttons, `<For>`
keyed iteration of rows, each row has its own `+`/`-` buttons that
increment/decrement an isolated `RwSignal<i32>`. Getting there forced
four real fixes:

### 1. `mount_before` had no LayoutHandle propagation

`<For>` calls `Rndr::try_mount_before(item, marker)` to insert each
new row before a marker placeholder. Our impl synthesised a parent
`Element` wrapper around `marker.superview()` *without* a
`LayoutHandle` — the new row's NSView was added as a subview, but
the Taffy tree never learnt about it, so layout never assigned a
frame and rows piled at (0, 0).

Fix: derive the parent's `LayoutHandle` from `marker.layout_slot()`
using `tree.parent(marker.node_id)`. Added
`Node::from_view_with_handle` so the synthesised wrapper carries
the right handle. Now newly-mounted children register correctly in
the tree they belong to.

### 2. No layout pass triggered after tree mutations

Web Leptos relies on the browser's natural reflow when the DOM
changes. AppKit doesn't do this — Taffy is a passive library. After
`attach_child` / `insert_child_at` / `detach_child` mirrored a
mutation into Taffy, *nothing* re-ran `compute_layout`, so frames
stayed at their last-computed values.

Fix: each tree mutation calls `schedule_relayout_for_tree`, which
deduplicates via a thread-local `PENDING` set and dispatches a
single `compute_layout` pass on the next main-loop tick (via
`DispatchQueue::main().exec_async`). Coalesces N batched mutations
into one recompute.

### 3. Stored-root NodeId on each tree

The dispatched relayout closure needs to know which NSView to
recompute against. The first attempt walked up via
`tree.parent(captured_id)` to find the root — but `captured_id`
could be stale (its slot reused by a fresh `new_leaf_with_context`
between enqueue and dispatch), and `tree.parent(stale_id)` panics
("invalid SlotMap key used").

Fix: introduced `LayoutTree { tree, root: Option<NodeId> }`
(`TreeRef = Rc<LayoutTree>`). The root NodeId is set on the first
`register_in_tree` call against an empty tree (which is always the
contentView), and never changes. The dispatched closure reads
`tree.root` directly — no walking, no risk of stale ids.

This added `tree.tree.borrow_mut()` access patterns throughout
(extra `.tree`), but is a small constant overhead.

### 4. Content changes weren't invalidating Taffy's layout cache

After `set_text("-1")` or `set_attribute("title", "Reset")`, the
NSView's content changed but Taffy's cached layout for that node
was still valid from its perspective — Taffy doesn't observe our
external content. The measure callback was never re-invoked, the
label's frame stayed at its old size, "-1" was clipped to "-".

Fix: `set_text` and content-changing `set_attribute` paths now call
`schedule_relayout(node)`, which both `tree.mark_dirty(node_id)`
(invalidating the layout cache for that node and its ancestors) and
schedules the dispatched recompute.

### Validated

- `cocoa_dom/examples/counter.rs` — single counter, builders.
- `cocoa_dom/examples/counter_v2.rs` — single counter, tachys::cocoa.
- `cocoa_dom/examples/two_windows.rs` — multi-window.
- `cocoa_dom/examples/hello_window.rs` — low-level smoke.
- `examples/counter_macos/` — `view!{}` macro + `#[component]`.
- `examples/counters_macos/` — `<For>` keyed iteration, dynamic
  children, content-driven label resizing.

### Known gaps still

- `set_attribute` paths only schedule relayout for `title` / `value`
  / `hidden`. Other attributes (style updates) might also need it
  if/when we add reactive layout properties.
- `clear_children` doesn't trigger detach_child's relayout (we
  removed the per-child Taffy detach when we couldn't back-map
  NSView → Node). For now `<For>` always uses individual
  remove/insert so this isn't hit by current examples.
- Coalescing is per-tree, not per-frame: if the user dispatches
  multiple unrelated trees worth of mutations, each gets its own
  recompute. Fine for typical apps; could batch globally later.

---

## 2026-05-03 — Stage 5 part 3 (slices 2 + 3): IntoView, #[component], intrinsic sizing

### Slice 2: `IntoView` + `#[component]`

Stub `RenderHtml` + `AddAnyAttr` impls on each Cocoa element type
(Button, Label, TextField, View<Ch>) so they satisfy leptos's
`IntoView` blanket impl (`Render + RenderHtml + Send`). The stubs:
  - `to_html_with_buf`: empty (no SSR on native).
  - `hydrate`: delegates to `Render::build` (no real hydration).
  - `dry_resolve` / `resolve`: passthrough.
  - `into_owned`: returns self (each type is `'static`).
  - `MIN_LENGTH`: 0.

Lives in `tachys::cocoa::render_html_stub` as a macro
(`cocoa_stub_view_impls!`) for the non-generic types; `View<Ch>` has
its own hand-written impls (parametric over `Ch: Render + Send +
'static + RenderHtml`).

Send bounds added to closures throughout:
  - `MaybeReactive::Reactive(Box<dyn FnMut() -> T + Send + 'static>)`
  - `Button.on_click`, `Button::on_click(impl FnMut() + Send + 'static)`
  - `Button::on(EventDescriptor, FnMut + Send + 'static)`
  - `event::on(...)` / `OnAttribute.handler`

Result: functions can return `impl IntoView` and components can be
declared with `#[component]` and used inside `view!{}` as
`<MyComponent prop=value />`. Counter rewritten as a real
`#[component]` with prop.

### Slice 3: intrinsic-sized leaf controls

Replaced the hardcoded leaf sizes (button 80×32, label/text_field 22h)
with content-driven sizing via Taffy's measure callback:

  - **`TaffyTree<()>` → `TaffyTree<NodeContext>`.** New
    `cocoa_dom::layout::NodeContext { view: SendWrapper<Retained<NSView>> }`
    so the measure closure can call AppKit on each node.
  - **`compute_layout` → `compute_layout_with_measure`.** Closure
    dispatches per-NSView: for `NSControl`-derived views (Button,
    Label, TextField), call `sizeToFit` (which forces AppKit to
    compute proper bezel-inclusive size against the current title)
    and read `view.frame().size`. Restore the frame so we don't
    leave the view in a half-laid-out state. For non-control views
    (FlippedView containers, Placeholder), fall back to
    `intrinsicContentSize` (returns -1 for both axes → mapped to 0).
  - **`buttonWithTitle:target:action:`** instead of `initWithFrame:`.
    The default-init button gets a default bezel whose intrinsic
    content size is text-only without bezel padding; "Reset" rendered
    as "Rese". The `buttonWithTitle` constructor produces a proper
    rounded push button with sensible metrics. Title and target are
    set later via `set_attribute("title", ...)` and `on_click(...)`,
    which is fine since AppKit re-derives intrinsic on every
    `sizeToFit`.

Other layout tweaks in the same pass:
  - `Text::create_with` / `<label>` get `flex_shrink: 0.0` — text
    nodes hold their height under tight parent constraints. NSTextField
    doesn't clip its rendered content, so a frame shorter than the
    text height makes text overflow into siblings (the original
    "buttons cover up the label" symptom).
  - `COCOA_DOM_LAYOUT_DEBUG=1` env var toggles `[compute_layout]` /
    `[frame]` traces — no rebuild needed to flip on/off.

### Known perf debt: sizeToFit on every resize

`sizeToFit` is called from the measure closure on every leaf control
on every layout pass — and Taffy can call measure multiple times per
node per pass. Window resize fires `windowDidResize:` 60+ times per
second during a drag. So a slow drag can hit `sizeToFit` thousands of
times/sec.

Invisible for the 4-control counter; will matter at scale (a settings
pane with 50 controls, a list with hundreds of rows).

**Recommended fix when this bites:**
  1. Cache the measured size on the `NodeContext`. Invalidate when
     `set_attribute("title")` / `set_text()` mutates the underlying
     content. Most measures become hashmap lookups.
  2. Stop mutating frame in measure — use `NSButtonCell::cellSize`
     (or hold a reusable measurement-only NSCell) instead of
     `sizeToFit`.
  3. Coalesce resize events via `NSView::inLiveResize` — during live
     resize, recompute less frequently or use a cached layout and
     only fully re-layout on `viewDidEndLiveResize`.

---

## 2026-05-03 — Stage 5 part 3 (slice 1): view!{} macro working on macOS

**Decision.** Made `leptos_macro`'s unmodified `view!{}` macro work
against our Cocoa element builders by adding facade modules at the
paths the macro emits:

  - `tachys::html::element_macos` (re-exported as `tachys::html::element`
    on macOS) — provides `button`, `label`, `text_field`, `vstack`,
    `hstack`, `stack_view`, `div` (alias for view).
  - `tachys::html::event_macos` (re-exported as `tachys::html::event`
    on macOS) — provides `click` event descriptor, `on(...)` wrapper,
    `EventDescriptor` trait, `OnAttribute` and `HandlerKind` types.
  - `tachys::svg_macos` (re-exported as `tachys::svg` on macOS) — has
    a single `view` re-export so the macro's `<view>` (which it
    classifies as an SVG tag) routes to our generic flipped container.

Each cocoa element type grew the surface the macro emits:
  - `Button::child(impl IntoMaybeReactive<String>)` — sets title.
  - `Button::on(EventDescriptor, FnMut(EventType))` — `on:click=...`.
  - `Button::add_any_attr(OnAttribute)` — for spread-attribute syntax.
  - `Label::child(impl IntoMaybeReactive<String>)` — sets text.
  - `View::child(NewCh)` (already existed) — adds a child to the chain.

**Tag-name strategy chosen.** Snake_case tags. `<view>` aliases the
generic flipped NSView container. `<vstack>` / `<hstack>` are
SwiftUI-flavoured helpers that preset `flex_direction`. `<div>`
aliases `view` for users coming from web. PascalCase tags would have
required either a macro fork or wrapping every cocoa builder in a
`#[component]` (blocked by IntoView/RenderHtml).

**Macro attribute → builder method routing works.**
`<vstack padding=16.0 gap=12.0>` emits `.padding(16.0).gap(12.0)`
which our `View` builder already accepts. Numeric literals pass
through cleanly; no string parsing needed.

**Status.** Validated end-to-end with `examples/counter_macos`:

```rust
view! {
    <vstack padding=16.0 gap=12.0>
        <label>{move || format!("Count: {}", count.get())}</label>
        <hstack gap=8.0>
            <button on:click=move |_| count.update(|n| *n -= 1)>"-1"</button>
            <button on:click=move |_| count.set(0)>"Reset"</button>
            <button on:click=move |_| count.update(|n| *n += 1)>"+1"</button>
        </hstack>
    </vstack>
}
```

Compiles, runs, clicks update the label, resize reflows.

**Known gaps to address in follow-up slices:**

  - **`IntoView` trait still excludes our types** — requires
    `RenderHtml + Send` from leptos's IntoView definition. Functions
    returning `impl IntoView` don't compile; users must use
    `impl Render + 'static` or just pass directly to `mount_to_window`.
    This blocks `#[component]` (which requires IntoView).
  - **Only `on:click` works** — only event descriptor we expose.
    Adding more (`on:input`, `on:keydown`, etc.) needs both new
    `EventDescriptor` impls and matching hooks in `cocoa_dom::event`.
  - **No `class:`/`style:`/`prop:`/`bind:`** — those macro paths emit
    attributes we don't have facades for. Type errors on use
    (reasonable failure mode for unsupported features).
  - **Reactive non-text attributes**: `<button title=move || ...>` has
    a builder path (`.title(closure)`), but reactive numeric attrs
    (`<vstack padding=move || count.get()>`) aren't wired up — the
    macro emits `.padding(closure)` and `View::padding` only takes
    `f32`. Future: route through `IntoMaybeReactive<f32>`.
  - **Tag namespace clashes**: `<a>`, `<text>` are macro-classified
    differently than we'd want; could alias them through their
    correct macro-emitted paths if needed.

---

## 2026-05-03 — Multi-window: per-Window TaffyTree, deferred mount

**Decision.** Each window owns its own [`TaffyTree`]. `Window` is a
tachys `Render` type whose `build()` opens an NSWindow, creates a
fresh tree, registers a flipped contentView as the tree root, then
mounts the user's child Render under it. Multiple windows in the
same `mount::run` call (typically a tuple of `Window`s) each get
their own NSWindow + tree, fully isolated.

**The "no thread-local" implementation.** Each [`Node`] carries a
shared layout slot (`Rc<RefCell<NodeLayout>>` shared via `Clone`)
holding its current Taffy `Style` + an `Option<LayoutHandle>`. The
handle is `None` until the node joins a tree. Style setters mutate
the slot's `style`; if the handle is `Some`, they also push the
update into the handle's tree. There is no global TREE thread-local
and no NSView→Node back-mapping registry; effects firing later just
follow the handle they captured.

**The cascade — how children find their tree.** Tachys' `Render::build`
is no longer responsible for mounting children. Instead:

  - `View::build` (tachys::cocoa::element) builds children but does
    NOT mount them under `el`.
  - `ElementState::mount(parent, marker)` does two steps:
    1. `parent.insert_node(self.el, marker)` — also performs the
       Taffy registration if `parent` is in a tree (propagates
       parent's `LayoutHandle` to `self.el`).
    2. `self.children.mount(&self.el, None)` — recursive cascade.

  When `Window::build` later calls `children.mount(&content_root, None)`,
  the cascade walks the entire user tree top-down. At each level,
  `insert_node` sees a tree-having parent and registers the child in
  the same tree. No subview-walking, no back-mapping needed.

**Module reorganisation in cocoa_dom.**
  - New `cocoa_dom::window` — `WindowDelegate` (the resize-listener
    NSWindowDelegate) and `open_window(...)` / `OpenedWindow` helper
    that allocates the NSWindow, content_root, fresh tree, and
    delegate together.
  - `cocoa_dom::app` slimmed to just `init_app(mtm)` (NSApp, menu,
    AppDelegate, spawner) and `run_loop(&app)`. The old monolithic
    `run_app(title, size, build)` is gone — windows aren't owned by
    the app entry point anymore.

**`leptos::mount_macos`** now exposes:
  - `run(closure)` — generic; closure returns any `Render`. The
    typical use is to return one or more `Window`s.
  - `mount_to_window(title, size, closure)` — sugar that wraps the
    closure's return value in a single `Window` for the common case.

**Existing examples migrated:**
  - `cocoa_dom/examples/hello_window.rs` — uses `open_window` directly.
  - `cocoa_dom/examples/counter.rs` — uses `open_window` + manual frames.
  - `cocoa_dom/examples/counter_v2.rs` — uses `tachys::cocoa::window()` + builders.
  - `cocoa_dom/examples/two_windows.rs` (new) — opens two independent
    windows from a single `mount::run` call. Validated working
    (multi-window confirmed; resize works on both windows).
  - `examples/counter_macos/` — uses `leptos::prelude::*` with
    `mount_to_window`. Still works post-refactor.

**Known limitations:**
  - `Render::rebuild` is still a no-op for static attributes. Not
    tied to multi-window directly, but if we ever rebuild a `Window`
    (changing title/size on rebuild), nothing happens.
  - `mount_before` / `try_mount_before` in tachys::cocoa::Dom (used
    by tachys' dynamic-children/keyed-list code) synthesise a parent
    Element wrapper from the marker's superview — that wrapper has no
    `LayoutHandle`, so children inserted via this path won't be in
    the right Taffy tree until we add an NSView→Node back-mapping
    (or a per-window registry). No current example exercises this
    path; logged in the file's doc.
  - `clear_children` no longer can detach children from Taffy
    (no NSView→Node back-mapping). Children's Taffy entries get
    cleaned up via the `Mountable::unmount` chain instead. If
    `clear_children` is the only caller, entries leak until the
    surrounding subtree unmounts.

---

## 2026-05-03 — High-priority cleanup pass

Worked through the high-priority items from the review below. Status:

**Done in this pass:**

1. **Pin::new_unchecked unsoundness in spawner** (review #1) — fixed.
   Dropped the `Future for TaskFuture` impl with its unsafe pin
   projection. Replaced with a plain `TaskFuture::poll(&mut self, cx)`
   method that delegates to the inner `Pin<Box<dyn Future>>::as_mut()`
   (already heap-pinned, stable address). `cocoa_dom::spawner` now has
   zero unsafe blocks.

2. **`from_node_unchecked` release-mode hole** (review #2) — fixed.
   Promoted `debug_assert_eq!` to plain `assert_eq!` in
   `Element::from_node_unchecked`, `Text::from_node_unchecked`, and
   `Placeholder::from_node_unchecked`. One enum compare per
   construction; cheap insurance against silent type-confusion bugs.

3. **O(n²) `splice_subview_before`** (review #10) — fixed.
   Replaced the detach-everything-and-reattach algorithm with
   `addSubview:positioned:relativeTo:` using `NSWindowOrderingMode::Below`,
   which inserts the new subview immediately before the marker in the
   subviews array. Now O(1) per insert. (Earlier code would have made
   `<For>` over a long list unusable.)

4. **`Mountable::unmount` doesn't drop Taffy nodes or handlers**
   (review #5, #6, #8) — fixed. Added:
     - `cocoa_dom::layout::drop_node(node)` — removes the Taffy node
       and the NSView→NodeId registry entry.
     - `cocoa_dom::node::Node::teardown()` — calls
       `event::drop_handlers_for(view)`, then `layout::drop_node(node)`,
       then `view.removeFromSuperview()`.
   Updated every Mountable::unmount impl (Element/Text/Placeholder/
   Node in tachys::renderer::cocoa, plus ElementState/LabelState in
   tachys::cocoa::element) to call `teardown()` instead of just
   `removeFromSuperview()`. Recursion happens via the existing
   `children.unmount()` chain in ElementState.

5. **No window resize handling** (review #13) — fixed.
   Added a `WindowDelegate` ObjC subclass via `define_class!` that
   implements `NSWindowDelegate::windowDidResize:`. The delegate
   stores the root `Node` as an ivar and re-runs
   `layout::compute_layout` against the contentView's new size when
   resize fires. Installed by `app::run_app` after `setContentView`.

**Deferred** (would need bigger redesigns; see specific tasks):

  - **`mount_to_window` Owner leak** (review #7): tied to the lack of
    a real UnmountHandle story for native. Becomes natural once
    multi-window (task #15) lands and Window State owns its scope.
  - **Single-window/single-Taffy-tree assumption** (review #12):
    Tracked as task #15 ("Multi-window: App::run + Window as a Render
    type, Option C"). When that lands, the thread-local TREE goes
    away and the WindowDelegate from this round becomes per-Window
    instead of per-app.
  - **`Render::rebuild` no-op** (review #15): reactive attrs work via
    Effects; rebuild semantics for *static* attrs are a separate
    design exercise — defer until we have a concrete use case.
  - **Spawner per-wake batching** (review #14): perf, not correctness;
    current demos fine.
  - **`Node::from_view` loose bound** (review #3) and SendWrapper
    untested (review #4): both still concerns; will tighten when we
    have a tests harness.

**Validation.** counter_v2 still runs and clicks-through-effects-to-
labels still works (verified). Resize delegate wiring is textbook
AppKit; the run-without-crash confirms the delegate didn't blow up,
but I haven't directly verified a drag-resize visually reflows the
layout — worth poking at next time the window is open.

**Follow-up: layout bugs found while verifying resize.** Resize *was*
firing the delegate and recomputing layout, but the visible result
was still wrong. Two underlying bugs surfaced during diagnosis:

  - **Taffy didn't fill the root.** `compute_layout` was called with
    `available_size = window content size`, but the root's Taffy
    style had `size: auto`. Taffy sized the root to its *content*
    intrinsic size (e.g. 296×110 for the counter) and we then set
    that frame on the contentView. AppKit placed the smaller view
    at `(0, 0)` of the window's content area — which in the window's
    *unflipped* coords means the bottom-left corner. Visible bug:
    the entire UI pinned to the bottom-left, never reflowing.
    **Fix:** `compute_layout` now overrides `style.size` on the root
    with `Dimension::length(available_size)` before computing.

  - **`Render for ()` builds a Placeholder that took a flex slot.**
    Tachys' tuple children chain produces nested tuples like
    `(((), label), row)`. The leading `()` builds to a Placeholder
    NSView. Our Placeholder was registered with Taffy as a normal
    flex item with `size: 0×0`, which still consumed a flex slot
    and added a `gap`-worth of offset to the next sibling. Buttons
    visibly started at `x=8` instead of `x=0`. **Fix:** Placeholder's
    Taffy default style is now `position: Absolute` so it's removed
    from the parent's flex flow.

  - Confirmed working: window resize → relayout, buttons left-flush
    against padding edge, `flex_grow(1.0)` on buttons makes them
    grow with window width.

**New API added in the same pass:** `View::flex_grow(f32)`,
`Button::flex_grow(f32)`, `cocoa_dom::layout::set_flex_grow`. Plus
`cocoa_dom::layout::Position` re-exported.

**Still to do** (not in this pass):
  - `flex-shrink` interaction at small heights — at very small window
    heights the button row can overlap the label. Items with explicit
    `length` heights don't shrink the way explicit-height-CSS does.
    Workaround: set `min-height` or use intrinsic sizing.
  - `apply_layout` warning was previously firing constantly because it
    compared `subviews.len()` to Taffy children, and NSButton has
    internal subviews (cells, focus rings) the renderer doesn't own.
    **Fixed in same pass:** rewrote `apply_layout` to iterate subviews
    and only recurse into ones registered in `NODE_IDS`. AppKit
    internals are skipped without spurious warnings.

---

## 2026-05-03 — Honest review of the port so far

A pass over everything written across Stages 0–5, looking for bugs,
soundness issues, leaks, fragile abstractions, and gaps. Items are
roughly ordered by severity (most-likely-to-bite-us first).

### Soundness / correctness bugs

1. **`Pin::new_unchecked` in the spawner is probably unsound.** In
   `cocoa_dom/src/spawner.rs` `Task::poll_on_main`, we pin a
   `&mut TaskFuture` that's borrowed *out of* a `RefCell<Option<...>>`.
   When the future returns `Ready` we do `*slot = None` (line 126),
   which drops the future *in place*. Most non-trivial futures rely on
   being pinned to their address — async fn state machines self-borrow.
   Fix: store as `Pin<Box<dyn Future>>` so the future lives on the
   heap and the box's address is stable. Today's counter test happens
   not to hit this because the futures spawned by `Effect::new` are
   tiny `async move {}` blocks that don't self-borrow, but it's a
   timebomb.

2. **`Element::from_node_unchecked` only `debug_assert!`s the kind.**
   In release builds, an `Element` wrapper can wrap a `Text`-kind or
   `Placeholder`-kind Node, after which `set_attribute("title")` will
   silently no-op. Either keep the assert in release too (cheap — one
   enum compare) or hide the constructor entirely.

3. **`Node::from_view`'s bound is too loose.** `V: AsRef<NSView> +
   Message` is true of essentially every objc2 type, so you can pass
   in something that isn't actually a subclass of NSView. The cast
   would then be UB. Tighter bound: a custom marker trait, or an
   inherent `from_subclass<V: ClassType>(...)` that calls
   `T::class().instances_respond_to(...)` to verify.

4. **`SendWrapper<Retained<NSView>>` claims `Send + Sync`** so tachys'
   trait bounds are happy, but the runtime check only fires the *first
   time* a wrapper is touched on a wrong thread — by which point
   anything could have happened. Rely on it being mostly-correct, but
   we have zero tests verifying the panic actually fires at the right
   boundary.

### Memory leaks (all known, all currently unbounded)

5. **Taffy nodes leak forever.** `cocoa_dom::layout::ensure_taffy_node`
   adds entries to `NODE_IDS` and `TREE`, nothing removes them.
   Long-running apps with dynamic UIs will accumulate Taffy nodes
   indefinitely. Tied to issue #7 below — fixing teardown fixes this.

6. **Event handler retain-store leaks forever.**
   `cocoa_dom::event::HANDLER_STORE` grows on every `on_click`;
   `drop_handlers_for` exists but is never called.

7. **`mount_to_window` leaks the entire view State and the Owner via
   `std::mem::forget`.** This is fine for a whole-app-lifetime mount,
   but it means the design fundamentally cannot unmount anything. No
   way to swap views, no way to gracefully tear down. Currently a
   limitation; will block proper window-close semantics and
   second-window support.

8. **`Mountable::unmount` for our types removes from superview but
   does NOT clean up the Taffy node OR the handler-store entry.**
   So even if some upstream code did call unmount, leaks 5 and 6
   remain. The fix is to make unmount funnel through a shared
   teardown function that hits all three: NSView, Taffy, handler
   store.

### Architectural fragility

9. **NSView subviews and Taffy children must stay manually in sync.**
   `insert_node`, `remove_child`, `clear_children` mirror to Taffy.
   But anything that touches `addSubview:` directly (third-party code,
   future bug, even some AppKit machinery like NSScrollView's
   document view) bypasses our mirror. The runtime mismatch warning
   in `apply_layout` catches some divergence at layout time but says
   nothing about silent semantic drift in between.

10. **`splice_subview_before` is O(n²).** Inserting K children into a
    list of N existing siblings via the markered path costs O(K·N) —
    we detach all subviews from the marker forward, then re-attach. A
    `<For>` over 1000 items would be unusable.

11. **`mount_before` and `clear_children` fake the parent's
    `NodeKind`.** Both wrap an arbitrary NSView as `NodeKind::Element`
    "because that's what tachys expects." Works today because the
    only callers happen to have Element-kind parents, but it's a
    silent type lie that will bite when we generalise.

12. **Single thread-local Taffy tree.** Limits us to one window. The
    abstraction we'd need to thread through (a `LayoutContext` or
    similar) doesn't fit into tachys' `Mountable` interface — every
    mount currently has access to "the" Taffy tree implicitly.
    Multi-window is genuinely tricky given this.

13. **No window resize handling.** Documented in the Stage-4 entry
    but not tracked. Demos have a resize handle that does nothing —
    the user sees a clearly broken UI on first drag. Real teardown
    fix: install a window delegate that observes `windowDidResize:`
    and re-runs `compute_layout`.

14. **Spawner wakes are dispatched per-wake, not per-microtask.** If
    a signal triggers 10 dependent effects, that's 10 separate
    `dispatch_async` calls instead of one batch. The coalescing in
    `Task` works *per-task* but doesn't batch *across tasks*. For
    today's tiny demos this is fine; for a chatty UI it's wasted
    main-thread cycles.

### API / usability gaps

15. **`Render::rebuild` is a no-op for every Cocoa element.** Static
    attribute changes across rebuilds are dropped on the floor. A
    parent that reconstructs its child's props gets stale UI. Reactive
    attrs (via `MaybeReactive::Reactive`) work *because* the Effect
    re-fires, not because rebuild does anything.

16. **Button has no `.child()`.** The `view!{}` macro and Leptos
    convention is `<button>"Click"</button>`, not `.title("Click")`.
    This is part of why the macro doesn't work, but it's also a real
    ergonomic gap — you'd expect `button().child("Click")`.

17. **`CastFrom<Element>` only has the identity impl.** Anything that
    needs `T: CastFrom<Element>` for `T != Element` won't compile.
    Tachys' `event_target<T>(ev)` is the obvious caller — when we
    wire real `on:click` events with NSEvent payloads, we'll need
    real downcasts (probably via `NodeKind` + the cocoa class
    hierarchy).

18. **`IntoMaybeReactive<T>` only impls `T = String|bool|i32|f64`.**
    Adding any new attribute type means hand-writing two impls
    (static + closure). The conflict with the generic `Fn` impl can
    be sidestepped via a wrapper newtype like the web side does.

19. **Hardcoded leaf intrinsic sizes.** Buttons are always 80×32,
    labels are 22 high. A button with a 200-char title overflows.
    Real fix: Taffy measure functions that call into NSCell /
    NSAttributedString for content size.

20. **`set_padding` only takes one number.** No per-side API.
    Trivial to add but not done.

21. **Element types are not generic over their Owner.** Every
    `mount_to_window` leaks an Owner; we have no way to nest scopes
    that drop properly. Tied to #7.

22. **No `.style(...)` API on the cocoa builders.** All layout has
    to go through dedicated methods (`.flex_direction()`,
    `.padding()`). String-style parsing (e.g.,
    `style="padding:16px;gap:8px"`) doesn't exist.

### Hygiene / cleanup

23. **`tachys::cocoa` is `#![allow(missing_docs)]`** while every
    other tachys module is `#![deny(missing_docs)]`. Quick blanket
    fix (it's allowed because the real fix is writing docs).

24. **`leptos/src/mount_macos.rs` has a `PhantomData<Executor>`** as
    a workaround to silence an unused-import warning. The right fix
    is removing the import.

25. **Several stale unused-import / dead-code warnings in tachys**
    that I left in because they're cfg-gated: `borrow::Cow`,
    `ToTemplate`, `FEATURE_CONFLICT_DIAGNOSTIC`,
    `set_currently_hydrating`, `failed_to_cast_element`,
    `RemoveEventHandler::{new, into_inner}`. All harmless; all
    visible noise on every build.

26. **`cocoa_examples/` directory** is full of aspirational code
    that doesn't compile, sitting next to `examples/counter_macos/`
    which does. Easy to confuse. The README in cocoa_examples
    explains the difference, but a top-level pointer would help.

27. **Stage-numbering in `cocoa_examples/README.md` is now slightly
    out of date** (it predates the Slice A/B/C split inside Stage 5).

### Testing gaps (also captured in tests.md)

28. **Zero automated tests across the entire port.** Every
    "validation" so far is "user clicked and reported the trace
    output." Easy to silently regress with the next change. The
    `tests.md` checklist is comprehensive but nothing is wired up.

29. **`mount_before` / `try_mount_before` are never exercised.**
    Tachys uses them in dynamic-children diffing; no current demo
    triggers that path. Could be subtly broken without us knowing.

30. **The Effect cleanup-on-unmount story is untested.** When a
    `View`'s State drops, do its `_effects: Vec<RenderEffect<()>>`
    actually get dropped? Theoretically yes — Vec drop runs each
    Drop. But nothing verifies the chain works through nested
    elements.

31. **Threading panics are untested.** `SendWrapper` is supposed to
    panic if a Node is touched off-main; no test fires that path.

### Documentation drift

32. **The Stage-4 log entry says "no resize handling — Stage 4
    follow-up"** but no follow-up task exists. Should at least be
    in the task list.

33. **`mount_to_window` always opens *a single window with one root
    view*.** No multi-window, no menu-bar-only apps, no panels. Not
    documented as a limitation anywhere user-visible.

34. **The "what's the API" question is answered in three places**:
    `cocoa_examples/README.md`, the `tests.md` API conventions
    section, and inline doc comments. They're consistent today but
    will drift.

### Things that held up better than expected

For balance — these I was nervous about and they're actually fine:

- The renderer/Mountable wiring through tachys: the type-aliasing
  approach (`crate::renderer::types::*`) really did make most of
  view/ work without modification.
- The spawner coalescing under real button-mash testing: no missed
  wakes, no double-fires observed in the manual click traces.
- The Cocoa element builder API: feels like Leptos. The
  `IntoMaybeReactive` trick for "static value or closure" reads
  cleanly at the call site.
- `define_class!` for `FlippedView`, `ActionTarget`, and
  `AppDelegate`: less ceremony than expected; the macro hides most
  of the ObjC ivar dance.
- The split between cocoa_dom (low-level NSView wrappers) and
  tachys::cocoa (Render-shaped builders): turned out to be the
  right boundary; no awkward back-references.

---

## 2026-05-03 — Stage 5 part 2 (Slice A): leptos crate compiles + mount_to_window

**Decision.** The `leptos` crate now compiles on macOS. Web-only
modules (`form`, `portal`, `animated_show`, `mount`, `hydration`) are
gated behind `cfg(not(target_os = "macos"))`. A new
`leptos::mount_macos` module provides `mount_to_window(title, size, f)`
as the macOS analogue of `mount_to_body` / `mount_to`.

The prelude on macOS re-exports the core stuff (signals, effects,
control_flow without animated_show, into_view, suspense, text_prop,
component) plus `mount_macos::*` and `tachys::cocoa::*`. Web-only
prelude items (`bind:` infra, `node_ref`, `leptos_dom::helpers::*`,
`form::*`, etc.) are dropped.

A new `leptos::cocoa` re-exports `tachys::cocoa` for ergonomics:
`use leptos::cocoa::element::{button, label, ...}`.

**Status.** Validated with `examples/counter_macos/`, a counter that
uses `leptos::prelude::*` and `mount_to_window` end-to-end. Compiles,
runs, and quits cleanly via Cmd-Q (exit code 0).

**What's NOT in part 2 — split into Stage 5 part 3 (task #9).** The
`view!{}` macro is *not* wired up on macOS. It hardcodes 26
`::leptos::tachys::html::*` paths in `leptos_macro/src/view/mod.rs`,
and proc macros can't read consumer `target_os`. To make it work
natively we need either:
  1. Fork the macro into a Cocoa-flavoured `view_cocoa!{}`, or
  2. Build a `tachys::html::element`-shaped facade over our Cocoa
     elements (so unmodified macro emission resolves correctly via
     path aliasing), or
  3. Patch the macro to emit cfg-conditional paths (still needs the
     facade for the emitted code to compile).

Each is multi-day work. For now, users on macOS write builder syntax:

```rust
stack_view().padding(16.0).child((
    label().text(move || count.get().to_string()),
    button().title("+1").on_click(move || count.update(|n| *n += 1)),
))
```

`#[component]` and `IntoView` are mostly available already through
the `leptos` re-exports — what's missing is plumbing them into
`view!{}`.

---

## 2026-05-03 — App lifecycle: menu + close-to-quit

**Fix.** `cocoa_dom::app::run_app` now installs:
  1. A minimal main menu with `App > Quit (⌘Q)` so Cmd-Q is bound and
     the menu bar shows the process name (the standard "App menu" idiom
     in AppKit — the first menu's title is replaced by the process
     name automatically).
  2. A tiny `AppDelegate` (NSObject subclass via `define_class!`) that
     returns true from `applicationShouldTerminateAfterLastWindowClosed:`
     so closing the only window terminates the app instead of leaving
     a windowless process running.

**Why it was missing.** AppKit doesn't auto-bind Cmd-Q, and
`applicationShouldTerminateAfterLastWindowClosed:` defaults to NO.
Both have to be wired up explicitly.

**Test.** counter_v2 quits cleanly via either Cmd-Q or the red
traffic-light close button, returning exit code 0.

---

## 2026-05-03 — Stage 5 part 1: Cocoa element builders + Render integration

**Decision.** New module `tachys::cocoa` containing:
  - `element::{view, button, label, text_field, stack_view}` builder fns
  - Builder structs (`View<Ch>`, `Button`, `Label`, `TextField`) with
    chained methods like `.title(...)`, `.flex_direction(...)`,
    `.padding(N)`, `.gap(N)`, `.on_click(...)`, `.child(...)`
  - `Render` impls so each builder integrates with tachys' view tree
  - `attr::{IntoMaybeReactive, MaybeReactive, install}`: a small trait
    that lets attribute setters accept either a static value (`&str`,
    `String`) or a reactive closure (`Fn() -> String`); closures get
    wrapped in a `RenderEffect` automatically. The effect is owned by
    the element's `State` so it lives exactly as long as the mount.

**Why this scope.** Full Stage 5 is "view! macro + leptos crate
compiling on macOS + #[component] + IntoView" — multi-day work. Part 1
gets the *user-facing API* shape right with a builder syntax (the
equivalent of Leptos's `counter_without_macros` example), validates the
reactive integration end-to-end, and de-risks the design before we
touch the macro.

**Children handling.** Tuples like `(a, b, c)` are built by tachys'
existing `Render for (A, B, ...)` and `Mountable for (A, B, ...)`
impls. `View::child(c)` chains via nested tuples: `View<((), c1)>`,
`View<(((), c1), c2)>`, etc. Render then mounts each leaf under the
parent in source order — the tuple Mountable handles the recursion.
No custom trait needed.

**Status.** Validated end-to-end with
`cocoa_dom/examples/counter_v2.rs` — same counter as Stage 3/4 but
written in builder syntax with no manual `Effect::new`. Reactive
closures inside `.text(...)` and `.title(...)` re-fire automatically
on signal change. User confirmed clicks work.

**Remaining for full Stage 5** (tracked as task #8, "Stage 5 part 2"):
  - Make the `leptos` crate compile on macOS (it currently pulls in
    cfg'd-out tachys html submodules transitively).
  - Make the `view!{}` macro expand to our builder calls. Macro is
    in `leptos_macro`; it parses HTML-ish syntax and emits builder
    calls. Likely needs cocoa-aware pieces or a fork.
  - `mount_to_window` as a `leptos::mount` entry point.
  - `#[component]` + `IntoView` integration so users can write
    components naturally.

---

## 2026-05-03 — Stage 4: Taffy layout integrated

**Decision.** Layout is computed by `taffy` (the layout engine used by
Dioxus and Bevy). Each cocoa_dom Node has a paired `taffy::NodeId` in
a thread-local `TaffyTree`. Tree mutations (`insert_node`,
`remove_child`, `clear_children`) on `Element` mirror into the Taffy
tree. The layout module exposes inline-style setters
(`set_flex_direction`, `set_padding`, `set_gap`, `set_width`,
`set_height`, `set_justify_content`, `set_margin`) that update the
Taffy `Style` for a node.

The mapping NSView pointer → `taffy::NodeId` lives in a thread-local
hashmap; each Node clone shares the same id. Drops leak entries (no
cleanup yet — see limitations).

**Coordinate system.** AppKit defaults to bottom-left origin; Taffy
emits top-left coordinates. Rather than manually flip y-values during
frame application, we made every layout container a `FlippedView`
(ObjC subclass via `define_class!` that returns YES from `isFlipped`).
Tags `<view>`, `<stack_view>`, and the unknown-tag fallback all use
FlippedView. Leaf controls (`<button>`, `<label>`, `<text_field>`)
keep their native classes — flippedness only affects how a view
interprets its *children*, not how its parent positions it.

**No NSStackView.** We deliberately don't use AppKit's NSStackView
even for `<stack_view>`. Two layout systems competing (NSStackView's
constraints + Taffy's flexbox) is a recipe for confusion. `<stack_view>`
is a FlippedView with `flex_direction = Column` by default — same
behaviour, single source of truth.

**Status.** Validated end-to-end with `cocoa_dom/examples/counter.rs`,
which uses `set_flex_direction`, `set_padding`, `set_gap` instead of
hardcoded frames. Buttons land in distinct, clickable positions in a
horizontal row below the label, with even spacing.

**Known limitations to revisit.**

  - **No resize handling.** Window resize does not reflow. Layout is
    computed exactly once, just before `makeKeyAndOrderFront`. Fixing
    this needs either an NSView subclass that overrides `setFrameSize:`
    (and calls back into our layout module) or a window delegate
    listening for `windowDidResize`. Next layout follow-up.
  - **One Taffy tree per thread.** Single-window apps only. Multiple
    windows would need a per-mount `TaffyTree`, threaded through a
    `LayoutContext` type. Probably tied to multi-window mounting work.
  - **Taffy nodes leak on Node drop.** Same retain-store-leak story as
    Stage 3's event handlers — the cleanup hook will go in
    `Mountable::unmount` once we wire dynamic UIs (Stage 6).
  - **Hardcoded leaf sizes.** Button: 80×32. Label/text_field height:
    22. No content-based sizing. Real measure functions (Taffy's
    `compute_layout_with_measure`) need to call into `NSCell` /
    `NSAttributedString` to get intrinsic sizes; deferred.
  - **Small style surface.** Only the styles the counter demo uses are
    exposed via setters. Adding the rest (align_items, flex_grow,
    position, individual padding sides, etc.) is mechanical — do as
    they're needed.
  - **No string-style parser.** No `style="padding:16px;gap:8px"`
    parsing yet — every property has its own setter. Once we have
    string-attribute infra in Stage 5, we'll layer a parser on top.

---

## 2026-05-03 — Stage 3: events + spawner work end-to-end

**Decision.** The AppKit main-thread executor lives in
`cocoa_dom::spawner` and is registered via
`any_spawner::Executor::init_custom_executor`. Each spawned future gets
its own `Task` whose Waker re-dispatches a poll onto
`DispatchQueue::main()` (libdispatch via the `dispatch2` crate).
Events use a custom `ActionTarget` ObjC subclass (via
`define_class!`) that holds a `RefCell<Box<dyn FnMut()>>` ivar and
exposes one selector, `actionFired:`. Each `Element::on_click` builds
an `ActionTarget`, wires NSButton's target/action to it, and stashes
the `Retained<ActionTarget>` in a thread-local registry keyed by the
NSView pointer (so the target outlives the registration).

**Why dispatch over NSRunLoop performSelector?** Cleaner type story,
fewer ivars to invent, Apple's recommended modern API. The
`dispatch2` crate wraps it ergonomically (`exec_async(closure)`).

**Status.** Validated end-to-end with `cocoa_dom/examples/counter.rs`:
button clicks update an `RwSignal`, an `Effect` recomputes a label's
text, the AppKit run loop drains the dispatch queue between events.
User clicked through `+1` ×4, `-1` ×2, `Reset`, `+1` ×3 — every effect
fired, every label update landed on the right value.

**Known limitations to revisit.**

  - Handler retain registry leaks. `drop_handlers_for` exists but is
    never called. Fine for short-lived demos; needs hooking up to
    `Mountable::unmount` once we wire dynamic UIs (Stage 6).
  - One handler per control. `setTarget:setAction:` only stores one
    pair; calling `on_click` twice silently replaces the previous
    handler (the orphan stays in the retain-store as a leak). To
    support multiple listeners we'd need a fan-out target holding
    `Vec<Box<dyn FnMut>>`.
  - Only `on_click` is implemented. `on_input` (NSTextField changes),
    keyboard, mouse-move, etc. — all Stage 4+ work.
  - The first poll's "queued = true" optimisation in `Task::new` is
    safe for the dispatched-once initial poll, but its interaction
    with re-entrant wake() during that initial poll is subtle. If we
    see missed wakes, audit this first.

---

## 2026-05-03 — `bind:` two-way binding will be rebuilt for Cocoa, not deleted

**Decision.** Stage 2 cfg'd out `tachys/src/reactive_graph/bind.rs` on
macOS, but only because its implementation is heavily coupled to HTML
element marker types (`<input>`, `<select>`, etc.) that don't exist on
the native target. The *concept* is universal and probably more useful
on AppKit than on the web — native apps lean heavily on form controls,
and AppKit's target/action + delegate model is verbose enough that the
convenience matters.

A Cocoa-flavoured replacement (`tachys/src/reactive_graph/cocoa_bind.rs`,
or similar) is planned for Stage 5+. Trait shape will mirror the web
version: a `BindValue<V>` trait parameterised by the bound value type,
with one impl per NSControl-based element we expose
(`text_field` → `String`, `checkbox` → `bool`, `slider` → `f64`,
`pop_up_button` → `usize`/`T`, `color_well` → `NSColor`, etc.). Each
impl spins an `Effect` to push the signal out and registers a
target/action (or KVO / NSControlTextDidChange observer) to push input
back in.

**Cost of waiting.** Users have to wire signal-↔-control plumbing
manually with `on:` + `Effect` until this lands. Workable but
boilerplate-heavy.

**How to apply when implementing.** One impl per control kind. Make
sure the Effect is dropped when the element is unmounted (otherwise the
NSView outlives the signal). For controls that fire continuous events
(slider drag), debounce or coalesce so we don't thrash the signal
graph.

---

## 2026-05-03 — Stage 2: cfg-out the html element / mathml / svg / web modules

**Decision.** On macOS, the bulk of `tachys/src/html/` is gated out:
`element/`, `event.rs`, `style.rs`, `property.rs`, `node_ref.rs`,
`directive.rs`, `class.rs`, `islands.rs`, plus the Doctype and
InertElement types in `html/mod.rs`. Same for `mathml/`, `svg/`, the
DOM helpers in `dom.rs`, the web feature of `oco.rs`, and the web-bound
submodules of `tachys/src/reactive_graph/` (bind, class, inner_html,
node_ref, property, style — but `owned`, `suspense`, and the
`ReactiveFunction → Render/RenderHtml` core impls in `mod.rs` stay).

What survives on macOS:
  - `view/` — fully working (it was already web-agnostic).
  - `ssr/` — string-builder for HTML output; harmless to keep.
  - `hydration::Cursor` — the type compiles; tree-walking methods are
    stubbed in `cocoa_dom::Renderer` (panic if called). The
    `failed_to_cast_*` helpers have native panic-stubs.
  - `html::attribute::{any_attribute, key, value, mod, ...}` — the
    attribute trait machinery is mostly platform-agnostic; only
    `aria.rs`, `custom.rs`, `global.rs` are gated out (they tie back to
    HTML element types).
  - `renderer/cocoa.rs` — new module providing the `Dom` unit struct
    plus `Mountable` / `CastFrom` impls for the cocoa_dom types.

**Why.** The user wants Cocoa-flavoured elements (Stage 5 will define
those). Keeping the web HTML element types around on native would
require either implementing them as no-ops or fighting heavy web-sys
coupling. Cleaner to delete-by-cfg now and rebuild a Cocoa parallel.

**Cost.** A user on macOS gets a substantially diminished `tachys` API:
no Doctype/InertElement, no `class:`/`style:`/`prop:`/`on:` infra, no
NodeRef, no two-way binding (`bind:`), no SVG/MathML, no event types.
The `view!` macro will need similar care in Stage 5 (it will produce
Cocoa-element calls instead of HTML-element calls).

**Cleanup path.** None planned — these submodules are HTML-specific by
design. The Cocoa replacements (Stage 5) will live in a new module,
not by reviving these.

---

## 2026-05-03 — `Dom` is a unit struct, not a type alias

**Decision.** `tachys::renderer::cocoa::Dom` is a unit struct that
forwards to `cocoa_dom::Renderer`, rather than a type alias for it.

**Why.** `mount_before` / `try_mount_before` are tachys-specific
methods that take `M: Mountable` (a tachys trait). Orphan rules forbid
adding inherent methods to a foreign type, so they can't go on
`cocoa_dom::Renderer` directly. Wrapping in a unit struct gives us a
home for them.

**Cost.** ~100 lines of mechanical forwarding methods on `Dom`.

**How to apply.** When adding new renderer methods that are pure
NSView manipulation, put them on `cocoa_dom::Renderer` and add a
forwarder on `Dom`. When the new method needs tachys traits in scope,
put it directly on `Dom`.

---

## 2026-05-03 — Hydration: stub `hydrate` to delegate to `build`

**Decision.** On the native macOS target, every `RenderHtml::hydrate` impl
delegates to `Render::build` (and ignores the `Cursor` / `PositionState`
arguments). The cursor's tree-walking methods (`first_child`,
`next_sibling`, `get_parent`) on the cocoa_dom renderer are stubbed with
`unimplemented!()` — they should never be called on the native path.

**Why.** Hydration only earns its keep on the web: SSR sends a fully
rendered HTML page that the user can see instantly, and hydration walks
that pre-existing DOM to attach the live reactive system in-place. Native
macOS has no SSR phase — every NSView is created in-process by our code,
and event handlers are wired up as the tree is built. There is no
"existing tree" for hydration to walk.

The `<template>`-cloning optimization (the other use of the hydrate code
path, for CSR) is also moot: deep-cloning an NSView subtree isn't cheap
on AppKit (no native deep-copy API), and NSView construction is rarely
the bottleneck anyway.

**Why we keep the trait at all.** `RenderHtml` is a supertrait of
`IntoView` in the leptos crate. Removing it from the trait graph is
invasive (touches leptos, leptos_macro, every view impl site). Stubbing
the methods lets us compile and run on native without that yak-shave.

**Cost.** Carries dead code (the hydrate impls are never executed on
native). Slight binary-size and compile-time overhead. The `Cursor` type
and `PositionState` machinery exists in the binary but is unreachable.

**Cleanup path** (when we're ready to do it).

Feature-flag `RenderHtml` out of `IntoView`'s supertrait bound on the
native target — so types only need to implement `Render` to be usable in
`view!{...}`. Concretely:

  1. Add a `cocoa` (or similar) feature on `tachys` and `leptos`.
  2. Conditionally weaken `pub trait IntoView: ... + RenderHtml` to
     `pub trait IntoView: ... + Render` when that feature is active.
  3. Audit `tachys/src/view/`, `tachys/src/html/`, the macro output, and
     `leptos/src/*` for places that bound on `RenderHtml` and weaken to
     `Render` where the SSR path isn't reachable.
  4. Remove the stubbed `hydrate` impls and the unused `Cursor` machinery
     from the native build via `cfg`.
  5. Reconsider whether `RenderHtml::resolve` (async data resolution)
     should stay — it's used by Suspense/Transition and may still be
     useful for native async data loading even without SSR.

**Tracking.** Stage-2 work proceeds with stubbed hydrate. Real cleanup is
deferred until after the basic counter example is running end-to-end on
native (i.e. after Stage 6).


## bind:value redundant-set dedup at the platform boundary

**Symptom.** Typing the first character into a `<text_field bind:value=signal />`
caused the focus ring to flash blue → grey → blue.

**Root cause.** Standard reactive bind cycle:
  1. User types "a"; AppKit updates the field's stringValue.
  2. `controlTextDidChange:` fires → handler calls `signal.set("a")`.
  3. `Set` impl in `reactive_graph/src/traits.rs` does NOT diff before
     notifying subscribers — every `set` notifies.
  4. The `RenderEffect` installed by `bind:` re-runs, reads the signal,
     and calls `set_attribute("value", "a")`.
  5. `node.rs` calls `NSControl::setStringValue("a")`. AppKit treats this
     as a programmatic change, marks the cell needs-display, and the
     focus ring redraws → visible flash.

**Why the web doesn't have this problem.** Web bind in
`tachys/src/reactive_graph/bind.rs` does no dedup either. Same cycle
runs (user input → set → Effect → `el.value = "a"`). The browser
gracefully no-ops `el.value = same_value` while focused — no repaint,
no caret reset, no focus-ring flash. The browser provides the dedup
implicitly.

**Fix.** Diff the new value against the current platform value inside
`set_attribute` for `title` / `value` / `hidden`, and skip the AppKit
mutation (and the subsequent `schedule_relayout`) when they match.
This puts the dedup at the platform boundary — exactly where the web
gets it for free from the browser. See `cocoa_dom/src/node.rs`.

**Why not in the bind layer.** Putting dedup in `bind.rs` would only
protect `bind:value`. Any other reactive `value=...` (e.g. a derived
`Signal` passed as a one-way attribute) would still hit the unguarded
`setStringValue:`. The platform-boundary placement is uniform.

**Trade-offs.**
  * Small cost: every `set_attribute` call now reads the current value
    from AppKit before deciding. For text fields this is a CFString
    comparison — cheap.
  * Loses the ability to "force re-set with same value" (e.g. to trigger
    a side effect bound to the setter). Nothing in our API exposes that
    today; revisit if it ever matters.


## More controls: slider, pop_up_button, secure_text_field, button.enabled

Added `<slider>`, `<pop_up_button>`, `<secure_text_field>`,
`enabled=...` on controls, and `placeholder=...` on text fields.
Examples: `examples/settings_macos`, `examples/login_form_macos`.

### `on_action` vs `on_click` (NSControl vs NSButton)

`Element::on_click` downcasts to `NSButton` because the original
caller (Button) is one. NSPopUpButton happens to subclass NSButton
so that worked too. **NSSlider does NOT** — it's a sibling of
NSButton, both extending NSControl directly. The first slider
implementation used `on_click` and the bind silently dropped on
the failed downcast (slider dragged AppKit's value but never
reached the signal).

Fix: added `Element::on_action` that downcasts to NSControl, and
a corresponding `event::on_control_action` helper.
`event::on_button_click` is now a thin wrapper over
`on_control_action`. Slider bind uses `on_action`; popup bind
still uses `on_click` (via NSButton subclass) — both work.

**Lesson**: when a control type isn't a button, default to
`on_action`. Reserve `on_click` for the literal NSButton API.

### Custom AttributeKey (`Selection`) for non-HTML bindings

`bind:selection=signal` on `<pop_up_button>` emits
`.bind(::leptos::attr::Selection, signal)` from the macro.
"Selection" isn't in the upstream HTML attribute list, so we
defined `Selection` in `tachys::cocoa::bind` and re-exported it
from `tachys::html::attribute` (under
`#[cfg(target_os = "macos")]`). This is the pattern for any
future cocoa-only bind keys (e.g. eventually `bind:color` for a
color well).

### `IntoAttributeValue` escape hatches for non-web values

`<pop_up_button items=vec!["A","B"]>` failed to compile because
the macro wraps non-literal attribute values through
`IntoAttributeValue::into_attribute_value`. The blanket impl is
`impl<T: AttributeValue> IntoAttributeValue for T`, and
`Vec<&str>` isn't an `AttributeValue` (and shouldn't be — it's
not a web-attribute type).

Fix: direct `IntoAttributeValue` impls for `Vec<&'static str>`
and `Vec<String>` in `tachys/src/html/attribute/value.rs`,
gated by `cfg(target_os = "macos")`. `type Output = Self`. The
items setter on PopUpButton accepts any `IntoIterator<Item = Into<String>>`,
which covers both forms.

**General principle**: when our cocoa builders accept attribute
values that don't fit the web's `AttributeValue` shape, add a
narrow `IntoAttributeValue` impl with `Output = Self`. Don't
try to teach `AttributeValue` about new value types.

### Edit menu — first-responder dispatch for text-field shortcuts

Cmd+A / Cmd+X / Cmd+C / Cmd+V / Cmd+Z don't work in NSTextField
"by default" — they need menu items to bind the keyboard
shortcuts. AppKit dispatches the menu's action through the
responder chain (target: nil → focused control), and NSTextField
implements all the standard editing selectors natively. So we
just install an Edit menu with selectors `selectAll:`, `cut:`,
`copy:`, `paste:`, `undo:`, `redo:`, `delete:`. No callbacks
needed on our side.

This is the same first-responder pattern as Cmd+Q via
`terminate:` on the App > Quit item.


## Typed `set_bool_attribute` (no more "true"/"false" round-trips)

Booleans (`enabled`, `hidden`, `checked`) used to flow:
`bool → "true"/"false" → set_attribute(name, &str) → matches!(value, "true"|"1"|"") → bool`.

That detour is appropriate on the web (HTML attributes are
stringly-typed) but pure cargo cult here — this is a Rust app
talking to AppKit, neither side wants strings.

Replaced with `Element::set_bool_attribute(name: &str, value: bool)`
in cocoa_dom. Routes by `name`:
  * `"enabled"` → `NSControl::setEnabled:`
  * `"hidden"`  → `NSView::setHidden:`
  * `"checked"` → `NSButton::setState:`

Each setter still diffs against the current AppKit state before
mutating (focus-flash protection logic stays).

Removed those three names from string `set_attribute`. The doc on
`set_attribute` now points at `set_bool_attribute` for the bool
cases. `remove_attribute` for those names delegates to
`set_bool_attribute(name, default)`.

**Pattern**: when a future attribute has a non-string type
(integer, enum, NSColor), add a typed setter alongside (e.g.
`set_int_attribute`, `set_color_attribute`). Don't grow
`set_attribute` into a stringly-typed dispatch hub.


## Typed `Attribute` enum for compile-time-checked attribute dispatch

Builders previously called the renderer trait's stringly-typed
`set_attribute("title", ...)` / `set_bool_attribute("enabled", ...)`,
matching against the same string at the cocoa_dom layer. Typos
were silent runtime no-ops.

Added `cocoa_dom::Attribute` — an enum covering the small set of
attributes we recognize (`Title`, `Value`, `Placeholder`,
`Enabled`, `Hidden`, `Checked`). New typed entry points on
`Element`:
  * `set_string_attribute(Attribute, &str)`
  * `set_bool_attribute(Attribute, bool)`
  * `remove_attribute_typed(Attribute)`

Each setter `match`es only the variants relevant to its value
type; the others silently no-op. (Stricter would be two enums
`StringAttr` / `BoolAttr` — easy to refactor later if it becomes
a footgun.)

The string-keyed `set_attribute(&str, &str)` and
`remove_attribute(&str)` stay on `Element` because the upstream
`Rndr` trait expects them. They now route through
`Attribute::from_name(s)` and call the typed methods, so the
internal match still lives on the enum.

All cocoa-builder call sites (`tachys/src/cocoa/element.rs`,
`bind.rs`) now use the typed variants. The `Rndr` trait surface
in `cocoa_dom/src/renderer.rs` is unchanged — that one keeps
`&str` in/out for trait conformance.

**Pattern**: when an attribute name is hardcoded at the call site,
prefer the typed enum. Reserve the string entry points for paths
where the name is genuinely runtime-supplied (the `Rndr` trait
surface, `from_name` parsing, debug logging).


## Code review — events + attributes — concerns to revisit

Self-review of the events/attributes layer (`cocoa_dom/src/event.rs`,
`cocoa_dom/src/node.rs`, `tachys/src/html/event_macos.rs`,
`tachys/src/cocoa/element.rs`, `tachys/src/cocoa/bind.rs`,
`tachys/src/cocoa/attr.rs`). Things that work today but smell or
will bite us later — none urgent, all worth fixing before this layer
ossifies.

### Type-safety: silent no-ops on control-type mismatch

Multiple paths silently drop wiring when the underlying NSView isn't
the expected subclass:
  * `Element::on_click` downcasts to NSButton → no-op on non-buttons.
    We hit this exact bug ourselves with the slider (NSSlider is a
    sibling of NSButton, not a subclass; `on_click` did nothing).
    Fix was `Element::on_action` (NSControl-based), but `on_click`
    is still the path `PendingHandler::Click` uses.
  * `PendingHandler::apply_to` routes Click → on_click, Input/Change
    → text-field hooks. `on:click` on a `<text_field>` or `on:input`
    on a `<button>` silently drops, no warning.
  * `Element::on_text_change` / `on_text_end_editing` — same shape;
    silent no-op if the view isn't NSTextField.

Pattern matches the web's loose `addEventListener` shape, but here
we know the control type at the call site (the builder type tells
us). Could enforce at compile time per-builder (Button only accepts
ClickEvent; TextField only accepts Input/Change/etc). At minimum,
should `eprintln!` in debug builds when a downcast fails so the
user gets a hint rather than silence.

### Type-safety: `Attribute` enum allows runtime mismatch

`Attribute` is one enum with both string- and bool-valued variants.
`set_string_attribute(Attribute, &str)` and
`set_bool_attribute(Attribute, bool)` each match only the relevant
subset; passing a wrong-type variant silently no-ops.

E.g. `el.set_string_attribute(Attribute::Enabled, "foo")` compiles
and runs without warning. Stricter would be two enums (`StringAttr`
/ `BoolAttr`); easy to refactor if it bites.

### Resource lifecycle: handler retain leak

`HANDLER_STORE` and `TEXT_FIELD_STORE` (thread-local
HashMap<view_ptr, Vec<Retained<...>>>) keep delegate/target objects
alive for the lifetime of their NSView. Cleanup is via
`drop_handlers_for(view)` from `Node::teardown`, called from
`Mountable::unmount`. Two concerns:

  1. **Lifecycle isn't actually wired end-to-end.**
     `mount_to_window` leaks the Owner, so `Mountable::unmount`
     never runs in the only entry point we ship. Handlers
     accumulate forever in long-running apps.
  2. **Stale comment in `event.rs:108-110`** says "Currently never
     called". It IS called now (from `Node::teardown`); the comment
     is obsolete.

### Resource lifecycle: pointer-keyed handler stores

`view_key(view)` casts `&NSView` to `*const NSView as usize`.
Stale-key risk if an NSView is freed and a new one allocated at
the same address — the old store entry would attach to the new
view. In practice safe because we hold `Retained<NSView>` which
keeps refcount ≥ 1 until our entry is removed, but the contract
is implicit. If we ever introduce weak references or allow views
to be replaced under us, this breaks silently.

Alternative: use `objc2::rc::WeakId` keys, or attach the handler
state as an associated object on the NSView itself.

### Inconsistency: button click vs text-field input fan-out

Text fields support multiple handlers per event (the fan-out
delegate's `on_input`/`on_change` are `Vec<Box<dyn FnMut(String)>>`).
Buttons (and other NSControls) use NSControl's single target/action
slot — calling `on_click` twice replaces the previous wiring (the
old ActionTarget stays in the retain store but never fires).

So `bind:checked` + `on:click` on the same checkbox = the second
install wins, the first is silently dropped. Different semantics
from text fields, no warning.

### Boilerplate / duplication

  * **Bound{Value,Float,Index,Checked} structs** in `bind.rs` are
    structurally identical except for `T`. Could be one
    `Bound<T> { getter, setter }`. Same for `install_*_value_bind`
    /  `install_*_checked_bind` / etc.
  * **Builder boilerplate**: Button, Checkbox, Slider, PopUpButton,
    TextField each carry their own `Vec<PendingHandler>`,
    `enabled: Option<MaybeReactive<bool>>`, `.on()` / `.add_any_attr()`
    methods, and an `enabled` install block in `build()`. Plenty of
    copy-paste; a shared trait or macro could collapse it.
  * **`IntoMaybeReactive<T>` requires per-type impls** for both
    `Static` and `Reactive(closure)` paths. Combinatorial as we add
    new value types. Specialization or sealed-trait magic might
    collapse.

### Other smells / questions

  * **`PendingHandler` is a closed enum** (one variant per event
    kind). Each new event type (`on:keydown`, `on:focus`, etc.)
    requires editing four places: marker type, EventDescriptor
    impl, PendingHandler variant, apply_to arm. A
    `Box<dyn ApplyToElement>` trait object would localize the
    addition to the new event's own impl. Not urgent — events are a
    small bounded set.
  * **`MaybeReactive::Reactive` uses `FnMut`** where `Fn` would
    suffice (we only ever read). FnMut works but is over-broad.
  * **`add_any_attr` on builders only handles `OnAttribute`** —
    spread-attribute support is event-only. If a user spreads a
    class/style/prop attribute through `{..attr}`, it'll fail to
    typecheck; we don't gracefully degrade.
  * **`set_string_attribute` schedules relayout for Title/Value but
    not Placeholder**. Placeholder text affects intrinsic width;
    setting it mid-life could change visible size. In practice not
    hit because placeholder is set once at build, but inconsistent
    dirty-marking is a footgun for future reactive placeholder
    support.
  * **`on_button_click` is now a thin wrapper over
    `on_control_action`**. Could be inlined / removed.
  * **`Selection` AttributeKey lives in `cocoa::bind` and is
    re-exported from `tachys::html::attribute`** to satisfy the
    macro emit path (`::leptos::attr::Selection`). Awkward
    indirection that every future cocoa-only bind key will need.
    A small `#[cfg(target_os = "macos")] mod cocoa_keys` in the
    attribute module might centralize them.
  * **`Rndr` trait `set_attribute(&str, &str)` surface** is mostly
    vestigial on macOS — cocoa builders use the typed methods
    directly. Kept for trait conformance. May be removable once we
    audit which generic tachys code paths actually invoke it on
    cocoa::Dom.
  * **Re-entrance handling with `eprintln!`**: when a callback
    triggers another call into the same delegate, we skip with a
    stderr message (`event.rs:184-188`, `event.rs:210-214`). Better
    than panic but stderr noise feels wrong for a library; should
    probably be a tracing call or feature-gated.


## Compile-time event/attribute correctness + window cleanup

Three fixes from the events+attributes review.

### `SupportsEvent<E>` for compile-time event-on-builder check

Added `SupportsEvent<E>` marker trait in
`tachys/src/html/event_macos.rs`. Each builder's `.on()` method now
takes a `Self: SupportsEvent<E>` bound. Each builder explicitly
opts in to events it supports:

  * `Button` → `ClickEvent`
  * `Checkbox` → `ClickEvent`
  * `TextField` (incl. secure) → `InputEvent`, `ChangeEvent`
  * `Slider` / `PopUpButton` — no events yet (only `bind:`)

Mismatched pairings now produce a compile error rather than
silently no-oping. Verified: inserting `<button on:input=…>` errors
with "expected `ClickEvent`, found `InputEvent`" pointing at the
trait bound. Slight error-message awkwardness because the bound
forces type inference rather than producing a "no impl" error
directly, but acceptable.

The spread-attribute path (`{..attr}` carrying an `OnAttribute`)
remains type-erased — type checking there would require
reverting to the inline `.on()` shape per attribute. Documented
as a known limitation; mismatches there still hit cocoa_dom's
runtime downcast and silently no-op.

### Split `Attribute` → `StringAttr` + `BoolAttr`

Replaced the single-enum design with two enums:

  * `StringAttr { Title, Value, Placeholder }` — used with
    `Element::set_string_attribute(StringAttr, &str)` /
    `remove_string_attribute(StringAttr)`.
  * `BoolAttr { Enabled, Hidden, Checked }` — used with
    `Element::set_bool_attribute(BoolAttr, bool)` /
    `remove_bool_attribute(BoolAttr)`.

Passing the wrong-type variant to the wrong setter is now a
compile error. The `Rndr`-trait `set_attribute(&str, &str)` and
`remove_attribute(&str)` entry points stay (they look up both
enums via `from_name`). The stringly-typed bool-set route
(`set_attribute("enabled", "true")`) was deliberately dropped —
the typed setter is the only blessed path for booleans.

Earlier I went with one enum specifically to satisfy the user's
"one CocoaAttribute enum" request; the follow-up "compile-error"
requirement made that incompatible. Two enums won.

### `windowWillClose:` cleanup hook

`WindowDelegate` ivars widened from a bare `Node` to a
`WindowDelegateState { root, on_close: RefCell<Option<...>> }`,
and the delegate now observes `windowWillClose:` in addition to
`windowDidResize:`. New `WindowDelegate::install_close_handler`
sets the closure to run on close.

`tachys::cocoa::window::WindowState::build` now MOVES the built
children into a close-handler closure that calls
`children.unmount()` + `content_root.teardown()`. The closure
runs once when AppKit fires `windowWillClose:` (whether the user
clicks the close button, hits Cmd-W, or calls `close()`
programmatically). `WindowState` no longer stores `children` —
ownership lives entirely in the delegate's closure. `WindowState`
itself is still leaked by `mount_to_window` / `run` (the
`Box::leak` pattern), but that's now a small fixed cost rather
than the full reactive view tree.

For multi-window apps, this means closing one window actually
releases its handler stores, Taffy nodes, and Effect
subscriptions — instead of accumulating for app lifetime.

Verified the login_form example closes cleanly with no panics.
Doesn't fully prove leak-freedom (no instrumentation for that
yet), but the code path runs and the AppKit teardown order
(`willClose:` → handler runs → window deallocates) is correct.


## Small fixes from the review list

Knocked out a handful of low-cost items from the events+attributes
review:

  * **Stale `event.rs` doc** — top-of-file comment and
    `drop_handlers_for` doc claimed cleanup is "currently never
    called". Updated to reflect that it IS called from
    `Node::teardown`, which fires via the `Mountable::unmount`
    cascade (e.g. `windowWillClose:`).
  * **`MaybeReactive::Reactive` tightened from `FnMut` to `Fn`** —
    we only ever read through the closure inside a `RenderEffect`;
    no caller needs `FnMut`. The `IntoMaybeReactive<T>` impls
    already required `Fn`, so this just matches the variant to
    its actual usage.
  * **`set_string_attribute(StringAttr::Placeholder, ...)` now
    schedules relayout** when the placeholder changes. Placeholder
    text contributes to NSTextField's intrinsic content size when
    the field is empty; previously a mid-life placeholder change
    wouldn't have triggered Taffy re-measure. (Not hit by current
    examples — placeholder is set once at build — but consistency
    matters.) Also added a same-value diff guard, matching
    Title/Value behaviour.
  * **Removed `event::on_button_click`** — was a thin wrapper over
    `on_control_action`. `Element::on_click` now calls
    `on_control_action(button.as_ref(), cb)` directly. One less API
    surface.
  * **Re-entrance `eprintln!` gated behind `#[cfg(debug_assertions)]`**
    in `ActionTarget::action_fired`,
    `TextFieldDelegate::control_text_did_change`, and
    `control_text_did_end_editing`. Release builds no longer
    write to stderr on the (rare) re-entrance case.
  * **Cleared four stale warnings**: unused `NSButton` /
    `Dimension` / `AnyThread` imports + two unnecessary `unsafe`
    blocks. cocoa_dom now builds clean.

Items left from the review:
  * Pointer-keyed handler stores (smell, not a bug today)
  * Button click vs text-field fan-out semantic inconsistency
  * `Bound{Value,Float,Index,Checked}` struct duplication →
    `Bound<T>` generic refactor
  * `IntoMaybeReactive<T>` per-type impls (combinatorial)
  * `add_any_attr` event-only (incomplete spread support)
  * `Selection` AttributeKey re-export indirection
  * `Rndr::set_attribute(&str, &str)` vestigial trait surface


## Test infrastructure: cocoa_dom unit tests

Stood up a unit-test scaffold under `cocoa_dom/tests/`. Three test
binaries — `element_creation`, `attributes`, `events` — covering the
basic NSView façade. 52 tests, all passing.

### Custom main-thread harness

Cargo's default test harness spawns a worker thread per test. AppKit
requires the main thread, so `MainThreadMarker::new()` returns
`None` from worker threads and our constructors panic.

Each test binary uses `harness = false` in `cocoa_dom/Cargo.toml`'s
`[[test]]` block and supplies its own `fn main()` via a
`run_tests(&[(name, fn_ptr), ...])` helper in
`cocoa_dom/tests/common/mod.rs`. The helper runs each test on the
binary's main thread (where AppKit is happy), catches panics with
`std::panic::catch_unwind`, and prints a libtest-style summary.

Tests look like plain `fn name()` rather than `#[test] fn`, with a
single `main` registering them. Slightly more boilerplate than
`#[test]` but the only way to keep the actual main thread.

### `fire_action` helper

Tests dispatch NSControl actions via `msg_send![target,
actionFired: control]` directly. We tried
`performSelector:withObject:` first but objc2's strict-typed
`msg_send!` rejects it: that selector's declared return type is `id`
while our `actionFired:` returns void, and even forcing the
`Option<Retained<AnyObject>>` return triggers a segfault on the
garbage return-register read.

Hardcoding `actionFired:` couples tests to the selector name
(currently in `cocoa_dom::event::ActionTarget`), but the alternative
— a generic-but-untyped invocation — would need raw `objc_msgSend`
FFI. Acceptable trade-off.

### `fire_text_did_change` / `fire_text_did_end_editing`

These invoke the NSTextFieldDelegate methods (`controlTextDidChange:`
/ `controlTextDidEndEditing:`) DIRECTLY via `msg_send!` on the
field's delegate, building a synthetic `NSNotification` with the
field as `object`.

Initially we tried posting the notifications via
`NSNotificationCenter::postNotificationName_object`. The change
notification went through (AppKit must register the delegate as an
observer when `setDelegate:` is called), but the end-editing one
did not — possibly because AppKit only delivers that one via direct
delegate invocation. Direct invocation works for both and is
independent of AppKit's opaque observer registration.

### What's tested today

  * `cocoa_dom/tests/element_creation.rs` (10 tests) — every
    supported tag (view, button, checkbox, label, text_field,
    secure_text_field, slider, pop_up_button, stack_view, unknown)
    produces the expected NSView subclass with the right
    configuration (continuous slider, pull-up popup, editable text
    field, etc.).
  * `cocoa_dom/tests/attributes.rs` (27 tests) — `StringAttr` /
    `BoolAttr` `from_name` round-trips, typed setters per variant,
    cross-type silent no-ops, removal resets, the `&str` Rndr-trait
    entry point, idempotence guards.
  * `cocoa_dom/tests/events.rs` (15 tests) — `on_click` / `on_action`
    on every NSControl subclass, the slider-on-`on_click` regression
    guard, NSControl single target/action replacement,
    TextFieldDelegate fan-out (multiple `on_input` and `on_change`
    handlers coexist), value getters (slider double_value, popup
    selection, checkbox checked).

### Not yet covered

  * Tachys-side builder tests (`Button`, `Checkbox`, ... — needs
    reactive scope setup).
  * `bind:` integration (text/checkbox/slider/popup signal round-trips).
  * Layout + Taffy compute-layout tests.
  * `windowWillClose:` cleanup verification.
  * `SupportsEvent` compile-fail tests (needs `trybuild`).
  * Any XCUIAutomation tests (separate Xcode project, deferred).


## XCUIAutomation-equivalent test tier — `xcuitests/`

Stood up an end-to-end UI test tier driven by the Accessibility
framework (AXUIElement). 7 tests passing against
`examples/login_form_macos`; ~5–6s total wall time.

### Why not XCUIAutomation literally?

XCUIApplication requires a "UI testing bundle" target, which exists
only inside Xcode .xcodeproj projects — Swift Package Manager
doesn't support that bundle type. We'd have to either:
  (a) generate + maintain an .xcodeproj (xcodegen, hand-rolled
      pbxproj, or tuist), OR
  (b) drive the app via the lower-level Accessibility framework
      from a regular SPM XCTestCase.

Went with (b). Same end-to-end fidelity (real .app launches in a
real AppKit window, real CGEvent keyboard events, real button
clicks via target/action), no Xcode project ceremony, no extra
tools to install.

### Layout

```
xcuitests/
  Package.swift
  bundle_app.sh       — wraps a cargo example as a .app bundle
  run_tests.sh        — bundle + swift test entry point
  grant_permission.sh — opens System Settings to the AX pane
  Sources/AppDriver/
    Permissions.swift — AXIsProcessTrusted check + remediation
    AXElement.swift   — Swift wrapper around AXUIElement
    AXSession.swift   — launch app, cache primary window
  Tests/LoginFormUITests/
    LoginFormUITests.swift
```

### `bundle_app.sh`

Cargo example crates aren't workspace members, so each has its own
`target/` dir. The script builds release, copies the binary into a
hand-built `.app` skeleton with a minimal Info.plist, and prints
the absolute bundle path. Tests pick it up via
`LEPTOS_MAC_APP_PATH` env var.

### `AppDriver` — AX wrapper library

The Accessibility C API is verbose (`AXUIElementCopyAttributeValue`
+ `CFBridgingRelease` boilerplate). `AppDriver` collects the dance
into:

  * `AXSession.init(bundlePath:)` — launches the .app, waits for
    its first window, caches the primary window's AXUIElement.
  * `AXElement.role` / `subrole` / `title` / `stringValue` /
    `numberValue` / `enabled` — typed attribute reads.
  * `AXElement.firstChild(role:)` /
    `firstChild(role:title:)` / `firstChild(role:subrole:)` /
    `allChildren(role:)` / `allChildren(role:subrole:)` — finders.
  * `AXElement.click()` — `kAXPressAction`.
  * `AXElement.typeText(_:)` — focuses the field, then synthesises
    real CGEvent keyboard events with
    `keyboardSetUnicodeString` so unicode chars don't need
    virtual-keycode mapping. AppKit's normal field-editor path
    fires `controlTextDidChange:`, our `bind:value` write-back
    runs, signals update.
  * `AXElement.wait(timeout:for:)` /
    `waitForDescendant(timeout:matching:)` — 25ms polling for
    reactive state to settle.

### Permission gotchas (one-time setup pain)

TCC tracks Accessibility per-binary, not per-shell, and inherits
in surprising ways:

  * `swift test` ultimately invokes `xctest` at
    `/Applications/Xcode.app/Contents/Developer/usr/bin/xctest`
    (a symlink to the MacOSX.platform agent).
  * Granting Accessibility to the parent IDE/terminal does NOT
    cascade to xctest — they have separate signed identities.
  * `AXIsProcessTrustedWithOptions(.prompt: true)` registers an
    xctest entry in System Settings on first call so the user can
    toggle it on. Without prompt:true, the binary never appears
    in the list.

`Permissions.swift` does the prompt-enabled check and throws a
clear remediation message including the parent process name (so
the user knows whether to grant to Terminal, iTerm, Cursor,
Claude Code, etc. — but ALSO that they probably need to grant to
xctest itself).

### AppKit-via-AX gotchas

  * **AX `setStringValue` is silent.** Setting `kAXValueAttribute`
    on an NSTextField updates the displayed string but does NOT
    fire `controlTextDidChange:` — that's only fired for edits
    via the field editor. Our `bind:value` write-back leg listens
    to the change notification, so AX value-sets bypass it
    entirely. `AXElement.typeText` uses CGEvent keyboard input
    instead, which goes through the field editor naturally.
  * **NSSecureTextField shares the AXTextField role.** It uses
    `subrole=AXSecureTextField` to differentiate. Tests must
    filter by `(role: AXTextField, subrole: AXSecureTextField)`
    to find the password field; plain text fields have nil
    subrole.
  * **macOS spawns extra AX windows for system UI** (password
    autofill suggestion popup, save-panel sheets, etc.). These
    show up as AXWindow nodes in the app's AX tree. The test
    submission first failed because typing into a secure field
    spawned the autofill popup, which became `windows.first` in
    the AX tree — our test then queried the autofill tree
    instead of the login form. Fixed by capturing the original
    `primaryWindow` reference once at session init and reusing
    it. See `AXSession.primaryWindow`.

### What's covered

7 tests on `login_form_macos`:
  * `test_window_present_with_title` — launch + window title.
  * `test_initial_controls_present` — role/subrole filters work
    against the actual AppKit AX tree.
  * `test_sign_in_disabled_initially` — initial enabled-state.
  * `test_sign_in_enables_with_valid_input` — full reactive
    chain: type into fields → controlTextDidChange:` → bind:value
    setter → can_submit Memo → button.enabled re-renders.
  * `test_sign_in_stays_disabled_for_short_password` — Memo
    rejection path.
  * `test_remember_checkbox_toggles` — checkbox.value round-trip.
  * `test_submit_populates_status_label` — full submit flow with
    status label written by an Effect on click.

### Follow-ups

  * Extend to `settings_macos` (slider drag, popup selection, mute
    gating).
  * Extend to `counters_macos` (For-loop dynamic children, keyed
    add/remove).
  * Per-test screenshot capture for visual regression.
  * CI matrix on a macOS runner — needs an Accessibility-granted
    xctest entry, which is a manual TCC step.


## XCUI coverage extended to settings + counters

24 tests now passing across three test targets in ~17 s.

### Multi-bundle test runner

`xcuitests/run_tests.sh` now bundles all three example apps and
sets a `LEPTOS_MAC_<NAME>_PATH` env var per bundle. Tests use the
new `AXSession.forExample("LOGIN_FORM" | "SETTINGS" | "COUNTERS")`
convenience to read the right one.

### `AXElement.dumpTree()`

Added an indented multi-line tree-printer for diagnosing test
failures against new examples. Each new test target starts with a
`disabled_test_dump_tree` placeholder (rename without the prefix to
enable). It's the fastest way to see what AppKit is actually
exposing for a given UI before writing assertions.

### SettingsUITests gotchas

  * **Slider value setting just works.** Setting
    `kAXValueAttribute` with an `NSNumber` on NSSlider fires its
    target/action — our `bind:value` outgoing leg listens via
    `Element::on_action`, so the volume signal updates and the
    label re-renders.
  * **Popup item selection requires opening the menu first.**
    Setting `kAXValueAttribute` on NSPopUpButton with the item
    title is silent (the AX value attribute reports the current
    selection but isn't writable for selection changes). The
    correct flow is `kAXShowMenuAction` → wait for the AXMenu
    children → press the matching `AXMenuItem` by title.

### CountersUITests gotchas

  * **`<hstack>` is transparent to AX.** AppKit only surfaces
    NSView containers as AXGroup when they have specific
    accessibility configuration; our plain `FlippedView`
    containers don't. So per-row buttons appear as flat children
    of the window, interleaved with the header buttons.
  * **Rows by adjacency.** Locator helpers scan
    `window.children` for adjacent (-1 button, value label,
    +1 button) triples. Each triple is one row. This is robust
    against AppKit's lack of grouping but assumes our row
    composition stays `(minus, value, plus)` in source order.

### Possible follow-up if `<hstack>` ergonomics matter for AX

Could give our hstack/vstack containers an AX role of `AXGroup`
plus an explicit `AXIdentifier` so tests have a named hook. That'd
be a small-but-pervasive change in `cocoa_dom::node::Element::create`
plus the layout module. Not pursued now — adjacency-based locators
work fine for the current shape.


## Test coverage push

127 tests passing across two tiers:

  * **103 cocoa_dom unit tests** (8 test binaries):
    - `element_creation.rs` (10) — every supported tag + class.
    - `attributes.rs` (27) — typed StringAttr/BoolAttr setters,
      removers, name lookup, Rndr trait surface, idempotence.
    - `events.rs` (15) — on_click / on_action / fan-out
      delegate / value getters.
    - `text_and_placeholder.rs` (5) — Text + Placeholder ctors.
    - `tree_mutation.rs` (11) — insert_node / remove_child /
      clear_children / ptr_eq / into_node round-trip.
    - `layout.rs` (10) — Taffy compute_layout against built
      trees: row/column direction, padding, gap, flex_grow,
      nested containers, edge cases.
    - `app_menu.rs` (7) — App > Quit ⌘Q + Edit menu (Undo,
      Redo ⇧⌘Z, Cut, Copy, Paste, Delete, Select All) selectors,
      key equivalents, nil-target first-responder dispatch.
    - `builders.rs` (18) — tachys cocoa builders: Button,
      Checkbox, Label, TextField (incl. secure), Slider,
      PopUpButton, vstack. Static + reactive attribute paths.

  * **24 XCUIAutomation-equivalent tests** in `xcuitests/`
    (login_form_macos, settings_macos, counters_macos).

### Pump helper for reactive unit tests

`common::pump_run_loop(secs)` runs the main run loop briefly via
raw FFI to `CFRunLoopRunInMode`. Needed because `RenderEffect`
schedules its rebuild on the main queue via our spawner, and
without an active run loop those scheduled futures don't fire.
Tests that mutate signals call `pump_run_loop(0.1)` between the
`signal.set` and the assertion.

### Required `pub` on builder state fields

`ElementState.el` and `LabelState.text` were previously private
(crate-internal) — fine for production where Mountable's public
surface is enough, but tests need direct access to inspect the
constructed NSView. Made them `pub` (with a doc note pointing to
`Mountable::elements()` for non-test callers).

### tests.md status

  * 141 items marked done (■) across the checklist, up from 0.
  * 191 items still pending.

### What's left

Remaining items in tests.md that aren't yet covered, grouped by
why they're deferred:

  * **Defer-by-difficulty**: off-main-thread panics (need
    threaded test setup), performance budgets (no agreed
    targets), 1k-row stress tests, memory-leak detection.
  * **Defer-by-scope**: `view!` macro expansion tests (need
    fixture apps or proc-macro test harness), components with
    `children` prop, `mount_to_window` UnmountHandle (we leak
    the Owner).
  * **Defer-by-fixture**: bind: re-entrant updates, two binds
    to same signal, explicit programmatic unmount (would need
    purpose-built tiny fixture apps; existing examples don't
    expose these edges directly).
  * **Renderer trait surface tests**: `Dom::*` forwarders →
    `cocoa_dom::Renderer::*` smokes, `CastFrom<Node>`,
    hydration stub panics. Doable as cocoa_dom unit tests but
    not yet written.
  * **Spawner lifecycle**: init / re-init / spawn_local /
    waker behaviour. Needs an async test setup with run-loop
    pumping.
