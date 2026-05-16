# `<text_field>` / `<secure_text_field>`

A single-line text input. `<secure_text_field>` is the
password-masked variant.

```rust
let name = RwSignal::new(String::new());
view! { <text_field bind:value=name placeholder="Your name" /> }
```

## Platforms

| Tag                  | Cocoa             | GTK                | iOS                       |
|----------------------|-------------------|--------------------|---------------------------|
| `text_field`         | NSTextField       | gtk::Entry         | UITextField               |
| `secure_text_field`  | NSSecureTextField | gtk::PasswordEntry | UITextField (`isSecureTextEntry`) |

## Attributes

| Attribute     | Type           | Default       | Cocoa | GTK | iOS | Notes                                                |
|---------------|----------------|---------------|:-----:|:---:|:---:|------------------------------------------------------|
| `value`       | `String`       | `""`          | ✓     | ✓   | ✓   | Current text. Prefer `bind:value` for two-way state. |
| `placeholder` | `String`       | `""` (none)   | ✓     | ✓   | ✓   | Grey hint text when empty.                           |
| `enabled`     | `bool`         | `true`        | ✓     | ✓   | ✓   |                                                      |
| `bordered`    | `bool`         | `true`        | ✓     |     |     | Cocoa: NSTextField bezel.                            |
| `bezeled`     | `bool`         | `true`        | ✓     |     |     | Cocoa: alt bezel style.                              |
| `text_color`  | `Color`        | system label  | ✓     |     | ✓   |                                                      |
| `font_size`   | `f32`          | system size   | ✓     |     | ✓   |                                                      |
| `alignment`   | text alignment | natural       | ✓     |     | ✓   |                                                      |
| `intrinsic_width` | `IntrinsicWidth` | `FromParent` | ✓ |     |     | Override the default width=0 measure. Set to `FromContent` to let the field grow with its text. |

Plus all [shared layout attributes](../layout/attributes.md).

## Events

| Event          | Cocoa | GTK | iOS | Payload                |
|----------------|:-----:|:---:|:---:|------------------------|
| `on:input`     | ✓     | ✓   | ✓   | `String` (new contents)|
| `on:change`    | ✓     | ✓   | ✓   | `()` (read bound signal for value) |
| `on:commit`    | ✓     | ✓   | ✓   | `String` (committed)   |
| `on:focus`     | ✓     | ✓   | ✓   | `()`                   |
| `on:blur`      | ✓     | ✓   | ✓   | `()`                   |
| `on:keydown`   | ✓     |     | ✓   | `KeyEvent`             |
| `on:keyup`     | ✓     |     | ✓   | `KeyEvent`             |

- `on:input` fires on **every** keystroke with the new text.
- `on:change` fires on **every** value change (effectively the
  same as `on:input` here, with no payload) — kept for
  consistency with other value-bearing controls.
- `on:commit` fires when the user **commits** the edit — Return
  key, focus loss.
- `on:keydown` / `on:keyup` on Cocoa only fire for recognised
  command keys (Enter, Escape, Tab, arrows), not for ordinary
  text input. Use `on:input` for keystroke-level callbacks.

All events coexist on the same field, including alongside
`bind:value`.

## Bindings

| Bind         | Signal type        | Cocoa | GTK | iOS |
|--------------|--------------------|:-----:|:---:|:---:|
| `bind:value` | `RwSignal<String>` | ✓     | ✓   | ✓   |

`bind:value` does both directions automatically:

```rust
let email = RwSignal::new(String::new());
view! { <text_field bind:value=email /> }
```

If you need to filter or transform one direction, pass a
`(getter, setter)` tuple:

```rust
<text_field bind:value=(
    move || email.get(),
    move |v: String| email.set(v.trim().to_lowercase()),
) />
```

## A note about width

`<text_field>` reports its **natural** intrinsic width based on
its current content, which would otherwise cause the field to
grow with every keystroke. The Cocoa port overrides this in the
measure callback to force width=0 by default — the parent
decides the field's width. Set `width=...` or `flex_grow=1.0`
explicitly to size it.

If you actually *want* the content-tracking behaviour (a
read-only field used as a label, an editable field that grows
with its text), opt in via `intrinsic_width`:

```rust
<text_field
    value=move || username.get()
    intrinsic_width=IntrinsicWidth::FromContent />
```

## Combined example

The `checkbox` example uses every event at once:

```rust
let email = RwSignal::new(String::new());
let keystroke_count = RwSignal::new(0_u32);
let last_committed  = RwSignal::new(String::new());

view! {
    <text_field
        bind:value=email
        on:input=move |_v: String| keystroke_count.update(|c| *c += 1)
        on:commit=move |v: String| last_committed.set(v) />

    <label>{move || format!("Keystrokes: {}", keystroke_count.get())}</label>
    <label>{move || format!("Last committed: {:?}", last_committed.get())}</label>
}
```
