# `<scroll_view>`

A scrollable container. Covered in more depth in
[Layout / Scrolling](../layout/scroll.md).

```rust
<vstack flex_grow=1.0>
    <scroll_view has_vertical_scroller=true>
        <vstack padding=12.0 gap=8.0>
            // long content
        </vstack>
    </scroll_view>
</vstack>
```

## Platforms

| Port | Widget               |
|------|----------------------|
| Cocoa| NSScrollView         |
| GTK  | — (not implemented)  |
| iOS  | UIScrollView         |

## Attributes

| Attribute                 | Type   | Default | Cocoa | iOS | Notes                                  |
|---------------------------|--------|---------|:-----:|:---:|----------------------------------------|
| `has_vertical_scroller`   | `bool` | `true`  | ✓     | ✓   |                                        |
| `has_horizontal_scroller` | `bool` | `false` | ✓     | ✓   |                                        |
| `autohides_scrollers`     | `bool` | `true`  | ✓     |     | Cocoa: hide scrollers when content fits. |

Plus all [shared layout
attributes](../layout/attributes.md).

## Children

A single child. To scroll a list, put a `<vstack>` (or any
container) inside:

```rust
<scroll_view>
    <vstack gap=4.0>
        <Row /> <Row /> <Row />
    </vstack>
</scroll_view>
```

## The bounded-parent requirement

A scroll view needs a bounded parent — otherwise the parent
sizes to fit the scroll's content and the scroll view never
actually scrolls. See [Layout / Scrolling](../layout/scroll.md)
for the workaround.

## Events / bindings

None.
