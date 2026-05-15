# Scrolling

`<scroll_view>` wraps a region of content in a scrollable
viewport. It's available on Cocoa and iOS; the GTK port doesn't
have a scroll-view builder yet.

```rust
view! {
    <vstack flex_grow=1.0>
        <scroll_view has_vertical_scroller=true>
            <vstack padding=12.0 gap=8.0>
                <For each=move || items.get()
                     key=|i| i.id
                     children=move |i| view! { <Row item=i /> } />
            </vstack>
        </scroll_view>
    </vstack>
}
```

## Bounded parent required

A scroll view needs a *bounded* size to know what to clip
against. If you drop one directly into a container that sizes to
fit its content (the default for `vstack` / `hstack`), the
container will grow with the scroll-view's content, and the
scroll view will never need to scroll.

The fix is to give the scroll-view's parent a bounded size:

```rust
<vstack flex_grow=1.0>             // <-- absorbs free vertical space
    <scroll_view>...</scroll_view>
</vstack>

// or
<vstack height=400.0>               // <-- explicit cap
    <scroll_view>...</scroll_view>
</vstack>
```

Without this, the scroll bars never appear.

## Attributes (Cocoa)

| Attribute                    | Type   | Notes                                                             |
|------------------------------|--------|-------------------------------------------------------------------|
| `has_vertical_scroller`      | `bool` | Show the vertical scroller.                                       |
| `has_horizontal_scroller`    | `bool` | Show the horizontal scroller.                                     |
| `autohides_scrollers`        | `bool` | Hide scrollers when content fits.                                 |

Plus all the [shared layout attributes](./attributes.md).

## Attributes (iOS)

| Attribute                    | Type   | Notes                                                             |
|------------------------------|--------|-------------------------------------------------------------------|
| `has_vertical_scroller`      | `bool` | (Mostly nominal — UIScrollView's indicators are always temporary.) |
| `has_horizontal_scroller`    | `bool` | Same.                                                             |

iOS scroll views automatically show indicators during scroll
gestures; the boolean attributes are accepted for API parity but
have less visible effect than on macOS.

## Children

`<scroll_view>` takes a single child. To scroll a list, put a
`<vstack>` (or `<grid>`) inside it:

```rust
<scroll_view>
    <vstack gap=4.0>
        <Row /> <Row /> <Row />
        // ... many rows ...
    </vstack>
</scroll_view>
```

The child sizes naturally; the scroll view clips and provides
scrolling for the overflow.

## A note on layout passes

The scroll view runs a *second* Taffy pass on its content with
horizontal width pinned but height unconstrained — so wrapping
text and dynamic content grow vertically inside it the way you'd
expect. The internal hook is
`cocoa_dom::layout::relayout_scroll_views`; you don't normally
need to think about it.

## When you don't want a scroll view

If you just want clipping (no scrolling), set
`clip=true` on the parent and constrain its height. That uses CSS
`overflow: hidden` semantics via Taffy without bringing in a
native scroll view.
