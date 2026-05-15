# `<grid>`

A two-dimensional Grid container. Covered in depth in
[Layout / Grid](../layout/grid.md).

```rust
view! {
    <grid
        columns=vec![length(220.0), fr(1.0)]
        rows=vec![fr(1.0)]
        gap=12.0
        padding=12.0>
        <Sidebar />
        <Main />
    </grid>
}
```

## Platforms

| Port | Backing                                |
|------|----------------------------------------|
| Cocoa| NSView, layout via Taffy grid algorithm |
| GTK  | gtk::Box, layout via Taffy grid algorithm |
| iOS  | UIView, layout via Taffy grid algorithm |

All three ports run the same Taffy grid engine. The visual result
is identical across platforms.

## Container attributes

| Attribute        | Type                              | Notes                                              |
|------------------|-----------------------------------|----------------------------------------------------|
| `columns`        | `Vec<GridTemplateComponent>`      | Explicit column tracks (`fr`, `length`, `auto`, `minmax`, etc.). |
| `rows`           | `Vec<GridTemplateComponent>`      | Explicit row tracks.                               |
| `auto_columns`   | `Vec<TrackSizingFunction>`        | Sizing for implicit (auto-created) columns.        |
| `auto_rows`      | `Vec<TrackSizingFunction>`        | Sizing for implicit rows.                          |
| `auto_flow`      | `GridAutoFlow`                    | `Row` (default), `Column`, `RowDense`, `ColumnDense`. |
| `gap`            | `f32`                             | Spacing between cells in both axes.                |
| `column_gap`     | `f32`                             | Overrides `gap` for columns.                       |
| `row_gap`        | `f32`                             | Overrides `gap` for rows.                          |
| `justify_items`  | `JustifyItems`                    | Per-cell main-axis alignment default.              |
| `align`          | `AlignItems`                      | Per-cell cross-axis alignment default.             |
| `justify_content`| `JustifyContent`                  | Alignment of the entire track grid.                |
| `align_content`  | `AlignContent`                    | Cross-axis variant.                                |

Plus all [shared layout attributes](../layout/attributes.md).

## Child placement attributes

These live on the **children**, not the grid. Documented in
[Shared Layout Attributes /
Grid placement](../layout/attributes.md#grid-placement):

`grid_column_at`, `grid_row_at`, `grid_column=(start, end)`,
`grid_row=(start, end)`, `grid_column_span`, `grid_row_span`, and
the explicit `grid_column_start` / `grid_column_end` /
`grid_row_start` / `grid_row_end`.

## Children

Anything `Render` — same as the flex containers.

## Track-sizing helpers

Exported by `leptos::prelude::*`:

- `fr(n)` — fractional units of remaining space.
- `length(px)` — fixed pixel size.
- `auto()` — content-sized.
- `min_content()`, `max_content()` — intrinsic sizes.
- `minmax(min, max)` — bounded range.
- `fit_content(px)` — max-content clamped to `px`.
- `repeat(n, tracks)` — repeat a track pattern *n* times.

## See also

- [Layout / Grid](../layout/grid.md) for the full conceptual
  walkthrough.
- [Shared Layout Attributes](../layout/attributes.md) for the
  child placement attributes.
