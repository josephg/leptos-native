# Flexbox

Layout in this fork is driven by [Taffy](https://github.com/DioxusLabs/taffy),
the same engine many of the major Rust UI frameworks use. The
mental model is CSS Flexbox / Grid, ported to native widgets.

The flex containers are:

- **`<vstack>`** — vertical stack. Direction preset to Column.
- **`<hstack>`** — horizontal stack. Direction preset to Row.
- **`<stack>`** — generic flex container with no direction
  preset. Set `direction=` explicitly.

```rust
view! {
    <vstack padding=16.0 gap=12.0>
        <label>"Header"</label>
        <hstack gap=8.0>
            <button>"Cancel"</button>
            <button>"OK"</button>
        </hstack>
    </vstack>
}
```

## Container attributes

All four containers accept the same set of flex-layout
attributes:

| Attribute             | Type                  | Notes                                  |
|-----------------------|-----------------------|----------------------------------------|
| `direction`           | `FlexDirection`       | `Row`, `Column`, `RowReverse`, `ColumnReverse`. |
| `gap`                 | `f32`                 | Spacing between children.              |
| `justify_content`     | `JustifyContent`      | Main-axis alignment.                   |
| `align`               | `AlignItems`          | Cross-axis alignment of children.      |
| `align_content`       | `AlignContent`        | Multi-line cross-axis alignment.       |
| `justify_items`       | `JustifyItems`        | Per-child main-axis alignment override.|
| `wrap`                | `FlexWrap`            | `Wrap`, `NoWrap`, `WrapReverse`.       |

The enum names match Taffy's exports; you can `use leptos::prelude::*`
and write `AlignItems::Center` directly.

## Child attributes

Children inside a flex container accept the layout attributes
described in [Shared Layout Attributes](./attributes.md). The
flex-specific ones are:

| Attribute      | Type        | Notes                                                |
|----------------|-------------|------------------------------------------------------|
| `flex_grow`    | `f32`       | Share of free space along the main axis. `1.0` is the common "fill" value. |
| `flex_shrink`  | `f32`       | Share of *negative* space when overconstrained.      |
| `flex_basis`   | `f32`       | Preferred main-axis size before grow/shrink applies. |
| `align_self`   | `AlignSelf` | Override the container's `align` for this child.     |

## A worked example: title + body + buttons

```rust
view! {
    <vstack padding=16.0 gap=8.0>
        <label font_size=20.0>"Settings"</label>
        <vstack flex_grow=1.0 gap=8.0>
            // body — grows to fill remaining vertical space
            <Section title="General" />
            <Section title="Advanced" />
        </vstack>
        <hstack gap=8.0 justify_content=JustifyContent::FlexEnd>
            <button>"Cancel"</button>
            <button>"Apply"</button>
        </hstack>
    </vstack>
}
```

`flex_grow=1.0` on the middle `vstack` makes it absorb the
vertical space the title and button row don't claim — the
buttons stay pinned at the bottom.

## When to use `padding` vs `gap`

- `padding` — space *inside* a container, between its border and
  its children. Set on the container.
- `gap` — space *between* adjacent children. Also set on the
  container.
- `margin` — space *outside* a child, between it and its
  siblings. Set on the child.

`gap` is usually what you want for "even spacing between siblings"
— it doesn't add space at the start or end.

## Width and height

By default, a flex container sizes to its content. Pass
`width=...` / `height=...` (or `min_*` / `max_*`) to fix that.
Sizes can be:

- `f32` — pixels (points on macOS/iOS).
- `Dim::pct(0.5)` — a percentage of the parent.
- `Dim::Auto` — content-sized (the default).

See [Shared Layout Attributes](./attributes.md) for the full
sizing reference.

## A note about scroll views

A `<scroll_view>` (macOS / iOS only) needs a *bounded* parent. If
you put a `vstack` directly inside the window content root with a
scroll view inside it, the outer `vstack` will size to fit the
scroll content and the scroll view will never actually scroll.

The fix: give the outer `vstack` `flex_grow=1.0` or a fixed
`height=...`.

```rust
mount_to_window(..., || view! {
    <vstack flex_grow=1.0>  // <-- bound the height
        <scroll_view>
            // long content
        </scroll_view>
    </vstack>
})
.run();
```
