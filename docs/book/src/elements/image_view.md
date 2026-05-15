# `<image_view>`

Render an image — either from a file path, a bundled asset, or
an SF Symbol name.

```rust
view! {
    <image_view source="/path/to/photo.png".to_string() width=200.0 height=200.0 />
    <image_view sf_symbol="star.fill".to_string() tint=Color::YELLOW />
}
```

## Platforms

| Port | Widget               |
|------|----------------------|
| Cocoa| NSImageView          |
| GTK  | — (not implemented)  |
| iOS  | UIImageView          |

## Attributes

| Attribute   | Type     | Default      | Cocoa | iOS | Notes                                                    |
|-------------|----------|--------------|:-----:|:---:|----------------------------------------------------------|
| `source`    | `String` | `""` (none)  | ✓     | ✓   | File path to load (PNG/JPEG/etc.).                       |
| `sf_symbol` | `String` | `""` (none)  | ✓     | ✓   | SF Symbol name. See [SF Symbols](../platform/cocoa/sf_symbols.md). |
| `tint`      | `Color`  | system tint  | ✓     | ✓   | Tint the image (most useful with SF Symbols).            |

Plus all [shared layout
attributes](../layout/attributes.md).

## Events

None.

## Bindings

None.

## Sizing

`<image_view>` reports the underlying image's natural size as its
intrinsic size. Override with `width=` / `height=` if you need a
specific size, or `flex_grow=1.0` to fill a container.

## SF Symbols

The fastest path to iconography on Apple platforms is
`sf_symbol=`:

```rust
view! {
    <hstack gap=12.0>
        <image_view sf_symbol="house.fill".to_string()    width=24.0 height=24.0 />
        <image_view sf_symbol="gearshape".to_string()      width=24.0 height=24.0 />
        <image_view sf_symbol="person.crop.circle".to_string() width=24.0 height=24.0 />
    </hstack>
}
```

See the [SF Symbols](../platform/cocoa/sf_symbols.md) page for the
icon catalogue and tinting.
