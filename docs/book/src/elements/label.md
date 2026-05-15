# `<label>`

A read-only text display.

```rust
<label>"Hello, world"</label>
<label>{move || format!("Count: {}", count.get())}</label>
```

## Platforms

| Port | Widget                  |
|------|-------------------------|
| Cocoa| NSTextField (read-only) |
| GTK  | gtk::Label              |
| iOS  | UILabel                 |

## Attributes

| Attribute    | Type            | Cocoa | GTK | iOS | Notes                                                       |
|--------------|-----------------|:-----:|:---:|:---:|-------------------------------------------------------------|
| `text`       | `String`        | ✓     | ✓   | ✓   | The displayed text. A string child (`>"..."<`) sets it too. |
| `text_color` | `Color`         | ✓     |     | ✓   |                                                             |
| `alignment`  | text alignment  | ✓     |     | ✓   |                                                             |
| `font_size`  | `f32`           | ✓     |     | ✓   |                                                             |
| `bold`       | `bool`          | ✓     |     |     |                                                             |
| `line_break` | line-break mode | ✓     |     |     | How long text wraps/truncates.                              |
| `multiline`  | `bool`          | ✓     |     |     | Allow wrapping to multiple lines.                           |
| `selectable` | `bool`          | ✓     |     |     | Allow user to select & copy the text.                       |

Plus all [shared layout
attributes](../layout/attributes.md).

## Events

| Event      | Cocoa | GTK | iOS | Payload |
|------------|:-----:|:---:|:---:|---------|
| `on:click` | ✓     |     | ✓   | `()`    |

## Bindings

| Bind        | Type             | Cocoa | GTK | iOS | Notes                          |
|-------------|------------------|:-----:|:---:|:---:|--------------------------------|
| `bind:value`| `String`         |       | ✓   |     | One-way sink: signal → label. GTK only. |

## Child = text only

`<label>` accepts a string-typed child only. To render a value
that might be a `Result<T, E>` (so it can flow through an
[`<ErrorBoundary>`](../view/07_errors.md)), wrap it in `<stack>`:

```rust
// BAD — won't compile if value() returns Result<String, _>
<label>{move || value()}</label>

// GOOD — <stack> takes arbitrary Render children
<stack>{move || value()}</stack>
```

This is the most common "label rejected my closure" gotcha. The
restriction exists because Label's child setter is typed as
`IntoMaybeReactive<String>` for diff-based update efficiency.

## Multi-line

By default a label sizes to a single line. For wrapping body
text:

```rust
<label
    multiline=true
    line_break=LineBreakMode::WordWrap
    max_width=320.0>
    "This is a long paragraph that needs to wrap across multiple lines. \
     Setting `multiline=true` is necessary; `max_width` is what actually \
     forces wrapping at a particular width."
</label>
```

On GTK and iOS, wrapping is automatic once the label is bounded.
