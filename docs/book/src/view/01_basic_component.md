# A Basic Component

The smallest useful Leptos program looks like this:

```rust
use leptos::prelude::*;

#[component]
fn Counter() -> impl IntoView {
    let count = RwSignal::new(0);
    view! {
        <vstack padding=16.0 gap=12.0>
            <label>{move || format!("Count: {}", count.get())}</label>
            <button on:click=move |_| count.update(|n| *n += 1)>"+1"</button>
        </vstack>
    }
}

fn main() {
    mount_to_window("Counter", (320.0, 200.0), || {
        view! { <Counter /> }
    });
}
```

There are five things to notice.

## 1. `#[component]`

`#[component]` turns a Rust function into a component. The function's
**arguments become props**, and the body produces a view.
Component names use `PascalCase`, but the function itself is just
a Rust function — there's no class, no inheritance.

You invoke a component with JSX-like syntax in `view!{}`:

```rust
view! { <Counter /> }
view! { <Counter initial=5 /> }
```

## 2. `view!{}`

`view!{}` is a macro that turns angle-bracket syntax into a tree of
builder calls. On the web, that tree would build DOM nodes; in this
fork, it builds platform-native widgets (`NSView`, `UIView`, or
`gtk::Widget`).

Inside `view!{}` you can:

- Open native-tag elements: `<vstack>`, `<label>`, `<button>`,
  `<text_field>`, etc. See the [Element Reference](../elements/README.md).
- Set attributes as either literal values (`padding=16.0`) or
  reactive closures (`title=move || count.get().to_string()`).
- Wire events with `on:click=` (and other `on:*` keys).
- Set up two-way bindings with `bind:value=` and friends.
- Embed children — both static (`"Hello"`) and dynamic (`{move ||
  count.get()}`).
- Invoke other components like elements: `<Counter />`.

Strings inside `view!{}` need to be Rust string literals — note the
quotes around `"+1"` above.

## 3. Signals

```rust
let count = RwSignal::new(0);
```

A `RwSignal<T>` is a reactive cell. It has:

- `count.get()` — read the value, tracking it as a dependency of
  the current effect / view.
- `count.set(v)` — replace the value.
- `count.update(|v| ...)` — mutate in place.
- `count.with(|v| ...)` — read by reference (no clone).

When a signal changes, anything that read it via `get()` /
`with()` / `read()` re-runs. Inside `view!{}`, that's how a `<label>`
re-renders when `count` updates without you manually touching the
label.

There are also `(getter, setter)` pairs from `signal()`,
`Signal<T>` (read-only), `Memo<T>` (derived), `Effect` (side
effects), and stores. See [Working with Signals](../reactivity/signals.md).

## 4. `mount_to_window`

```rust
fn main() {
    mount_to_window("Counter", (320.0, 200.0), || {
        view! { <Counter /> }
    });
}
```

`mount_to_window` is the macOS entry point. It opens an NSWindow
with the given title and size, runs the closure inside a fresh
`Owner` (so signals/effects in it get cleaned up correctly when
the window closes), mounts the resulting view, and starts the
AppKit main loop.

On Linux/GTK the signature is similar but takes a leading
application ID:

```rust
mount_to_window(
    "org.example.counter",
    "Counter",
    (320, 200),
    || view! { <Counter /> },
);
```

On iOS there's only `run`:

```rust
leptos::mount_ios::run(|| view! { <Counter /> });
```

## 5. Closures everywhere

Anywhere a value can change over time, you pass a closure that
*reads* the signal:

```rust
view! { <label>{move || format!("Count: {}", count.get())}</label> }
```

The closure is wrapped in a `RenderEffect`. When `count` changes,
the effect re-runs and the label's text updates. Without `move ||`,
you'd be passing a static `String` that's evaluated once at
component build time and never updated.

This is the single most important pattern in Leptos: **`move ||
signal.get()` is reactive; `signal.get()` is a one-shot read.**

## Next

The next chapter — [Dynamic Attributes](./02_dynamic_attributes.md)
— covers how reactive closures plug into attribute values too,
not just text children.
