# `<stepper>`

An up/down stepper for an `f64` value. Typically paired with a
`<label>` or `<text_field>` showing the current value.

```rust
let quantity = RwSignal::new(1.0);
view! {
    <hstack gap=4.0 align=AlignItems::Center>
        <label>{move || quantity.get().to_string()}</label>
        <stepper bind:value=quantity min_value=0.0 max_value=99.0 increment=1.0 />
    </hstack>
}
```

## Platforms

| Port | Widget       |
|------|--------------|
| Cocoa| NSStepper    |
| GTK  | — (not implemented) |
| iOS  | UIStepper    |

## Attributes

| Attribute        | Type   | Cocoa | iOS | Notes                                       |
|------------------|--------|:-----:|:---:|---------------------------------------------|
| `value`          | `f64`  | ✓     | ✓   | Current value. Prefer `bind:value`.         |
| `min_value`      | `f64`  | ✓     | ✓   | Lower bound.                                |
| `max_value`      | `f64`  | ✓     | ✓   | Upper bound.                                |
| `increment`      | `f64`  | ✓     | ✓   | Step size per click. iOS: `step_value`.     |
| `enabled`        | `bool` | ✓     | ✓   |                                             |

Plus all [shared layout
attributes](../layout/attributes.md).

## Events

| Event      | Cocoa | iOS | Payload |
|------------|:-----:|:---:|---------|
| `on:click` | ✓     | ✓   | `()`    |

Stepper "click" semantics mean *value changed*: both the up and
down arrows fire it. Read the current value from the bound
signal, not from the event payload.

## Bindings

| Bind         | Signal type     | Cocoa | iOS |
|--------------|-----------------|:-----:|:---:|
| `bind:value` | `RwSignal<f64>` | ✓     | ✓   |
