# `<segmented_control>`

A row of mutually-exclusive choices — "tabs" in everything but
name.

```rust
let mode = RwSignal::new(0_usize);
view! {
    <segmented_control
        items=vec!["List", "Grid", "Map"]
        bind:selection=mode />
}
```

## Platforms

| Port | Widget                |
|------|-----------------------|
| Cocoa| NSSegmentedControl    |
| GTK  | — (not implemented)   |
| iOS  | UISegmentedControl    |

## Attributes

| Attribute        | Type             | Default | Cocoa | iOS | Notes                                       |
|------------------|------------------|---------|:-----:|:---:|---------------------------------------------|
| `items`          | `Vec<String>`    | `[]`    | ✓     | ✓   | Segment titles in order. Static (set once at build).|
| `selection`      | `usize`          | `0`     | ✓     | ✓   | Selected index. Prefer `bind:selection`.    |
| `enabled`        | `bool`           | `true`  | ✓     | ✓   |                                             |
| `segment_style`  | segment style    | automatic | ✓   |     | Cocoa: capsule / textured / etc.            |

Plus all [shared layout
attributes](../layout/attributes.md).

## Events

| Event       | Cocoa | iOS | Payload |
|-------------|:-----:|:---:|---------|
| `on:change` | ✓     | ✓   | `()`    |

Fires when the selection changes. Read the bound signal for the
new index.

## Bindings

| Bind             | Signal type        | Cocoa | iOS |
|------------------|--------------------|:-----:|:---:|
| `bind:selection` | `RwSignal<usize>`  | ✓     | ✓   |

## Example: tab bar

```rust
#[derive(Copy, Clone, PartialEq)]
enum Tab { List, Grid, Map }

let tab = RwSignal::new(0_usize);
let current = move || match tab.get() {
    0 => Tab::List, 1 => Tab::Grid, _ => Tab::Map,
};

view! {
    <vstack gap=8.0>
        <segmented_control
            items=vec!["List", "Grid", "Map"]
            bind:selection=tab />
        <Switch>
            <Match when=move || current() == Tab::List><ListView /></Match>
            <Match when=move || current() == Tab::Grid><GridView /></Match>
            <Match when=move || current() == Tab::Map ><MapView  /></Match>
        </Switch>
    </vstack>
}
```
