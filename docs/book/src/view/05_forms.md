# Forms and Inputs

A "form" on native is just a stack of input controls with a submit
button — there's no `<form>` element, no `action=`, no
`<ActionForm>`. You manage state with signals and react to a
button click.

```rust
#[component]
fn LoginForm() -> impl IntoView {
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let remember = RwSignal::new(false);
    let status   = RwSignal::new(String::new());

    let can_submit = Memo::new(move |_| {
        !username.get().is_empty() && password.get().len() >= 8
    });

    let on_submit = move |_| {
        status.set(format!(
            "Signed in as {} (remember={})",
            username.get_untracked(),
            remember.get_untracked()
        ));
    };

    view! {
        <vstack padding=16.0 gap=8.0>
            <label>"Sign in"</label>

            <text_field bind:value=username placeholder="Username" />
            <secure_text_field bind:value=password placeholder="Password (8+ chars)" />

            <checkbox bind:checked=remember>
                "Remember me on this device"
            </checkbox>

            <button
                enabled=move || can_submit.get()
                on:click=on_submit>
                "Sign in"
            </button>

            <label>{move || status.get()}</label>
        </vstack>
    }
}
```

Four ideas show up in almost every form:

## 1. `bind:value` and `bind:checked`

Each field owns a signal, and `bind:value=` (or `bind:checked=`)
keeps the field and the signal synchronised in both directions.

You can mix in additional `on:input` / `on:change` handlers if you
need to do something beyond the binding — count keystrokes, log
validation events, persist on every commit, etc. They all
coexist:

```rust
<text_field
    bind:value=email
    on:input=move |_| keystroke_count.update(|c| *c += 1)
    on:change=move |v: String| last_committed.set(v) />
```

## 2. `Memo` for derived validity

`can_submit` is a `Memo<bool>` — re-computed whenever
`username` or `password` changes. Use a memo, not a closure read
inline in `enabled=`, when the same derived value is referenced
in multiple places (you'd otherwise be doing the same work twice).

```rust
let can_submit = Memo::new(move |_|
    !username.get().is_empty() && password.get().len() >= 8
);

<button enabled=move || can_submit.get() ...>
```

For one-off uses, `enabled=move || …expression…` is fine.

## 3. `get_untracked` inside event handlers

Event handlers are not inside a tracking context, so calling
`signal.get()` inside one is identical to `signal.get_untracked()`.
Using `get_untracked` is purely a documentation choice: it makes
clear at the call site that you're taking a snapshot, not
subscribing.

```rust
let on_submit = move |_| {
    let user = username.get_untracked();
    /* ... */
};
```

## 4. `placeholder=` on text fields

`<text_field>` and `<secure_text_field>` accept a `placeholder=`
prop — the greyed-out hint text shown when empty.

```rust
<text_field bind:value=name placeholder="Your name" />
```

## Field types at a glance

| Builder                | Bind key     | Signal type        |
|------------------------|--------------|--------------------|
| `text_field`           | `bind:value` | `RwSignal<String>` |
| `secure_text_field`    | `bind:value` | `RwSignal<String>` |
| `text_view` *(Cocoa/iOS)* | `bind:value` | `RwSignal<String>` |
| `checkbox` *(Cocoa/GTK)* / `switch` *(iOS)* | `bind:checked` | `RwSignal<bool>` |
| `slider`               | `bind:value` | `RwSignal<f64>`    |
| `stepper`              | `bind:value` | `RwSignal<f64>`    |
| `pop_up_button` *(Cocoa)* | `bind:value` | `RwSignal<usize>` (selected index) |
| `segmented_control`    | `bind:selection` | `RwSignal<usize>` |
| `date_picker`          | `bind:value` | `RwSignal<Date>`   |
| `color_well` *(Cocoa)*  | `bind:value` | `RwSignal<Color>`  |

## Bidirectional bindings from non-signal sources

`bind:` also accepts a `(getter, setter)` tuple. The getter is a
`Fn() -> T` and the setter is an `FnMut(T)`. This lets you wire a
field to derived state — for example, persisting through a store
on every keystroke:

```rust
<text_field
    bind:value=(
        move || store.username().get(),
        move |v: String| store.patch(User { username: v, ..store.snapshot() }),
    ) />
```
