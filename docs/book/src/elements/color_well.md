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
| iOS  | UIColorWell (iOS 14+) |

On iOS, tapping the well opens the system color picker as a
modal sheet — different visual UX from Cocoa's inline picker
panel, but the same `bind:value` API.

## Attributes

| Attribute    | Type     | Default | Cocoa | iOS | Notes                                |
|--------------|----------|---------|:-----:|:---:|--------------------------------------|
| `value`      | `Color`  | `WHITE` (Cocoa) / `BLACK` (iOS) | ✓ | ✓ | Current colour. Prefer `bind:value`. |
| `enabled`    | `bool`   | `true`  | ✓     |     |                                      |

Plus all [shared layout
attributes](../layout/attributes.md).

## Events

| Event       | Cocoa | iOS | Payload |
|-------------|:-----:|:---:|---------|
| `on:change` | ✓     | ✓   | `()`    |

Fires when the user selects a colour. Read the new value from
the bound signal.

## Bindings

| Bind         | Signal type       | Cocoa | iOS |
|--------------|-------------------|:-----:|:---:|
| `bind:value` | `RwSignal<Color>` | ✓     | ✓   |

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
            overflow=Overflow::Clip>
            <label text_color=Color::BLACK>"Preview"</label>
        </vstack>
    </vstack>
}
```
