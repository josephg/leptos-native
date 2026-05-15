# `<toggle>` / `<checkbox>` / `<switch>`

A boolean toggle.

```rust
// Portable — same on all three ports.
<toggle bind:checked=subscribed>"Subscribe to newsletter"</toggle>

// Native-named aliases:
<checkbox bind:checked=remember>"Remember me"</checkbox>  // Cocoa / GTK
<switch bind:checked=notifications_on />                  // iOS
```

## Tag naming

- **`<toggle>`** — portable; available on all three ports. Maps
  to whatever the native toggle widget is.
- **`<checkbox>`** — Cocoa / GTK alias. Same widget as `<toggle>`
  on those ports.
- **`<switch>`** — iOS alias. UISwitch is the canonical iOS
  toggle (visually a slider, not a checkmark). Same widget as
  `<toggle>` on iOS.

`<switch>` collides with a Rust keyword; the iOS macro emits
`r#switch` internally. You don't need to worry about this at the
source level — just write `<switch>`.

For portable code that runs on all three ports, prefer
`<toggle>`. For per-port code where the native name is
expected, use `<checkbox>` / `<switch>`.

## Platforms

| Tag         | Cocoa             | GTK             | iOS      |
|-------------|-------------------|-----------------|----------|
| `toggle`    | NSButton (Switch) | gtk::CheckButton| UISwitch |
| `checkbox`  | NSButton (Switch) | gtk::CheckButton| —        |
| `switch`    | —                 | —               | UISwitch |

## Attributes

| Attribute  | Type     | Default  | Cocoa | GTK | iOS | Notes                                                    |
|------------|----------|----------|:-----:|:---:|:---:|----------------------------------------------------------|
| `title`    | `String` | `""`     | ✓     | ✓   |     | Cocoa/GTK: trailing text label. iOS UISwitch has no label.|
| `checked`  | `bool`   | `false`  | ✓     | ✓   | ✓   | Current state. Prefer `bind:checked`.                    |
| `enabled`  | `bool`   | `true`   | ✓     | ✓   | ✓   |                                                          |

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
