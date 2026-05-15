# `<progress_indicator>`

A progress bar (determinate) or spinner (indeterminate).

```rust
// Determinate
<progress_indicator value=move || download.get() max_value=1.0 />

// Indeterminate (spinning)
<progress_indicator indeterminate=true />
```

## Platforms

| Port | Widget               |
|------|----------------------|
| Cocoa| NSProgressIndicator  |
| GTK  | — (not implemented)  |
| iOS  | UIProgressView       |

iOS uses `UIProgressView`. Its visual is a determinate bar only —
an iOS-style spinner would use `UIActivityIndicatorView` instead;
that's not exposed in this fork.

## Attributes

| Attribute                | Type   | Default | Cocoa | iOS | Notes                                                    |
|--------------------------|--------|---------|:-----:|:---:|----------------------------------------------------------|
| `value`                  | `f64`  | `0.0`   | ✓     | ✓   | Current progress in `[0, max_value]`.                    |
| `max_value`              | `f64`  | `1.0`   | ✓     | ✓   | Upper bound.                                             |
| `indeterminate`          | `bool` | `false` | ✓     |     | Cocoa: switch to spinner style.                          |
| `displayed_when_stopped` | `bool` | `true`  | ✓     |     | Cocoa: stay visible (greyed) when no progress is active. |

Plus all [shared layout attributes](../layout/attributes.md).

## Events

None.

## Bindings

None — progress indicators are output-only.

## Example: file copy progress

```rust
let bytes_done = RwSignal::new(0_u64);
let bytes_total = 1024_u64 * 1024 * 100;  // 100 MB

view! {
    <vstack gap=8.0>
        <progress_indicator
            value=move || bytes_done.get() as f64
            max_value=bytes_total as f64 />
        <label>{move || format!(
            "{}/{} MB",
            bytes_done.get() / (1024*1024),
            bytes_total / (1024*1024),
        )}</label>
    </vstack>
}
```

## Indeterminate use

```rust
let loading = RwSignal::new(false);

view! {
    <Show when=move || loading.get()>
        <progress_indicator indeterminate=true />
    </Show>
}
```
