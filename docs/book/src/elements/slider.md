# `<slider>`

A continuous range slider for an `f64` value.

```rust
let volume = RwSignal::new(0.5_f64);
view! { <slider bind:value=volume min_value=0.0 max_value=1.0 /> }
```

## Platforms

| Port | Widget      |
|------|-------------|
| Cocoa| NSSlider    |
| GTK  | gtk::Scale  |
| iOS  | UISlider    |

## Attributes

| Attribute        | Type   | Default | Cocoa | GTK | iOS | Notes                                |
|------------------|--------|---------|:-----:|:---:|:---:|--------------------------------------|
| `value`          | `f64`  | `0.0`   | ✓     | ✓   | ✓   | Current value. Prefer `bind:value`.  |
| `min_value`      | `f64`  | `0.0`   | ✓     | ✓   | ✓   |                                      |
| `max_value`      | `f64`  | `1.0`   | ✓     | ✓   | ✓   |                                      |
| `enabled`        | `bool` | `true`  | ✓     | ✓   | ✓   |                                      |
| `vertical`       | `bool` | `false` | ✓     |     |     | Cocoa: orient vertically.            |
| `num_tick_marks` | `u32`  | `0`     | ✓     |     |     | Cocoa: render tick marks.            |
| `snaps_to_ticks` | `bool` | `false` | ✓     |     |     | Cocoa: snap to tick positions.       |

Plus all [shared layout
attributes](../layout/attributes.md).

## Events

| Event       | Cocoa | GTK | iOS | Payload |
|-------------|:-----:|:---:|:---:|---------|
| `on:change` | ✓     | ✓   | ✓   | `()`    |

Fires on every value change — both ends of a drag, every tick
along the way. Read the bound signal for the current value.

Most of the time you'll prefer `bind:value` and react to the
signal via an `Effect`:

```rust
let volume = RwSignal::new(0.5);

Effect::new(move |_| {
    apply_audio_volume(volume.get());
});

view! { <slider bind:value=volume min_value=0.0 max_value=1.0 /> }
```

## Bindings

| Bind         | Signal type     | Cocoa | GTK | iOS |
|--------------|-----------------|:-----:|:---:|:---:|
| `bind:value` | `RwSignal<f64>` | ✓     | ✓   | ✓   |

## Example

The `settings` example:

```rust
let muted = RwSignal::new(false);
let volume = RwSignal::new(50.0);

view! {
    <checkbox bind:checked=muted>"Mute"</checkbox>
    <slider
        bind:value=volume
        min_value=0.0 max_value=100.0
        enabled=move || !muted.get() />
    <label>{move || format!("Volume: {:.0}%", volume.get())}</label>
}
```
