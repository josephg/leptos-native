# `<date_picker>`

A date and/or time picker.

```rust
use leptos_native::prelude::*;

let date = RwSignal::new(Date::now());
view! { <date_picker bind:value=date /> }
```

## Platforms

| Port | Widget               |
|------|----------------------|
| Cocoa| NSDatePicker         |
| GTK  | — (not implemented)  |
| iOS  | UIDatePicker         |

## Attributes

| Attribute   | Type             | Default     | Cocoa | iOS | Notes                                                  |
|-------------|------------------|-------------|:-----:|:---:|--------------------------------------------------------|
| `value`     | `Date`           | now         | ✓     | ✓   | Current date. Prefer `bind:value`.                     |
| `min_date`  | `Date`           | unset       | ✓     | ✓   | Lower bound (inclusive).                               |
| `max_date`  | `Date`           | unset       | ✓     | ✓   | Upper bound (inclusive).                               |
| `style`     | picker style     | textual (Cocoa) / wheels (iOS) | ✓ | ✓ | Visual style. Cocoa: textual / stepper / clock-and-calendar. iOS: wheels / compact / inline. |
| `enabled`   | `bool`           | `true`      | ✓     | ✓   |                                                        |

Plus all [shared layout attributes](../layout/attributes.md).

## Events

| Event       | Cocoa | iOS | Payload |
|-------------|:-----:|:---:|---------|
| `on:change` | ✓     | ✓   | `()`    |

Fires when the date value changes. Read the new value from the
bound signal.

## Bindings

| Bind         | Signal type      | Cocoa | iOS |
|--------------|------------------|:-----:|:---:|
| `bind:value` | `RwSignal<Date>` | ✓     | ✓   |

## The `Date` type

Each port exposes a `Date` type:

- Cocoa: `cocoa_dom::Date` — wraps `NSDate`.
- iOS: `ios_dom::Date` — wraps `NSDate`.

Both share the same conceptual API (constructors from
year/month/day, conversion to/from `chrono::DateTime` if you
bring that crate in). Re-exported by each port's prelude.
