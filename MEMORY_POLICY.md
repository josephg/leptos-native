# Memory & ownership policy

This document is the prescriptive policy for how state is owned and
released in the cocoa and uikit ports. It exists because we have
**four** orthogonal allocators in play — Rust's `Box`/`Arc`/`Rc`,
Apple's ObjC refcount, Apple's autorelease pools, and reactive_graph's
arenas/effects — and getting the interactions wrong has produced every
single memory bug we've hit so far.

The policy is unified: there is no per-element memory model. Every
element follows the same rules. If you find yourself needing
bespoke retain logic for a new control, the rules below are wrong
(or the control is) — push back here first.

---

## 1. The five memory systems

| System | Owns lifetime via | Drop trigger |
|---|---|---|
| Rust scalar | borrow checker | scope end / explicit drop |
| `Rc` / `Arc` | reference count | last clone drops |
| ObjC `Retained<T>` | `[obj retain]` / `[obj release]` | refcount → 0 → `dealloc` |
| Autorelease pool | `[obj autorelease]` adds a deferred `release` to the pool | pool drains (which sends `release` to each pooled object) |
| reactive_graph arena / Owner | `Owner` parent/child tree | `Owner::drop` removes its arena entries |

Each system thinks it owns the value. The job of this codebase is to
keep them in agreement on **which one is authoritative for each
piece of state**.

For our purposes:

- **Rust is authoritative for view-tree lifetime.** Nodes, Elements,
  bundles, Effects — these die when their last Rust handle drops.
- **ObjC retains are scaffolding.** They keep AppKit/UIKit happy
  while Rust holds the master reference. When the Rust master drops,
  ObjC must also release. `objc2::rc::Retained<T>` represents a `+1`
  owned reference (per the standard ObjC ownership rules — methods
  named `alloc`/`new`/`copy`/`mutableCopy` return owned, others
  return autoreleased). Dropping a `Retained` calls `release` once.
- **Autorelease pools are noise we have to actively suppress.** They
  prolong lifetimes opaquely. Wherever they matter, drain them
  explicitly.
- **reactive_graph cleans up its own arena.** Don't try to manage
  signals manually; let the Owner do it.

---

## 2. Where to put state — the decision tree

Use the first row that applies. **Do not invent new storage
mechanisms.** If your case isn't covered, push back at the start of
this doc.

| What kind of state? | Put it here | Example |
|---|---|---|
| Handler / delegate `Retained` for a `Node`-backed view | Field on `NodeHandlers` (cocoa: `cocoa_dom::event::NodeHandlers`; ios: `ios_dom::event::IosNodeHandlers`) | `text_view_delegate`, `action_target` |
| Handler / delegate `Retained` for a non-Node wrapper (NSMenuItem, NSToolbarItem) | Field on the Rust wrapper struct itself; `impl Drop` that nils setTarget first | `MenuItem::action_target` |
| Per-tree state (per-window or per-scene) | Field on `LayoutTree<B>` | `LayoutTree::relayout_queued` |
| Per-Node, needed during layout | Field on `CocoaMeta` / `IosMeta` (the port's `NodeMeta`) | `intrinsic_width_from_content` |
| Process-wide counters / IDs | `static AtomicU64` | toolbar identifier generator |
| App-scoped pinning (Owners that live forever) | `Box::leak` | `mount::run` Owner pinning |

### Forbidden

- **`thread_local!` for new state.** TLS shutdown order is unspecified
  and we've already been bitten by the resulting `Drop` panics. The
  only TLS that's allowed is:
  - what vendored reactive_graph internals already use
    (`Owner::current`, the active subscriber), and
  - the app-scoped pinning carve-out described later — a single
    value or fixed-size set that lives until process exit
    (e.g. `cocoa_dom::debug_overlay::OVERLAYS`,
    `ios_dom::app::BUILDER`).

  A TLS that grows during program execution (a map keyed by ObjC
  pointers, a list of per-element handlers, anything resembling a
  registry) is **never** OK. Such state belongs on a real Rust
  owner — the appropriate `NodeHandlers` / `LayoutTree` /
  per-wrapper field per the table above. If your case doesn't fit
  one of those slots, the slot is missing; add it. Don't reach for
  TLS as the escape hatch.
- **ObjC associated objects (`objc_setAssociatedObject`).** We tried;
  they tie handler lifetime to the NSView's ObjC `dealloc`. AppKit
  routinely returns NSViews autoreleased from internal getters
  (`subviews`, `documentView`, hit-testing, drawing-rect queries),
  which delays NSView `dealloc` until the active autorelease pool
  drains — sometimes never within a single test run. Associated
  objects then survive past their logical owner. Documented Apple
  policies (e.g. `firstResponder` is `weak`, gesture recognizers
  hold their view weakly) don't save you here; the autorelease-pool
  delay alone is enough to produce the bug. The resulting leaks are
  invisible until a fuzzer or instrument catches them.
- **Sidetables keyed by ObjC pointer.** Same failure mode plus the
  pointer can be reused after dealloc.

The Node-field design replaces both of the above. Don't bring them
back.

---

## 3. Capture rules for ObjC callback closures

The single most common bug in this codebase has been "callback closure
keeps the thing it's installed on alive forever, because it captured
a strong handle that traces back to itself." The rule that prevents
this:

> A closure stored on a `Node` (or destined to be stored on one) must
> only capture **typed `Retained<NSSpecificSubclass>`** handles to
> AppKit/UIKit objects, plus plain owned data (signals, setters,
> primitive payloads). It must not capture `Node`, `CocoaElement`,
> `Element`, or any wrapper that holds an `Rc<NodeHandlersBundle>`.

### Why

`Node` holds `Rc<NodeHandlersBundle>`. The bundle holds
`Retained<ActionTarget>` (or delegate Retained). The ObjC handler's
ivars hold the closure. If the closure captures `Element` (which is
`Node`), the strong-ref chain closes:

```
Closure → Element → Rc<bundle> → Retained<handler> → ivars → Closure
```

This is an unbreakable cycle. The bundle's `Drop` (which nils
`setTarget` and `setDelegate`) never runs because the Rc never drops.

Capturing a `Retained<NSButton>` instead of an `Element` breaks the
cycle — `Retained<NSButton>` doesn't transitively reach the Rust
`Rc<bundle>`. ObjC's NSButton has no concept of the bundle; it just
holds a `target` weak pointer at our `ActionTarget`.

### How

Every element exposes `ns_view_retained()` / `ui_view_retained()` for
exactly this purpose. The bind installers in
`cocoa/leptos_cocoa/src/cocoa/bind.rs` show the canonical pattern:

```rust
let view_for_set = el.as_node().ns_view_retained();
let button: Retained<NSButton> = view_for_set
    .downcast::<NSButton>().expect("…");
RenderEffect::new(move |_| {
    button.setEnabled(getter().into());
})
```

This Effect can be installed on the same `Node`'s handlers without
forming a cycle.

### Setters are always safe

`bound.setter` (an `RwSignal::into_set()` result, `Box<dyn FnMut(T)>`)
doesn't capture anything that reaches back into the Node. Capturing it
in a delegate closure is fine and necessary.

---

## 4. The autoreleasepool rule for ObjC delegate / observer registration

This is the rule that fixed P1.

> Wrap calls to `NSText.setDelegate:` / `NSTextView.setDelegate:`
> / `UITextView.setDelegate:` (and any other setup call where you
> register a Rust-owned `Retained` and can't verify the resulting
> refcount math) in a tight `objc2::rc::autoreleasepool`.

### What Apple documents

These delegate properties are weak. Per the Apple docs:

- `NSText.delegate` — `unowned(unsafe)` (legacy non-zeroing weak).
- `NSTextView.delegate` — `weak` (zeroing).
- `NSTextField.delegate` — `weak`.
- `UITextView.delegate` — `weak`.
- `NSControl.target`, `NSMenuItem.target`, `NSToolbarItem.target` —
  `weak`.
- `UIControl.addTarget:action:forControlEvents:` — explicitly "does
  not retain the object in the `target` parameter."

By Apple's documented semantics, none of these registration calls
should retain the delegate / target. Nothing in the docs predicts
the bug below.

### What we actually observe

For `NSTextView.setDelegate:` specifically: the FIRST call within an
autoreleasepool scope leaves the delegate with `retainCount == 2`
immediately after `setDelegate` returns. Subsequent calls in the same
pool scope show `retainCount == 1` (matching the documented `weak`
behavior). Verified empirically with `[delegate retainCount]`
instrumentation; see commit history for the P1 investigation.

The most likely cause is AppKit's text-system lazy initialisation —
the first NSTextView in a given pool scope triggers setup that
briefly retains and autoreleases the delegate. Subsequent text views
reuse the already-initialised shared state and don't trip the path.
This is undocumented; treat as an implementation quirk.

The effect: when we later drop our `Retained<TextViewDelegate>`, the
ObjC refcount goes from 2 to 1, not to 0. `dealloc` doesn't fire.
The delegate stays alive (and keeps holding its captured handler
`SharedTextViewHandlers`) until the *outer* autorelease pool drains
— which may be far in the future (test end, process exit). It shows
up as a "leak" in delegate live-counters even though every Rust
reference has been dropped on time.

### How

Wrap the registration in a pool that drains immediately:

```rust
let delegate = objc2::rc::autoreleasepool(|_| {
    let d = TextViewDelegate::new(handlers.clone(), mtm);
    let proto: &ProtocolObject<dyn NSTextViewDelegate> =
        ProtocolObject::from_ref(&*d);
    tv.setDelegate(Some(proto));
    d
});
slot.text_view_delegate = Some(delegate);
```

Returning `d` out of the pool is correct ObjC semantics: our
`Retained<…>` is a real `+1` owned reference, separate from any
autoreleased entry. When the inner pool drains on closure exit, it
sends `release` to its autoreleased entries — bringing the
transient extra retain to 0 — but our `Retained` is still alive at
refcount 1. The pool boundary doesn't deallocate the owned object.

### When to apply this rule beyond the known case

- `NSControl` / `NSButton.setTarget:` does **not** need it. We
  verified with the fuzzer that button handlers don't leak. It's a
  pure pointer-store with no text-system lazy-init.
- `NSNotificationCenter.addObserver:` doesn't appear to need it
  either. On iOS 9 / macOS 10.11 and later, observers are held
  weakly and auto-cleaned on dealloc (per Apple docs); we haven't
  observed a leak there. Not pool-wrapped today.
- For any new AppKit/UIKit registration call where you store the
  returned `Retained` long-term, run a small leak test before
  trusting it. If `retainCount` immediately after the call is more
  than you can explain, wrap in a tight pool.

The cost of a redundant `autoreleasepool` is one runtime push+pop;
the cost of forgetting one is hours of bug hunting.

---

## 5. Drop discipline

### Node and bundle

`Node` is `#[derive(Clone)]`. Every clone bumps the `Rc<NodeHandlersBundle>`
strong count. The bundle drops when the last clone drops. The bundle's
`Drop` impl:

1. Calls `disconnect_view_handlers(&self.view)`, which nils the view's
   `target`/`action` and `delegate` slots. This severs the path AppKit
   might have used to dispatch into a freed closure.
2. Drops the `RefCell<NodeHandlers>` field, releasing each
   `Retained<ActionTarget>` / `Retained<TextViewDelegate>`. Those
   `dealloc` and run their `LiveTracker` decrement.

For this to be deterministic, **the bundle must drop on the main
thread**. The `SendWrapper<Retained<NSView>>` in the bundle enforces
this with a runtime check — if the bundle is somehow dropped on
another thread, the `SendWrapper::valid()` check returns false and we
skip `disconnect_view_handlers` to avoid an abort. The handlers leak
in that case; it's the lesser of two evils.

### Off-main drop

Nothing in the framework should be dropped off-main. The
`any_spawner::CustomExecutor` we install is the main-thread
`DispatchQueue`, so reactive effects' futures also drop on main.
If you add a new spawner or background thread, make sure anything
that holds `Node` / `Element` / `Retained` is moved back to main
before drop.

### Wrappers around non-Node ObjC

`MenuItem` and `ToolbarItemRegistration` are not `Node`s. They wrap
NSMenuItem / NSToolbarItem directly. The pattern there:

- Hold the `Retained<ActionTarget>` as a field on the Rust wrapper.
- `impl Drop` for the wrapper, which:
  1. Nils `setTarget:` / `setAction:` on the NSObject first.
  2. Then drops the field (releasing the `Retained`).

Same shape as the bundle's `Drop`, just hand-rolled because there's no
shared `Node`.

---

## 6. reactive_graph cleanup

`RenderEffect<T>` holds `inner: Arc<RwLock<EffectInner>>`. EffectInner
contains the channel `Sender` and the `SourceSet` (signals this effect
subscribes to). Signal subscribers and effect sources are both
`Weak` — there are no Arc cycles inside the reactive graph. When the
last `RenderEffect` handle drops:

1. The inner `Arc` drops, `EffectInner` drops, `Sender` drops.
2. The channel's `Inner::drop` wakes the receiver.
3. The spawned async future polls, `Receiver::poll_next` sees the
   `Weak::upgrade` fail, returns `Ready(None)`.
4. The async future completes; the spawner drops the boxed future.
5. Dropping the future drops `fun`, releasing whatever the closure
   captured (the `Retained<NSButton>`, the `getter`, the `setter`).

For this chain to release a `Node` clone, **the closure must not
capture an `Element` / `Node`**. See section 3.

`Owner`-scoped signals and stored values are cleaned up by
`Owner::drop` (or `with_cleanup` mid-iteration). Don't try to
manually `dispose` signals — the Owner tree handles it.

---

## 7. Worked example: a new control with target/action + signal binding

Suppose we add `<rating>` — a 5-star NSControl analog with
`bind:value=signal`.

1. **Create the view in `node.rs`**: `Element::create_with("rating", mtm)`
   matches a new tag, allocates `RatingControl` (subclass of
   `NSControl`), wraps in `Retained<NSView>`, builds a `Node` via
   `Node::from_view`. The new Node's `NodeHandlersBundle` is created
   fresh.

2. **Wire the target/action in `event.rs`**: `on_rating_change`
   takes `&Node` and `cb: impl FnMut(u8)`. Internally:
   ```rust
   let target = ActionTarget::new(...);
   unsafe { control.setTarget(Some(&target)); control.setAction(Some(sel!(actionFired:))); }
   node.handlers().borrow_mut().action_target = Some(target);
   ```
   Nothing autoreleased here, so no pool-wrap needed (NSControl's
   setTarget is the simple case).

3. **Bind it in `bind.rs`**: `install_rating_value_bind` looks like:
   ```rust
   pub(crate) fn install_rating_value_bind(
       el: &CocoaElement, bound: BoundU8,
   ) -> RenderEffect<()> {
       let mut setter = bound.setter;
       el.on_rating_change(move |v| setter(v));     // outgoing, no Element captured

       let getter = bound.getter;
       let control: Retained<RatingControl> = el
           .as_node().ns_view_retained()
           .downcast::<RatingControl>().expect("rating control");
       RenderEffect::new(move |_| {                  // incoming, capture Retained
           let v = getter();
           if control.value() != v { control.setValue(v); }
       })
   }
   ```

4. **No tests of bundle Drop needed** — the existing `leak_lifecycle`
   suite asserts the patterns above; if your new builder follows
   them, leak counters return to baseline automatically.

If you find yourself wanting to capture `el` in the Effect because
"I need to call multiple methods on it" — store a `Retained<RatingControl>`
once and call methods on that. If you need *several* AppKit subviews,
capture each typed `Retained` you need.

### Exception: side-effecting setters

The typed-`Retained` capture works when the only thing the Effect
closure does is invoke an AppKit method (and read back for the
diff guard). If the setter must *also* trigger Rust-side
bookkeeping — typically a `schedule_relayout` call after a content
change that affects `intrinsicContentSize` — route through the
`Element` layer (`el_for_set.set_string_attribute(...)` /
`el_for_set.set_intrinsic_*(...)`) so that bookkeeping runs.

Examples that require the `Element` route today:
- `<text_field>` / `<text_view>` `bind:value=` incoming write —
  `Element::set_string_attribute` re-runs `schedule_relayout` so
  intrinsic-width-from-content settles after each change.
- Anything else that adjusts a Taffy measure-dependent property
  (font size on a label, image dimensions on an image_view).

The `el.clone()` capture in those cases is still cycle-safe under
§3 — the closure lives on `ElementState::_effects`, *not* in the
Node's handler bundle. The cycle rule only applies to closures
stored in handler ivars (delegates, target/action). Closures owned
by a `RenderEffect` that drops with the element state can safely
hold an `Element` clone.

Empirically: bypassing `schedule_relayout` for a string setter
during chaos shows up as a delegate-store leak in the fuzzer
(Taffy ends up with stale measure cache; the affected node's
state isn't unmounted cleanly on teardown). Don't try to be clever
here — when in doubt, prefer the `Element` route and inline
typed-`Retained` capture only for pure numeric / boolean setters.

Never reach for `el.clone()` to *avoid* understanding the lifecycle.
Use it deliberately when (a) the closure isn't in a handler bundle
and (b) you need an Element-layer side effect that has no typed
analogue.

---

## 8. Anti-pattern catalogue

These are the bugs we've fixed. Pattern-match against them in code
review.

### a) Element-cycle bind

```rust
// BAD: cycle through Rc<bundle>
let el_for_set = el.clone();
RenderEffect::new(move |_| el_for_set.set_…())
```

```rust
// GOOD: typed Retained capture
let control: Retained<NSButton> = el.as_node().ns_view_retained()
    .downcast::<NSButton>().unwrap();
RenderEffect::new(move |_| control.setEnabled(…))
```

### b) Sidetable keyed by NSView pointer

```rust
// BAD: AppKit retains the view in places we don't control,
// so the sidetable entry leaks
thread_local! { static HANDLERS: RefCell<HashMap<*mut NSView, …>> = … }
```

```rust
// GOOD: state lives on Node's NodeHandlersBundle
node.handlers().borrow_mut().my_field = Some(…)
```

### c) Forgotten autoreleasepool around `NSTextView.setDelegate`

```rust
// BAD: empirically, the first setDelegate per pool scope leaves an
// extra retain on the delegate. It survives our Retained::drop and
// the delegate stays alive until the outer pool drains.
tv.setDelegate(Some(proto));
slot.text_view_delegate = Some(delegate);
```

```rust
// GOOD: tight pool drains the extra retain before we hand off
let delegate = objc2::rc::autoreleasepool(|_| {
    tv.setDelegate(Some(proto));
    delegate
});
slot.text_view_delegate = Some(delegate);
```

See §4 for the empirical-vs-documented distinction.

### d) New thread_local for framework state

```rust
// BAD
thread_local! { static MY_REGISTRY: RefCell<…> = …; }
```

Use a field on the appropriate owner (Node / LayoutTree / wrapper
struct). If you genuinely need process-global, use a
`static AtomicU64` for IDs, or `Box::leak` for app-scoped pinning.
TLS introduces shutdown-order bugs you'll only catch in production.

### e) Drop running on a background thread

```rust
// BAD: spawn a tokio task that captures an Element
tokio::spawn(async move { let _ = element; });
```

```rust
// GOOD: use the main-thread spawner
use any_spawner::Executor;
Executor::spawn_local(async move { let _ = element; });
```

### f) Manual signal disposal

```rust
// BAD: tries to outsmart the Owner
let sig = RwSignal::new(…);
// later
sig.dispose();
```

```rust
// GOOD: scope the Owner; signals die with it
let owner = Owner::new();
owner.with(|| { let sig = RwSignal::new(…); … });
drop(owner);
```

---

## 9. Quick reference

When adding a new feature, ask in order:

1. **Where does this state belong?** → §2 table.
2. **If it's a callback closure, what does it capture?** → §3 rule.
3. **Am I calling an ObjC setDelegate-style method?** → §4 wrap in
   autoreleasepool.
4. **How does it get dropped?** → §5 (main-thread, bundle's `Drop`
   nils first, then releases).
5. **If it's a reactive effect, what does the Effect's closure
   capture?** → same as §3.

If your answer to (1) is "I need a new storage mechanism," stop and
re-read this doc — odds are one of the existing slots already fits.

---

## 10. Why one policy and not bespoke per-element rules

Every memory bug in this codebase's history (handler-leak,
TLS-shutdown panic, scroll-view dangling delegate, P1 text_view
delegate leak) came from a one-off design that worked for the
control it was written for and broke under a slightly different
shape. The unified policy:

- Makes new controls a copy of existing controls. No design work
  per element.
- Concentrates the AppKit/UIKit weirdness in two places (the bundle
  `Drop` and the `setDelegate`-pool rule) where it can be tested
  once and trusted.
- Makes leak tests reusable: the same `leak_lifecycle` shape
  catches every regression across every control, because every
  control uses the same ownership skeleton.

Don't add a new clever mechanism. If a control genuinely doesn't fit
this policy, the policy is wrong and we update it here, not in the
control.
