# Shared Layout Attributes

These attributes are defined once in `common/renderer/src/attrs.rs`
and accepted by **every** element on every port. If a builder
takes a child, parent, or has any visual presence, it has these.

## Spacing

```rust
<vstack padding=16.0 margin=8.0 gap=8.0>
```

| Attribute | Type             | Default | Notes                                                              |
|-----------|------------------|---------|--------------------------------------------------------------------|
| `padding` | `f32` or `Edges` | `0.0`   | Inner spacing. `f32` is uniform on all four sides.                 |
| `margin`  | `f32` or `Edges` | `0.0`   | Outer spacing. Same shape.                                         |
| `gap`     | `f32`            | `0.0`   | Container-only: spacing *between* adjacent children. Not on leaves.|

`Edges` lets you set per-side:

```rust
use leptos::prelude::*;

padding=Edges::axis(16.0, 8.0)         // horiz=16, vert=8
padding=Edges::trbl(8.0, 16.0, 8.0, 16.0) // top, right, bottom, left
```

## Sizing

```rust
<view width=240.0 height=120.0 />
<view max_width=480.0 flex_grow=1.0 />
```

| Attribute    | Type      | Default       | Notes                                          |
|--------------|-----------|---------------|------------------------------------------------|
| `width`      | `Dim`     | `auto`        | `f32` → pixels (points), or `Dim::pct(0.5)`.   |
| `height`     | `Dim`     | `auto`        | Same.                                          |
| `min_width`  | `Dim`     | `auto`        | Lower bound.                                   |
| `min_height` | `Dim`     | `auto`        | Lower bound.                                   |
| `max_width`  | `Dim`     | `auto`        | Upper bound.                                   |
| `max_height` | `Dim`     | `auto`        | Upper bound.                                   |
| `size`       | `f32`     | unset         | Sets `width = height = min = max = value` — rigid square. |
| `hidden`     | `bool`    | `false`       | Removes the element from layout when `true`.   |

`Dim` accepts:

- `f32` → `Dim::Px(value)`
- `Dim::pct(0.5)` → 50% of parent
- `Dim::auto()` → content-sized (the default)

## Flex

```rust
<view flex_grow=1.0 align_self=AlignSelf::Center />
```

| Attribute     | Type        | Default       | Notes                                              |
|---------------|-------------|---------------|----------------------------------------------------|
| `flex_grow`   | `f32`       | `0.0`         | Share of free space on the main axis.              |
| `flex_shrink` | `f32`       | `1.0`         | Share of *negative* space when overconstrained.    |
| `flex_basis`  | `f32`       | `auto`        | Preferred main-axis size before grow/shrink applies. |
| `align_self`  | `AlignSelf` | `Auto`        | Override the parent container's `align` for this child. |

## Grid placement

(See [Grid](./grid.md) for the container side.)

| Attribute            | Type            | Notes                                            |
|----------------------|-----------------|--------------------------------------------------|
| `grid_column_at`     | `i16`           | Place in column `n` (1-based).                   |
| `grid_row_at`        | `i16`           | Place in row `n`.                                |
| `grid_column`        | `(i16, i16)`    | `(start, end)`. `end = -1` means "to the last line". |
| `grid_row`           | `(i16, i16)`    | `(start, end)`.                                  |
| `grid_column_span`   | `u16`           | Span N columns from auto-placed position.        |
| `grid_row_span`      | `u16`           | Same, rows.                                      |
| `grid_column_start`  | `GridLine`      |                                                  |
| `grid_column_end`    | `GridLine`      |                                                  |
| `grid_row_start`     | `GridLine`      |                                                  |
| `grid_row_end`       | `GridLine`      |                                                  |

## Universal

```rust
<button alpha=0.5 tool_tip="Disabled".to_string() ...>
```

| Attribute  | Type     | Default | Notes                                  |
|------------|----------|---------|----------------------------------------|
| `alpha`    | `f32`    | `1.0`   | Opacity, `0.0 ..= 1.0`.                |
| `tool_tip` | `String` | unset   | Hover tooltip. *(Cocoa only, no-op on GTK / iOS.)* |

## Decoration *(Cocoa / iOS)*

```rust
<vstack
    background_color=Color::rgb(0.95, 0.95, 0.95)
    corner_radius=8.0
    border_width=1.0
    border_color=Color::rgb(0.7, 0.7, 0.7)
    overflow=Overflow::Clip
/>
```

| Attribute          | Type     | Default       | Notes                                                |
|--------------------|----------|---------------|------------------------------------------------------|
| `background_color` | `Color`  | transparent   | Background fill.                                     |
| `corner_radius`    | `f32`    | `0.0`         | Rounded-corner radius. Pair with `overflow=Overflow::Clip` (or `Hidden`) to actually clip children to the rounded shape. |
| `border_width`     | `f32`    | `0.0`         | Border thickness.                                    |
| `border_color`     | `Color`  | transparent   | Border colour.                                       |

GTK styling goes through gtk4's CSS theming rather than these
attributes — see [GTK theming](../platform/gtk/settings.md).

## Overflow

CSS-style overflow control. Lives on the layout side (alongside
`width` / `height` / `padding`) because the `Hidden` variant changes
how the element behaves as a flex/grid item.

```rust
<vstack height=200.0 overflow=Overflow::Hidden>
    // children that overflow vertically get clipped at the frame,
    // and this vstack can be shrunk to zero by its flex parent.
</vstack>
```

| Value               | Visual clip | Auto-min-size (as flex/grid item) |
|---------------------|-------------|------------------------------------|
| `Overflow::Visible` | no          | content-based (default)            |
| `Overflow::Clip`    | yes         | content-based                      |
| `Overflow::Hidden`  | yes         | `0` — parent can shrink past content |

- Use `Overflow::Clip` for cosmetic clipping that shouldn't change
  layout shape — most commonly paired with `corner_radius` so the
  children clip to the rounded shape.
- Use `Overflow::Hidden` when the element is expected to be shrunk
  by a flex parent — e.g. a fixed-height list whose container should
  be allowed to collapse on small windows, hiding the rest of the
  content.

For an actual scrolling viewport with scrollbars, use
[`<scroll_view>`](./scroll.md) — overflow doesn't model that today.

Port support:
- **Cocoa / GTK**: both clip variants take visual effect.
- **iOS**: the layout half (Hidden's auto-min-size to 0) takes effect;
  the visual clip is a no-op until UIView's `clipsToBounds` is wired.

## Text *(applies to elements with text)*

These attributes are accepted by elements that render text:
`label`, `button`, `text_field`, `text_view`, etc.

| Attribute    | Type             | Default       | Notes                              |
|--------------|------------------|---------------|-----------------------------------|
| `text_color` | `Color`          | system label  | Foreground text color.            |
| `alignment`  | `NSTextAlignment` / `TextAlignment` | natural | Text alignment.        |
| `font_size`  | `f32`            | system size   | Size in points.                    |

## All attributes are reactive

Every attribute on this page accepts either a value or a closure
returning the same type. The closure is re-run whenever a signal
inside it changes.

```rust
<vstack padding=move || if compact.get() { 8.0 } else { 16.0 }>
```

See [Reactivity and Functions](../reactivity/functions.md).
