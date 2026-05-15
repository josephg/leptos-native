# `<date_picker>`

A date and/or time picker.

```rust
use leptos::prelude::*;

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

| Attribute   | Type             | Cocoa | iOS | Notes                                                  |
|-------------|------------------|:-----:|:---:|--------------------------------------------------------|
| `value`     | `Date`           | ✓     | ✓   | Current date. Prefer `bind:value`.                     |
| `min_date`  | `Date`           | ✓     | ✓   | Lower bound (inclusive).                               |
| `max_date`  | `Date`           | ✓     | ✓   | Upper bound (inclusive).                               |
| `style`     | picker style     | ✓     | ✓   | Visual style. Cocoa: textual / stepper / clock-and-calendar. iOS: wheels / compact / inline. |
| `enabled`   | `bool`           | ✓     | ✓   |                                                        |

Plus all [shared layout attributes](../layout/attributes.md).

## Events

| Event      | Cocoa | iOS | Payload |
|------------|:-----:|:---:|---------|
| `on:click` | ✓     | ✓   | `()`    |

`on:click` fires when the date value changes. Read the new value
from the bound signal.

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
