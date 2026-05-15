# `<button>`

A momentary push-button.

```rust
<button on:click=move |_| count.update(|n| *n += 1)>"Click me"</button>
```

## Platforms

| Port | Widget    |
|------|-----------|
| Cocoa| NSButton  |
| GTK  | gtk::Button |
| iOS  | UIButton  |

## Attributes

| Attribute        | Type     | Default       | Cocoa | GTK | iOS | Notes                                                                  |
|------------------|----------|---------------|:-----:|:---:|:---:|------------------------------------------------------------------------|
| `title`          | `String` | `""`          | ✓     | ✓   | ✓   | Visible label. `child=` and a string child (`>"OK"<`) also set it.     |
| `enabled`        | `bool`   | `true`        | ✓     | ✓   | ✓   | Greyed-out if `false`.                                                 |
| `bordered`       | `bool`   | `true`        | ✓     |     |     | Toggle the bezel.                                                      |
| `key_equivalent` | `String` | `""` (none)   | ✓     |     |     | Default activator (e.g. `"\r"` for Return).                            |
| `text_color`     | `Color`  | system label  | ✓     |     | ✓   | System label colour (dark-mode aware) when unset.                      |
| `bold`           | `bool`   | `false`       | ✓     |     |     |                                                                        |
| `sf_symbol`      | `String` | `""` (no icon)| ✓     |     | ✓   | Use an [SF Symbol](../platform/cocoa/sf_symbols.md) for the button image. |
| `font_size`      | `f32`    | system size   | ✓     |     | ✓   | Inherited from `WithText`.                                             |
| `alignment`      | text alignment | center  | ✓     |     | ✓   |                                                                        |

Plus all [shared layout attributes](../layout/attributes.md)
(`padding`, `margin`, `width`, `flex_grow`, etc.) and
[universal attributes](../layout/attributes.md#universal)
(`alpha`, `tool_tip`).

## Events

| Event      | Cocoa | GTK | iOS | Payload |
|------------|:-----:|:---:|:---:|---------|
| `on:click` | ✓     | ✓   | ✓   | `()`    |

There's also a convenience `.on_click(handler)` builder method
that is equivalent to `.on(click, handler)`.

## Bindings

None — buttons have no editable state.

## Single-handler restriction (Cocoa)

NSControl has a single target/action slot. Attaching two click
handlers to one button — `on:click` plus a duplicate from a
wrapping component, or `bind:` on a different attribute that
internally registers a click handler — **panics at build time**.
The error message names the control and suggests a workaround.

If you need a single button to do multiple things, combine them
in one closure:

```rust
<button on:click=move |_| {
    save_form();
    track_event("submit_clicked");
}>"Submit"</button>
```

If you're building a reusable component that has an internal
`on:click` and want callers to also react, accept a callback
prop and invoke it inside the component's own handler:

```rust
#[component]
fn SubmitButton(on_submit: impl FnMut(()) + Send + 'static) -> impl IntoView {
    let mut on_submit = on_submit;
    view! {
        <button on:click=move |_| {
            do_internal_work();
            on_submit(());
        }>
            "Submit"
        </button>
    }
}
```

GTK and iOS don't have the single-slot restriction (GTK signal
handlers stack; iOS uses a fan-out delegate per control). The
restriction is Cocoa-specific.

## Example

```rust
view! {
    <hstack gap=8.0>
        <button on:click=move |_| count.update(|n| *n -= 1)>"-1"</button>
        <button
            enabled=move || count.get() > 0
            on:click=move |_| count.set(0)>
            "Reset"
        </button>
        <button
            sf_symbol="plus".to_string()
            key_equivalent="\r".to_string()
            on:click=move |_| count.update(|n| *n += 1) />
    </hstack>
}
```
