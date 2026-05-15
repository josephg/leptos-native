# `<text_view>`

A multi-line text editor.

```rust
let body = RwSignal::new(String::new());
view! { <text_view bind:value=body /> }
```

## Platforms

| Port | Widget       |
|------|--------------|
| Cocoa| NSTextView   |
| GTK  | — (not implemented) |
| iOS  | UITextView   |

## Attributes

| Attribute     | Type           | Default       | Cocoa | iOS | Notes                                            |
|---------------|----------------|---------------|:-----:|:---:|--------------------------------------------------|
| `value`       | `String`       | `""`          | ✓     | ✓   | Current text. Prefer `bind:value`.               |
| `enabled`     | `bool`         | `true`        | ✓     | ✓   | iOS: maps to `editable`, not `enabled`.          |
| `text_color`  | `Color`        | system label  | ✓     | ✓   |                                                  |
| `font_size`   | `f32`          | system size   | ✓     | ✓   |                                                  |
| `alignment`   | text alignment | natural       | ✓     | ✓   |                                                  |

Plus all [shared layout attributes](../layout/attributes.md).

## Events

UITextView and NSTextView are not UIControl/NSControl, so the
fan-out delegate pattern used elsewhere doesn't apply. Currently
no `on:` events are wired through for `text_view` — use
`bind:value` to react to changes, and reactively read the signal
inside an `Effect` for "did something change" behavior.

## Bindings

| Bind         | Signal type        | Cocoa | iOS |
|--------------|--------------------|:-----:|:---:|
| `bind:value` | `RwSignal<String>` | ✓     | ✓   |

Wired via a `UITextViewDelegate` / `NSTextViewDelegate` that
pushes edits into the signal as the user types.

## Sizing

`<text_view>` doesn't have a meaningful intrinsic size — give it
explicit dimensions or grow flags:

```rust
<text_view bind:value=body flex_grow=1.0 min_height=120.0 />
```

Wrap it in a `<scroll_view>` if you want long content to scroll
rather than expand the parent.
