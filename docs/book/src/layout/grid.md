# Grid

`<grid>` is a two-dimensional layout container backed by Taffy's
grid algorithm. It's modelled on the same primitives as web Grid —
explicit tracks, fractional units, named lines, `auto-flow` — so
the mental model carries over cleanly.

```rust
use leptos_native::prelude::*;

view! {
    <grid
        columns=vec![fr(1.0), fr(3.0), fr(1.0)]
        rows=vec![length(56.0), fr(1.0), length(40.0)]
        gap=12.0
        padding=12.0
    >
        <vstack grid_row=(1, 2) grid_column=(1, -1)>
            <label>"Header — spans all columns"</label>
        </vstack>
        <vstack grid_row=(2, 4) grid_column_at=1>"Sidebar"</vstack>
        <vstack>"Main content (row 2, col 2)"</vstack>
        <vstack>"Right rail (row 2, col 3)"</vstack>
        <vstack grid_row_at=3 grid_column=(2, -1)>
            <label>"Footer — cols 2–3"</label>
        </vstack>
    </grid>
}
```

This is `cocoa/examples/grid`, abbreviated. Resize the window and
the `fr(...)` columns reflow proportionally.

## Tracks

`columns=` and `rows=` take a `Vec<GridTemplateComponent>`. The
helpers from the prelude:

| Helper            | What it means                                  |
|-------------------|------------------------------------------------|
| `fr(n)`           | `n` fractional units of remaining space.       |
| `length(px)`      | Fixed pixel size.                              |
| `auto()`          | Content-sized.                                 |
| `min_content()`   | Smallest size that fits the content.           |
| `max_content()`   | Largest size that fits the content unwrapped.  |
| `minmax(min, max)`| Bounded range.                                 |
| `fit_content(px)` | `max_content` clamped to `px`.                 |
| `repeat(n, ...)`  | Repeat a track pattern *n* times.              |

A typical sidebar layout:

```rust
columns=vec![length(220.0), fr(1.0)]
```

A photo grid:

```rust
columns=vec![repeat(3, vec![fr(1.0)])]
```

## Placing children

By default, items flow into grid cells in source order. To pin a
child to a specific cell, use the placement attributes:

| Attribute            | Type                    | What it does                                       |
|----------------------|-------------------------|----------------------------------------------------|
| `grid_column_at`     | `i16`                   | Place in this column (1-based).                    |
| `grid_row_at`        | `i16`                   | Place in this row.                                 |
| `grid_column`        | `(start, end)`          | Shorthand for start+end. `end=-1` means "to the last line". |
| `grid_row`           | `(start, end)`          | Same, for rows.                                    |
| `grid_column_span`   | `u16`                   | Span N columns from auto position.                 |
| `grid_row_span`      | `u16`                   | Span N rows from auto position.                    |
| `grid_column_start`  | `GridLine`              | Just the start line.                               |
| `grid_column_end`    | `GridLine`              | Just the end line. Negative = from the end.        |
| `grid_row_start`     | `GridLine`              | Just the start.                                    |
| `grid_row_end`       | `GridLine`              | Just the end.                                      |

`grid_column=(1, -1)` is the "span all columns" idiom — same as
CSS `grid-column: 1 / -1`.

## Container attributes

| Attribute        | Type                              | Notes                                              |
|------------------|-----------------------------------|----------------------------------------------------|
| `columns`        | `Vec<GridTemplateComponent>`      | Explicit column tracks.                            |
| `rows`           | `Vec<GridTemplateComponent>`      | Explicit row tracks.                               |
| `auto_columns`   | `Vec<TrackSizingFunction>`        | Sizing for implicit (auto-created) columns.        |
| `auto_rows`      | `Vec<TrackSizingFunction>`        | Sizing for implicit rows.                          |
| `auto_flow`      | `GridAutoFlow`                    | `Row` (default), `Column`, `RowDense`, `ColumnDense`. |
| `gap`            | `f32`                             | Spacing between cells in both axes.                |
| `column_gap`     | `f32`                             | Override `gap` for columns.                        |
| `row_gap`        | `f32`                             | Override `gap` for rows.                           |
| `justify_items`  | `JustifyItems`                    | Default main-axis alignment of items in their cells. |
| `align_items`    | `AlignItems`                      | Default cross-axis alignment.                      |
| `justify_content`| `JustifyContent`                  | Alignment of the *track grid* inside the container. |
| `align_content`  | `AlignContent`                    | Cross-axis variant.                                |

## Auto-flow

When you don't pin children explicitly, they flow into cells in
source order:

```rust
<grid columns=vec![fr(1.0), fr(1.0), fr(1.0)] gap=8.0>
    <Card />  // row 1, col 1
    <Card />  // row 1, col 2
    <Card />  // row 1, col 3
    <Card />  // row 2, col 1
    // ...
</grid>
```

Set `auto_flow=GridAutoFlow::Column` to flow down columns first
instead.

## Implicit tracks

If you place a child outside the explicit `columns` / `rows` (e.g.
`grid_row_at=7` when you only declared three rows), Taffy creates
implicit rows. They size according to `auto_rows=` (default
`auto()`).

## A note on platform support

Grid works the same on Cocoa, GTK, and iOS — all three drive
Taffy through the shared `common/renderer` layout core.
