# Memos

A `Memo<T>` is a derived value: a pure computation over other
signals. Memos cache their result and only re-compute when one of
their dependencies actually changes.

```rust
let username = RwSignal::new(String::new());
let password = RwSignal::new(String::new());

let can_submit = Memo::new(move |_| {
    !username.get().is_empty() && password.get().len() >= 8
});

view! {
    <button enabled=move || can_submit.get() ...>"Sign in"</button>
}
```

Reading `can_submit.get()` is cheap — the result is cached.
Reading the same `Memo` from ten different `enabled=` closures
doesn't re-compute the boolean ten times; it computes once and
fans out.

## When to use a memo

Use a memo when:

1. The same derived value is read in more than one place.
2. The derivation is non-trivially expensive (string formatting in
   a tight loop, filtering a large list, doing math).
3. You want to *prune* an updates fan-out. Memos only notify their
   subscribers when the new value differs from the old (by
   `PartialEq`). A chain of `Memo → Memo → Memo` will stop
   propagating as soon as a value stabilises.

Don't reach for a memo for one-off derivations:

```rust
// Fine — this expression runs once per render anyway.
<label text=move || format!("Total: {}", count.get() * 2) />
```

## Pruning

Memos compare via `PartialEq`. If the new value equals the old,
subscribers don't see a change.

```rust
let count = RwSignal::new(0);
let is_even = Memo::new(move |_| count.get() % 2 == 0);

Effect::new(move |_| {
    println!("is_even changed to {}", is_even.get());
});

// count: 0 → 2 → 4 → 6 — is_even stays true, effect doesn't refire.
// count: 6 → 7 — is_even goes false, effect fires once.
```

This is what makes memo chains efficient: a complicated
derivation that ends in a `bool` only re-fires its downstream
when the boolean actually flips.

## `ArcMemo`

`ArcMemo<T>` is the reference-counted variant — heap-allocated,
not arena-allocated. Use it when the memo needs to outlive the
current owner, or when you want explicit ownership semantics.

## The previous-value parameter

Just like effects, the memo closure receives the previous
result:

```rust
Memo::new(move |prev: Option<&i32>| {
    let now = source.get();
    // Could be used for incremental computation.
    now
})
```

Most memos ignore it. It's there for the (rare) case where
re-computing from scratch is more expensive than updating
incrementally.

## `AsyncDerived`

For derived values that are *async* — fetched from a server,
computed in a worker, etc. — there's `AsyncDerived<T>` /
`ArcAsyncDerived<T>`. The closure returns a future:

```rust
let user = AsyncDerived::new(move || async move {
    fetch_user(user_id.get()).await
});

// Use inside an Effect that awaits, or directly via .get() which
// returns Option<T> until the future resolves.
```

The native fork has no `Resource` or `Suspense` — async data flow
is handled with `AsyncDerived` plus your own `Effect` / `<Switch>`
wiring.
