# `<checkbox>` / `<switch>`

A boolean toggle.

```rust
// Cocoa / GTK
<checkbox bind:checked=subscribed>"Subscribe to newsletter"</checkbox>

// iOS
<switch bind:checked=notifications_on />
```

## Tag naming

- **Cocoa / GTK** use `<checkbox>`.
- **iOS** uses `<switch>` — UISwitch is the canonical iOS
  toggle, and visually it's a slider, not a checkmark. There is
  no `<checkbox>` on iOS.

`<switch>` collides with a Rust keyword; the iOS macro emits
`r#switch` internally. You don't need to worry about this at the
source level — just write `<switch>`.

## Platforms

| Tag         | Cocoa             | GTK             | iOS      |
|-------------|-------------------|-----------------|----------|
| `checkbox`  | NSButton (Switch) | gtk::CheckButton| —        |
| `switch`    | —                 | —               | UISwitch |

## Attributes

| Attribute  | Type     | Cocoa | GTK | iOS | Notes                                                    |
|------------|----------|:-----:|:---:|:---:|----------------------------------------------------------|
| `title`    | `String` | ✓     | ✓   |     | Cocoa/GTK: trailing text label. iOS UISwitch has no label.|
| `checked`  | `bool`   | ✓     | ✓   | ✓   | Current state. Prefer `bind:checked`.                    |
| `enabled`  | `bool`   | ✓     | ✓   | ✓   |                                                          |

Plus all [shared layout
attributes](../layout/attributes.md).

A string child sets the title on Cocoa/GTK:

```rust
<checkbox bind:checked=remember>"Remember me"</checkbox>
```

## Events

| Event      | Cocoa | GTK | iOS | Payload |
|------------|:-----:|:---:|:---:|---------|
| `on:click` | ✓     | ✓   | ✓   | `()`    |

`on:click` fires on every toggle. Combine with `bind:checked` to
do extra work on toggle:

```rust
<checkbox bind:checked=subscribed
          on:click=move |_| log("user toggled subscription")>
    "Subscribe"
</checkbox>
```

## Bindings

| Bind           | Signal type        | Cocoa | GTK | iOS |
|----------------|--------------------|:-----:|:---:|:---:|
| `bind:checked` | `RwSignal<bool>`   | ✓     | ✓   | ✓   |

On iOS the same `bind:checked` key applies to `<switch>`.

## Example

From `cocoa/examples/login_form`:

```rust
let remember = RwSignal::new(false);
view! {
    <checkbox bind:checked=remember>
        "Remember me on this device"
    </checkbox>
}
```
