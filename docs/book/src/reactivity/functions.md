# Reactivity and Functions

There's a single rule that explains 90% of the surprises you'll
hit working with Leptos:

> **Reactivity flows through *functions*, not through *values*.**

A signal's value isn't reactive. A *reference* to a signal isn't
reactive. The thing that's reactive is a **closure that reads the
signal**.

## What this looks like in practice

```rust
let count = RwSignal::new(0);

// This is a `String`. It was computed once. It's frozen.
let label_text = format!("Count: {}", count.get());

// This is a `Fn() -> String`. It re-computes on every signal change.
let label_text = move || format!("Count: {}", count.get());

// Inside view! { ... }:
<label text=label_text />     // depends on which version above
```

Pass a value, and you get a snapshot. Pass a closure, and you get
reactivity.

## Why this matters in `view!{}`

Most native-element attribute setters accept either:

- A static value: `padding=16.0`, `enabled=true`,
  `text="Hello".to_string()`.
- A closure that returns the same type:
  `padding=move || pad.get()`, `enabled=move || can_save.get()`.

The macro doesn't introduce reactivity by magic. If you read a
signal in expression position and don't wrap it in `move ||`, it's
read once.

Same with children:

```rust
<label>{count.get()}</label>          // one-shot
<label>{move || count.get()}</label>  // reactive
```

## Closures are the unit of reactive granularity

A `RenderEffect` or `Effect` runs your closure and subscribes to
every signal it reads. When any of those signals changes, the
closure re-runs.

That means smaller closures = finer reactivity:

```rust
// Two effects, each with one dependency. Re-runs only when their
// own input changes.
view! {
    <label text=move || format!("Name: {}", name.get()) />
    <label text=move || format!("Age:  {}", age.get()) />
}

// One effect, two dependencies. Re-runs when either changes —
// rebuilds both strings every time.
let combined = move || (format!("Name: {}", name.get()),
                        format!("Age:  {}", age.get()));
view! {
    <label text=move || combined().0 />
    <label text=move || combined().1 />
}
```

You don't *usually* care about this level of optimisation in
practice — strings are cheap. But the principle generalises: a
closure that reads three signals is subscribed to all three.

## Functions, methods, and the tracking scope

A method call inside a tracking closure inherits the closure's
scope:

```rust
fn full_name(first: RwSignal<String>, last: RwSignal<String>) -> String {
    format!("{} {}", first.get(), last.get())
}

// Effect subscribes to BOTH `first` and `last` through this call.
Effect::new(move |_| {
    let n = full_name(first, last);
    println!("{}", n);
});
```

You can build up reactive helpers by stacking closures: a function
that takes signals and returns a `String` is just a normal Rust
function; calling it inside a tracking scope subscribes to the
signals it reads.

## The "I forgot `move ||`" antipattern

This bug shows up in every Leptos project at least once:

```rust
let greeting = format!("Hello, {}", name.get());
view! { <label text=greeting /> }
```

`greeting` is computed *once*. It will never update when `name`
changes. The fix is one tiny edit:

```rust
let greeting = move || format!("Hello, {}", name.get());
view! { <label text=greeting /> }
```

If your UI stops updating, your first hypothesis should always be
"I dropped a `move ||` somewhere."
