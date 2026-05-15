# `<color_well>`

A colour-picker swatch. Clicking the well opens the system colour
panel.

```rust
let tint = RwSignal::new(Color::rgb(0.2, 0.4, 0.9));
view! { <color_well bind:value=tint /> }
```

## Platforms

| Port | Widget               |
|------|----------------------|
| Cocoa| NSColorWell          |
| GTK  | — (not implemented)  |
| iOS  | — (not implemented)  |

iOS has no inline colour-well equivalent (UIColorPickerViewController
is a modal sheet, not a small control). If you need colour
picking on iOS, build a custom button that opens
UIColorPickerViewController via NodeRef + objc2 bindings.

## Attributes

| Attribute    | Type     | Default | Cocoa | Notes                                |
|--------------|----------|---------|:-----:|--------------------------------------|
| `value`      | `Color`  | `WHITE` | ✓     | Current colour. Prefer `bind:value`. |
| `enabled`    | `bool`   | `true`  | ✓     |                                      |

Plus all [shared layout
attributes](../layout/attributes.md).

## Events

| Event       | Cocoa | Payload |
|-------------|:-----:|---------|
| `on:change` | ✓     | `()`    |

Fires when the user selects a colour. Read the new value from
the bound signal.

## Bindings

| Bind         | Signal type       | Cocoa |
|--------------|-------------------|:-----:|
| `bind:value` | `RwSignal<Color>` | ✓     |

## Example

```rust
let bg = RwSignal::new(Color::rgb(1.0, 1.0, 1.0));

view! {
    <vstack padding=16.0 gap=8.0>
        <color_well bind:value=bg />
        <vstack
            height=120.0
            background_color=move || bg.get()
            corner_radius=8.0
            clip=true>
            <label text_color=Color::BLACK>"Preview"</label>
        </vstack>
    </vstack>
}
```
