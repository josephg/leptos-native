# Effects

An effect is "code that re-runs when its dependencies change."
Effects are how you bridge reactive state to *imperative* side
effects: logging, persisting to disk, talking to a non-reactive
API.

```rust
let count = RwSignal::new(0_i32);

Effect::new(move |_| {
    println!("count is now {}", count.get());
});
```

The closure runs once when the effect is created, and again
whenever a signal it reads (`count.get()` here) changes.

## Anatomy

```rust
Effect::new(move |previous_value: Option<T>| -> T { ... })
```

The closure receives the *previous* return value (or `None` on
the first run) and returns a new one. Most effects don't care
about the previous value and return `()`:

```rust
Effect::new(move |_| {
    save_to_disk(&value.get());
});
```

You can use the return value to compare against the previous and
skip work:

```rust
Effect::new(move |last: Option<i32>| {
    let now = count.get();
    if Some(now) != last {
        save_value(now);
    }
    now
});
```

## When to use effects

- Persistence — write a signal to disk / `UserDefaults` /
  `GSettings`.
- Logging — emit a log line on state change.
- Talking to platform APIs — call a non-reactive system call
  whenever something changes.
- Imperative interop — call out to a NodeRef'd element's
  imperative methods.

## When NOT to use effects

- **Don't use an effect to derive a value from other signals.**
  Use `Memo` for that. Effects are for side effects, not pure
  computation.
- **Don't use an effect to write into a signal that other reactive
  code reads.** That creates a loop. (Leptos's runtime will catch
  some of these, but not all.) Use a `Memo` or compose closures
  instead.

## Tracking

The closure body is a *reactive scope*: every `.get()`,
`.with()`, `.read()` it calls subscribes the effect to that
signal. If a signal isn't read in the closure body, the effect
doesn't subscribe to it.

That means conditional reads matter:

```rust
Effect::new(move |_| {
    if enabled.get() {
        // Only subscribes to `value` while enabled is true.
        println!("{}", value.get());
    }
});
```

The first time `enabled` becomes `false`, the effect re-runs but
doesn't read `value`, so it unsubscribes. Toggle `enabled` back
to `true` and it re-subscribes on the next run.

To explicitly *not* track a read inside an otherwise reactive
closure, use `untrack`:

```rust
Effect::new(move |_| {
    let now = enabled.get();
    let snapshot = untrack(|| value.get_untracked());  // also fine: just use _untracked
    /* ... */
});
```

## `RenderEffect`

Internally, the renderer uses `RenderEffect` for view-tree updates
— that's what turns `move || count.get()` inside a `<label>`
into a re-rendering label. You'll rarely create one directly;
`Effect::new` is the user-facing API.

## Lifetimes

Effects are tied to the current `Owner`. When the owner is
dropped (component unmounted, window closed), all its effects
are cancelled and won't fire again.

If you need an effect that outlives its owner, use
`Effect::new_isolated` or stash an `ArcRwSignal`-driven effect
on a longer-lived owner. This is rare in practice — let the
owner tree handle cleanup unless you have a specific reason not
to.
