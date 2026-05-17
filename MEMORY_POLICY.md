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
| Autorelease pool | `[obj autorelease]` adds a pool retain | pool drains |
| reactive_graph arena / Owner | `Owner` parent/child tree | `Owner::drop` removes its arena entries |

Each system thinks it owns the value. The job of this codebase is to
keep them in agreement on **which one is authoritative for each
piece of state**.

For our purposes:

- **Rust is authoritative for view-tree lifetime.** Nodes, Elements,
  bundles, Effects — these die when their last Rust handle drops.
- **ObjC retains are scaffolding.** They keep AppKit/UIKit happy
  while Rust holds the master reference. When the Rust master drops,
  ObjC must also release.
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
  only TLS that's allowed is what vendored reactive_graph internals
  already use (`Owner::current`, the active subscriber). The
  framework should not add more.
- **ObjC associated objects (`objc_setAssociatedObject`).** We tried;
  they tie handler lifetime to the NSView ObjC refcount, which AppKit
  bumps in places we don't control (autorelease pools, undo manager,
  focus chain, gesture-recognizer lists). The resulting leaks are
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

> Any call that registers our Rust-owned `Retained` with an AppKit or
> UIKit subsystem must be wrapped in a tight
> `objc2::rc::autoreleasepool`.

The specific calls that need this:

- `NSText.setDelegate:` / `UITextView.setDelegate:` (and other text
  delegate calls — `NSTextView`, `NSTextStorage`).
- Any `NSNotificationCenter.addObserver:` or its UIKit equivalent.
- Any setup call documented as "the receiver creates the text
  system" or "first call initialises shared state" — AppKit
  routinely autoreleases extra retains during these lazy setups.

### Why

AppKit's `setDelegate:` (and several related calls) does not just
store our pointer as a weak ref. The first time per pool scope, it
also walks an internal setup path that briefly retains+autoreleases
the delegate. The autoreleased retain sits in the *outer* pool until
that pool drains. Our `Retained<TextViewDelegate>` is no longer the
only strong reference — there's an invisible second one.

When we later drop our `Retained`, the ObjC refcount goes from 2 to
1, not to 0. `dealloc` doesn't fire. The delegate stays alive (and
keeps holding its captured handler `SharedTextViewHandlers`) until
the outer pool drains — which may be never in a test or a fuzzer.

This shows up as a "leak" in delegate live-counters even though every
Rust reference has been dropped on time.

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

Returning `d` out of the pool is correct: the `Retained<…>` we hold
is a real refcount-1 retain, separate from the autoreleased copy.
The pool drains the autoreleased copy as the closure exits.

`NSControl` / `NSButton`'s `setTarget:` does **not** need this — it's
a pure pointer-store with no internal text-system lazy-init. The
fuzzer confirms button handlers don't leak. But if in doubt, wrap it
anyway. The cost of a redundant `autoreleasepool` is one runtime
push+pop; the cost of forgetting one is a hours-long bug hunt.

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
capture each typed `Retained` you need. Never reach for `el.clone()`.

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

### c) Forgotten autoreleasepool around setDelegate

```rust
// BAD: AppKit autoreleases an extra retain on the delegate;
// it survives our Retained::drop
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
