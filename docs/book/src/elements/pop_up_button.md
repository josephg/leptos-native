# `<pop_up_button>`

A dropdown menu of mutually-exclusive choices.

```rust
let selected = RwSignal::new(0_usize);
view! {
    <pop_up_button
        items=vec!["Small", "Medium", "Large"]
        bind:value=selected />
}
```

## Platforms

| Port | Widget                |
|------|-----------------------|
| Cocoa| NSPopUpButton         |
| GTK  | gtk::DropDown         |
| iOS  | — (not implemented)   |

iOS has no inline popup equivalent — UIMenu and UIPickerView
are both different shapes. Use `<segmented_control>` for small
choice sets, or build a custom modal sheet.

## Attributes

| Attribute    | Type           | Cocoa | GTK | Notes                                                |
|--------------|----------------|:-----:|:---:|------------------------------------------------------|
| `items`      | `Vec<String>`  | ✓     | ✓   | Choice titles. Static (set once at build).           |
| `selection`  | `usize`        | ✓     | ✓   | Selected index. Prefer `bind:value`.                 |
| `enabled`    | `bool`         | ✓     | ✓   |                                                      |
| `pulls_down` | `bool`         | ✓     |     | Cocoa: pull-down menu style instead of pop-up.       |

Plus all [shared layout
attributes](../layout/attributes.md).

## Events

| Event      | Cocoa | GTK | Payload |
|------------|:-----:|:---:|---------|
| `on:click` | ✓     | ✓   | `()`    |

Fires when the selection changes.

## Bindings

| Bind         | Signal type        | Cocoa | GTK |
|--------------|--------------------|:-----:|:---:|
| `bind:value` | `RwSignal<usize>`  | ✓     | ✓   |

The bound value is the **selected index**, not the string. Map
to your domain type with a `Memo`:

```rust
#[derive(Copy, Clone, PartialEq)]
enum Theme { Light, Dark, System }

let idx = RwSignal::new(0_usize);
let theme = Memo::new(move |_| match idx.get() {
    0 => Theme::Light, 1 => Theme::Dark, _ => Theme::System,
});

view! {
    <pop_up_button
        items=vec!["Light", "Dark", "System"]
        bind:value=idx />
}
```
