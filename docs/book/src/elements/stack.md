# `<stack>` / `<vstack>` / `<hstack>` / `<view>`

Flex containers. Covered in depth in
[Layout / Flexbox](../layout/flexbox.md).

```rust
<vstack padding=16.0 gap=8.0>
    <label>"Title"</label>
    <hstack gap=4.0>
        <button>"A"</button>
        <button>"B"</button>
    </hstack>
</vstack>
```

## Platforms

| Tag       | Cocoa            | GTK           | iOS              |
|-----------|------------------|---------------|------------------|
| `stack`   | NSView (Taffy)   | gtk::Box      | UIView           |
| `vstack`  | NSView, direction=Column | gtk::Box, direction=Column | UIView, flex_direction=Column |
| `hstack`  | NSView, direction=Row    | gtk::Box, direction=Row    | UIView, flex_direction=Row    |

## Distinguishing the variants

- `vstack` / `hstack` — preset direction. Use these whenever you
  know the axis. Reads better than `<stack direction=...>`.
- `stack` — no direction preset. You'll set `direction=`
  yourself. Available on all three ports.

## Attributes

| Attribute        | Type             | Default          | Cocoa | GTK | iOS | Notes                                  |
|------------------|------------------|------------------|:-----:|:---:|:---:|----------------------------------------|
| `direction`      | `FlexDirection`  | `Column` for vstack, `Row` for hstack, `Row` for stack | ✓ | ✓ | ✓ | `Row`, `Column`, etc.                  |
| `gap`            | `f32`            | `0.0`            | ✓     | ✓   | ✓   | Spacing between children.              |
| `justify_content`| `JustifyContent` | `FlexStart`      | ✓     | ✓   | ✓   | Main-axis alignment.                   |
| `align`          | `AlignItems`     | `Stretch`        | ✓     | ✓   | ✓   | Cross-axis alignment.                  |
| `align_content`  | `AlignContent`   | `Stretch`        | ✓     | ✓   | ✓   | Multi-line cross-axis alignment.       |
| `justify_items`  | `JustifyItems`   | `Stretch`        | ✓     | ✓   | ✓   | Per-child main-axis override.          |
| `wrap`           | `FlexWrap`       | `NoWrap`         | ✓     | ✓   | ✓   | `Wrap`, `NoWrap`, `WrapReverse`.       |

Plus all [shared layout
attributes](../layout/attributes.md) (including the
[decoration attributes](../layout/attributes.md#decoration-cocoa--ios)
`background_color`, `corner_radius`, `border_width`,
`border_color` on Cocoa and iOS, and [`overflow`](../layout/attributes.md#overflow)
on every port).

## Children

Containers accept arbitrary children inside their tags:

```rust
<vstack>
    <label>"A"</label>
    <button>"B"</button>
    <For each=... key=... children=... />
    {move || if condition.get() { view!{ <X/> } } else { view!{ <Y/> } }}
</vstack>
```

There's no fixed children type — anything that implements
`Render` works.

## Events

| Event      | Cocoa | GTK | iOS | Payload |
|------------|:-----:|:---:|:---:|---------|
| `on:click` |       |     | ✓*  | `()`    |

\* iOS attaches a `UITapGestureRecognizer` so any container can
become clickable. Cocoa/GTK don't auto-attach gesture recognisers
to plain views.

## Bindings

None.

## Decoration

Containers double as the "styled box" primitive — there's no
separate `<box>` builder. Use the decoration attributes:

```rust
<vstack
    background_color=Color::rgb(0.95, 0.95, 0.95)
    corner_radius=8.0
    border_width=1.0
    border_color=Color::rgb(0.85, 0.85, 0.85)
    padding=12.0>
    <label>"Card"</label>
</vstack>
```
