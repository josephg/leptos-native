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

| Attribute    | Type            | Default       | Cocoa | GTK | iOS | Notes                                                       |
|--------------|-----------------|---------------|:-----:|:---:|:---:|-------------------------------------------------------------|
| `text`       | `String`        | `""`          | ✓     | ✓   | ✓   | The displayed text. A string child (`>"..."<`) sets it too. |
| `try_text`   | `Fn() -> Result<String, E>` | unset | ✓ |  |  | On `Ok`, sets the text; on `Err`, renders empty and flows the error to the nearest `<ErrorBoundary>`. |
| `text_color` | `Color`         | system label  | ✓     |     | ✓   | System label colour (dark-mode aware) when unset.           |
| `alignment`  | text alignment  | natural       | ✓     |     | ✓   | "Natural" follows the user's locale reading direction.      |
| `font_size`  | `f32`           | system size   | ✓     |     | ✓   |                                                             |
| `bold`       | `bool`          | `false`       | ✓     |     |     |                                                             |
| `line_break` | line-break mode | `TruncatingTail` | ✓  |     |     | How long text wraps/truncates.                              |
| `multiline`  | `bool`          | `false`       | ✓     |     |     | Allow wrapping to multiple lines.                           |
| `selectable` | `bool`          | `false`       | ✓     |     |     | Allow user to select & copy the text.                       |

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

`<label>`'s string-typed child accepts only `IntoMaybeReactive<String>`.
To render a value that might be a `Result<T, E>` (so it can flow
through an [`<ErrorBoundary>`](../view/07_errors.md)), use
`.try_text()`:

```rust
<label try_text=move || "12".parse::<i32>().map(|n| n.to_string()) />
```

On `Ok(s)`, the label shows `s`. On `Err(e)`, the label renders
empty *and* the error is registered with the nearest
`<ErrorBoundary>` (so its `fallback` takes over the subtree).

The longer-hand alternative — wrapping in `<stack>` — still
works for arbitrary `Result<T, E>` shapes:

```rust
<stack>{move || value()}</stack>
```

Use `<stack>` when the success value is more complex than a
`String`. Use `.try_text()` for the common case of "label that
might fail to parse / format / fetch."

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
