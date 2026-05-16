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
| iOS  | UIButton + UIMenu (iOS 14+) |

On iOS the popup is implemented as a UIButton whose `menu`
property holds a UIMenu. The button shows its current title; a
tap reveals the menu; selecting an item updates both the button's
title and the bound signal.

## Attributes

| Attribute    | Type           | Default | Cocoa | GTK | iOS | Notes                                                |
|--------------|----------------|---------|:-----:|:---:|:---:|------------------------------------------------------|
| `items`      | `Vec<String>`  | `[]`    | ✓     | ✓   | ✓   | Choice titles. Static (set once at build).           |
| `selection`  | `usize`        | `0`     | ✓     | ✓   | ✓   | Selected index. Prefer `bind:value`.                 |
| `enabled`    | `bool`         | `true`  | ✓     | ✓   | ✓   |                                                      |
| `pulls_down` | `bool`         | `false` | ✓     |     |     | Cocoa: pull-down menu style instead of pop-up.       |

Plus all [shared layout
attributes](../layout/attributes.md).

## Events

| Event       | Cocoa | GTK | iOS | Payload |
|-------------|:-----:|:---:|:---:|---------|
| `on:change` | ✓     | ✓   | ✓   | `()`    |

Fires when the selection changes. Read the bound signal for the
new index.

## Bindings

| Bind             | Signal type        | Cocoa | GTK | iOS |
|------------------|--------------------|:-----:|:---:|:---:|
| `bind:value`     | `RwSignal<usize>`  | ✓     | ✓   | ✓   |

The bound value is the **selected index**, not the string.

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
