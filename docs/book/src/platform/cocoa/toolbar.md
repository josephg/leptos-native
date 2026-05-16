# Toolbar

NSToolbar attaches to the title bar of a window. The
`<toolbar>` builder declares the toolbar inline in your view
tree; it walks up the view chain at mount time to find the
containing NSWindow.

```rust
use leptos::prelude::*;

run(|| view! {
    <window title="Notes" size=WindowSize(800.0, 600.0)>
        <toolbar>
            <toolbar_toggle_sidebar />
            <toolbar_sidebar_tracking_separator />
            <toolbar_item
                identifier="new"
                label="New"
                sf_symbol="square.and.pencil"
                on:action=move |_| new_note() />
            <toolbar_flexible_space />
            <toolbar_search_item
                identifier="search"
                placeholder="Search notes..."
                on:change=move |q: String| search(&q) />
        </toolbar>
        <split_view>
            <split_pane>...</split_pane>
            <split_pane>...</split_pane>
        </split_view>
    </window>
});
```

## `<toolbar>`

The container. Walks up to find the NSWindow.

| Attribute      | Type                  | Notes                                                |
|----------------|-----------------------|------------------------------------------------------|
| `identifier`   | `String`              | NSToolbar identifier for state persistence.          |
| `display_mode` | `ToolbarDisplayMode`  | Icon-only / label-only / both.                       |
| `visible`      | `bool`                | Hide/show the toolbar.                               |
| `handle`       | `ToolbarHandle`       | Programmatic control (currently limited).            |

Children are the toolbar items.

### Reactive item sets

The macro-emitted child list inside `<toolbar>` is fixed at
build time — `<For>` directly inside `<toolbar>` isn't supported
yet. For dynamic item sets, drive the toolbar via a
[`ToolbarHandle`](#toolbarhandle) and an `Effect`:

```rust
use leptos::prelude::*;

let toolbar = ToolbarHandle::new();
let docs = RwSignal::new(Vec::<Doc>::new());

Effect::new({
    let toolbar = toolbar.clone();
    move |_| {
        let desired: Vec<(String, _)> = docs.get()
            .into_iter()
            .map(|d| (
                d.id.clone(),
                toolbar_item()
                    .label(d.title.clone())
                    .on(event::action, move |_| open(&d.id)),
            ))
            .collect();
        toolbar.set_items(desired);
    }
});

view! {
    <window title="Notes" ...>
        <toolbar handle=toolbar.clone()>
            <toolbar_toggle_sidebar />
            // (set_items inserts/removes here on each docs change)
            <toolbar_flexible_space />
            <toolbar_search_item identifier="search" />
        </toolbar>
        ...
    </window>
}
```

`ToolbarHandle::set_items(Vec<(String, ToolbarItem)>)` diffs by
identifier:

- **Additive changes** (items added/removed without reordering
  retained ones): minimal insert/remove pass; retained items
  stay in place.
- **Reordering** (any retained item's relative position
  changes): removes every item and reinserts from scratch in
  the desired order. Simpler than a full LCS-based diff; the
  toolbar briefly thrashes but lands at the correct state.

The tuple's `String` key is the source-of-truth identifier; any
`.identifier(…)` set on the builder is overridden.

For finer-grained control, the lower-level `insert_item` /
`remove_item` / `current_identifiers` / `contains_item` methods
on `ToolbarHandle` give you the diff primitives directly.

## `<toolbar_item>`

A generic action item.

| Attribute        | Type     | Notes                                                |
|------------------|----------|------------------------------------------------------|
| `identifier`     | `String` | Required. NSToolbarItem identifier.                  |
| `label`          | `String` | Title shown under the item when labels visible.      |
| `palette_label`  | `String` | Label in the toolbar customisation palette.          |
| `tool_tip`       | `String` | Hover tooltip.                                       |
| `icon`           | `Icon`   | SF Symbol or NSImage.                                |
| `sf_symbol`      | `String` | Shorthand for `icon=Icon::SfSymbol(...)`.            |
| `enabled`        | `bool`   |                                                      |
| `bordered`       | `bool`   |                                                      |
| `navigational`   | `bool`   | macOS 11+ navigation-style item (back/forward).      |
| `view`           | view     | Embed an arbitrary view as the item's content.       |

Event:

| Event        | Payload | Notes                          |
|--------------|---------|--------------------------------|
| `on:action`  | `()`    | Fires when the user clicks.    |

## `<toolbar_search_item>`

A native NSSearchToolbarItem with embedded NSSearchField.

| Attribute        | Type     | Notes                                                |
|------------------|----------|------------------------------------------------------|
| `identifier`     | `String` | Required.                                            |
| `label`          | `String` |                                                      |
| `palette_label`  | `String` |                                                      |
| `tool_tip`       | `String` |                                                      |
| `placeholder`    | `String` | Search field hint text.                              |
| `enabled`        | `bool`   |                                                      |
| `value`          | `String` | Search field contents (use `bind:value` for two-way).|
| `preferred_width`| `f64`    | Width when the search field is collapsed.            |
| `width`          | `f64`    | Width when expanded.                                 |

Events:

| Event        | Payload  | Notes                            |
|--------------|----------|----------------------------------|
| `on:input`   | `String` | Every keystroke.                 |
| `on:change`  | `String` | On commit (Return / focus loss). |

## Spacers and built-ins

| Builder                                  | What it does                                          |
|------------------------------------------|-------------------------------------------------------|
| `<toolbar_space/>`                       | Standard fixed gap.                                   |
| `<toolbar_flexible_space/>`              | Expands to fill the remaining width.                  |
| `<toolbar_toggle_sidebar/>`              | System-provided sidebar-toggle item (pairs with `<split_view>`). |
| `<toolbar_sidebar_tracking_separator/>`  | Vertical divider that aligns with the sidebar split.  |
| `<toolbar_print/>`                       | System print item that triggers `printDocument:` up the responder chain. |

These don't take children or most attributes — they're
fixed-function system items.

## A typical "sidebar + main" toolbar

```rust
<toolbar>
    <toolbar_toggle_sidebar />
    <toolbar_sidebar_tracking_separator />
    <toolbar_item identifier="add" label="Add" sf_symbol="plus" on:action=add />
    <toolbar_flexible_space />
    <toolbar_search_item identifier="search" placeholder="Search..."
        on:change=move |q: String| filter.set(q) />
</toolbar>
```

The `toggle_sidebar` button collapses/expands the first
`<split_pane>` of the enclosing `<split_view>`; the
`sidebar_tracking_separator` keeps the visual divider aligned
with the split.

## See also

- `cocoa/examples/toolbar_demo/src/main.rs` — toolbar variations.
- `cocoa/examples/pages/src/main.rs` — toolbar + split view +
  window working together.
