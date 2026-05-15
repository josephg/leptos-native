# Working with Signals

Signals are reactive cells. They have a current value, and any
piece of code that reads them gets re-run when the value changes.

## `RwSignal<T>` — the default

```rust
let count = RwSignal::new(0_i32);
count.get();             // read (subscribes the current reactive scope)
count.set(42);           // overwrite
count.update(|n| *n += 1); // mutate in place
count.with(|n| *n + 1);  // read by reference (no clone)
count.read();            // borrow guard (no clone, no copy)
count.write();           // mutable borrow guard
count.get_untracked();   // read without subscribing
```

`RwSignal<T>` is the right default for state inside a component.
It's `Copy` (it's a handle into an arena), so you don't need to
clone it when passing into closures.

## `signal()` — split (`ReadSignal`, `WriteSignal`)

If you want explicit read-only / write-only halves:

```rust
let (count, set_count) = signal(0_i32);

count.get();                  // ReadSignal: read-only
set_count.set(42);            // WriteSignal: write-only
set_count.update(|n| *n += 1);
```

Use this when the type signature is documentation — e.g. a child
component that only needs to write to a parent's state takes
`set: WriteSignal<bool>`.

## `Signal<T>` — read-only erased

`Signal<T>` is a read-only view that can wrap either a
`ReadSignal<T>`, an `RwSignal<T>`, a `Memo<T>`, or even a
plain closure. Use it as a prop type when the component just
needs *some* source of `T` over time and doesn't care which.

```rust
#[component]
fn Display(value: Signal<i32>) -> impl IntoView {
    view! { <label>{move || value.get().to_string()}</label> }
}
```

## Read access patterns

| Method            | Subscribes? | Returns          | Use when                          |
|-------------------|-------------|------------------|-----------------------------------|
| `.get()`          | yes         | `T` (cloned)     | inside a reactive closure         |
| `.with(\|v\| …)`    | yes         | whatever you return | avoid clone of expensive values   |
| `.read()`         | yes         | borrow guard     | streaming reads with no clone     |
| `.get_untracked()`| no          | `T` (cloned)     | event handlers / one-shot snapshots |
| `.with_untracked()`| no         | result of closure | event handlers, no clone           |

`.get()` is the most common; reach for the others when:

- `.with()` — your value is `Vec<...>` or a large struct and
  cloning is wasteful.
- `.get_untracked()` — you're in an event handler or `Effect` and
  you don't want this read to track as a dependency. (Event
  handlers aren't inside a tracking scope anyway, so this is
  documentation rather than a behavioral change.)

## Write access patterns

| Method                  | Use when                                       |
|-------------------------|-------------------------------------------------|
| `.set(new)`             | replace wholesale                              |
| `.update(\|v\| ...)`     | mutate in place                                |
| `.write()`              | mutable borrow guard for streaming writes      |

`.update(|v| *v += 1)` is the idiomatic single-step mutation.

## `Arc`-flavored signals

`ArcRwSignal<T>` (and `ArcSignal`, `ArcReadSignal`, etc.) are the
reference-counted variants. They have heap-allocated storage and
their own ownership, so they outlive the current `Owner` —
useful for state shared with async tasks or stored in
long-lived collections.

The arena-allocated `RwSignal<T>` is `Copy` and faster to clone,
but it's destroyed when its owner is dropped. Stick with
`RwSignal` unless you specifically need a longer lifetime.

## Don't forget `move ||`

The single most common bug: writing `value=count.get()` instead
of `value=move || count.get()`. The first is a one-shot read at
build time; the second is a reactive closure that re-runs when
`count` changes.

```rust
// frozen at build time — won't update
<label text=count.get().to_string() />

// reactive — updates on every change
<label text=move || count.get().to_string() />
```
