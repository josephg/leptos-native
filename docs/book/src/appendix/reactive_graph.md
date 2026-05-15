# How the Reactive System Works

This appendix is a conceptual walkthrough of the runtime that
makes signals, effects, memos, and views all stay in sync. You
don't need to understand any of it to use the framework, but it
helps when debugging weird reactivity bugs.

## The graph

Every reactive primitive is a node in a directed graph:

- **Source nodes** (`RwSignal`, `ArcRwSignal`) — hold a value.
  They have no inputs.
- **Derived nodes** (`Memo`, `AsyncDerived`) — compute their
  value from other nodes. They have inputs and an output.
- **Effect nodes** (`Effect`, `RenderEffect`) — run side
  effects. They have inputs but no value other reactive code can
  read.

Edges in the graph go from a node to its *subscribers*: every
node that needs to be notified when the source changes.

## Tracking

The graph is built **dynamically**, by observation. When a node
is computing its value (memo) or running its body (effect), the
runtime sets it as the current "observer". Any signal whose
`.get()` is called *while* an observer is active records that
observer as one of its subscribers.

This is why `move ||` matters: the closure is what gets re-run.
The reads inside the closure are observed in the closure's
execution context, and *those* reads are what create the
dependency edges.

```rust
Effect::new(move |_| {
    // This effect is the current observer for the duration of this closure.
    // count.get() will register this effect as a subscriber of `count`.
    println!("{}", count.get());
});
```

If `count` changes, the runtime walks `count.subscribers` and
re-runs each. The effect re-runs, which re-tracks its
dependencies — so conditional reads (signals only read when a
flag is true) get re-subscribed correctly each pass.

## Pruning

A `Memo` doesn't notify its subscribers unless its computed
value actually changed (by `PartialEq`). This is what allows
chains of derivations to stay efficient: a memo computing
`is_even` will mark itself as "didn't really change" between
0 → 2, suppressing downstream updates.

This is also what makes `set_attribute`'s widget-state diff
work: an `Effect` driving an attribute won't bother rewriting
the widget if the new value equals the old.

## Scheduling

Updates aren't immediate. When a signal's `.set()` is called, the
runtime marks each subscriber as "dirty" and queues a flush.
Effects don't run synchronously inside `.set()`; they run when
the queue is drained, typically at the end of the current task.

This batching is what makes a `update` of two related signals
behave as one update from the view's perspective:

```rust
user.set(new_user);
session.set(new_session);
// Effects depending on either or both run once each, not twice.
```

## Owners

Reactive nodes are owned by an `Owner`. When the owner is
dropped, all its descendants are dropped — signals are
disposed, effects are cancelled, memos lose their cache.

Components, `<For>` rows, and conditional branches each create a
child owner. When you `<Show when=...>` flips false, the owner
for the children is dropped along with their signals and
effects.

This is what makes per-row state in `<For>` clean up correctly
when a row is removed.

## `untrack`

Sometimes you want to read a signal *without* recording a
subscription:

```rust
let snapshot = untrack(|| signal.get_untracked());
```

`get_untracked` does this for a single read; `untrack(|| { ... })`
does it for a block. Use it when you're inside a tracking scope
(another effect / memo) but don't want this particular read to
trigger re-runs.

## Where the runtime lives

`common/reactive_graph/` contains the implementation. The same
crate is used unchanged from upstream Leptos — the reactivity
model is what carries over verbatim.

## Further reading

- [Working with Signals](../reactivity/signals.md)
- [Effects](../reactivity/effects.md)
- [Memos](../reactivity/memos.md)
- [Reactivity and Functions](../reactivity/functions.md)

The original [Leptos Book](https://book.leptos.dev/) contains an
extended discussion of fine-grained reactivity in the web
context. The runtime is the same; the rendering layer is what
differs.
