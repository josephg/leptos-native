# Dynamic Attributes

Most attributes accept either a literal value or a reactive closure.

```rust
// literal
<button enabled=true on:click=...>"Save"</button>

// reactive — closure read on every signal change
<button enabled=move || can_save.get() on:click=...>"Save"</button>
```

This applies to nearly every builder method on every element:
`title`, `value`, `enabled`, `padding`, `gap`, `text_color`,
`alpha`, `tool_tip`, and so on. If you pass a closure, the runtime
wires up a `RenderEffect` that re-runs the setter whenever a signal
read inside the closure changes.

## Static vs reactive: a worked example

```rust
let count = RwSignal::new(0);
let limit = 10;

view! {
    // Static — set once at build time.
    <label text="Counter".to_string() />

    // Reactive — re-evaluated each time `count` changes.
    <label text=move || format!("Count: {}", count.get()) />

    // Mixed-source reactive — both `count` and a captured constant.
    <button enabled=move || count.get() < limit on:click=...>
        "Increment"
    </button>
}
```

If you forget `move ||`, you read the signal *once* at build time
and the attribute becomes a one-shot snapshot:

```rust
// BAD — reads count.get() once, never updates.
<label text=format!("Count: {}", count.get()) />
```

## Children as text

Text children inside angle-bracket tags work the same way:

```rust
<label>"static"</label>
<label>{count.get().to_string()}</label>          // one-shot
<label>{move || count.get().to_string()}</label>  // reactive
```

## Two-way bindings (`bind:*`)

Some attributes have a two-way variant. `bind:value=signal` on a
`<text_field>` does both: writes signal → field, and field
edits → signal.

```rust
let name = RwSignal::new(String::new());
view! { <text_field bind:value=name /> }
```

The supported keys are:

| Key             | Signal type                      | Elements                         |
|-----------------|----------------------------------|----------------------------------|
| `bind:value`    | `RwSignal<String>`               | `text_field`, `secure_text_field`, `text_view` |
| `bind:value`    | `RwSignal<f64>`                  | `slider`, `stepper`              |
| `bind:value`    | `RwSignal<usize>`                | `pop_up_button` *(Cocoa, GTK)*   |
| `bind:value`    | `RwSignal<Date>`                 | `date_picker`                    |
| `bind:value`    | `RwSignal<Color>`                | `color_well` *(Cocoa)*           |
| `bind:checked`  | `RwSignal<bool>`                 | `checkbox` *(Cocoa/GTK)*, `switch` *(iOS)* |
| `bind:selection`| `RwSignal<usize>`                | `segmented_control`              |

You can also pass a `(getter_fn, setter_fn)` tuple in place of an
`RwSignal` if you need to derive one direction or filter the
other. See [Forms and Inputs](./05_forms.md).

## Events (`on:*`)

Events use the same `on:` prefix you know from web Leptos:

```rust
<button on:click=move |_| println!("clicked")>"Click me"</button>
```

Event handlers are `FnMut` closures. The payload type depends on
the event:

- `click`, `change`, `focus`, `blur`, `action` — `()`. Read the
  bound signal for any associated value.
- `input`, `commit` — `String` (text-field only).
- `keydown`, `keyup` — `KeyEvent`.

Each element only supports a subset of events; passing an unsupported
one is a compile error. See the individual element pages in the
[Element Reference](../elements/README.md).

## Coexistence

You can mix `bind:value` with `on:input` / `on:change` on the same
field. They all share one underlying delegate:

```rust
<text_field
    bind:value=email
    on:input=move |_| keystroke_count.update(|c| *c += 1)
    on:commit=move |v: String| last_committed.set(v) />
```

This is taken straight from the `checkbox` example
(`cocoa/examples/checkbox/src/main.rs`).

## A note about diffing

On every backend, `set_attribute` diffs against the current widget
state before mutating. This is what makes `bind:` cycles safe — an
incoming signal write that matches the current value doesn't fire
the widget's change notification and re-trigger the effect.
