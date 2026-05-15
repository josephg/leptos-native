# Menus

The macOS menu bar is built from `<menu_bar>`, `<menu>`,
`<menu_item>`, and `<menu_separator/>`. The menu bar lives at
the top of the view tree — a sibling of `<window>` — and
renders in the system menu bar at the top of the screen.

```rust
use leptos::prelude::*;

run(|| view! {
    <menu_bar>
        <menu title="App">
            <menu_item title="About My App" />
            <menu_separator />
            <menu_item
                title="Quit"
                shortcut="q"
                modifiers=Modifiers::CMD
                on:action=move |_| std::process::exit(0) />
        </menu>
        <menu title="File">
            <menu_item title="New" shortcut="n" modifiers=Modifiers::CMD
                       on:action=move |_| new_document() />
            <menu_item title="Open…" shortcut="o" modifiers=Modifiers::CMD
                       on:action=move |_| open_document() />
        </menu>
        <menu title="Edit">
            <menu_item title="Cut"  shortcut="x" modifiers=Modifiers::CMD />
            <menu_item title="Copy" shortcut="c" modifiers=Modifiers::CMD />
            <menu_item title="Paste" shortcut="v" modifiers=Modifiers::CMD />
        </menu>
    </menu_bar>

    <window title="My App" size=WindowSize(800.0, 600.0)>
        <Root />
    </window>
});
```

## `<menu_bar>`

A single top-level container. There's only one menu bar per
application; multiple `<menu_bar>` siblings is not supported.

It has no attributes — its only job is to contain `<menu>`s.

## `<menu>`

A submenu within the menu bar (or within another menu — menus
nest).

| Attribute | Type     | Notes              |
|-----------|----------|--------------------|
| `title`   | `String` | Visible menu name. |

Reactive — the menu's title can change at runtime.

## `<menu_item>`

A leaf item.

| Attribute   | Type        | Notes                                                |
|-------------|-------------|------------------------------------------------------|
| `title`     | `String`    | Item label.                                          |
| `enabled`   | `bool`      | Greyed-out if `false`.                               |
| `checked`   | `bool`      | Show ✓ next to the title.                            |
| `icon`      | `Icon`      | Optional icon. Use [SF Symbols](./sf_symbols.md).    |
| `shortcut`  | `String`    | Keyboard shortcut character (e.g. `"q"`, `"n"`).     |
| `modifiers` | `Modifiers` | Bitflags: `CMD`, `CTRL`, `SHIFT`, `ALT`. Combine with `|`. |

Event:

| Event        | Payload | Notes                          |
|--------------|---------|--------------------------------|
| `on:action`  | `()`    | Fires when the item is chosen. |

## `<menu_separator/>`

Visual divider. No attributes, no children, no events.

## A reactive `checked` state

Menu items with reactive `checked=` flip the checkmark as state
changes:

```rust
let dark_mode = RwSignal::new(false);

view! {
    <menu_item
        title="Dark mode"
        checked=move || dark_mode.get()
        on:action=move |_| dark_mode.update(|d| *d = !*d) />
}
```

## Keyboard shortcuts

`shortcut="q"` plus `modifiers=Modifiers::CMD` registers ⌘Q. The
shortcut is interpreted relative to the macOS conventions:

- One-character `shortcut`s map to that physical key.
- `Modifiers::CMD` is the system meta-key (⌘).
- Combine modifiers with `|`:
  ```rust
  modifiers=Modifiers::CMD | Modifiers::SHIFT
  ```

The OS automatically displays the shortcut on the right side of
the menu item.

## Conditional menus and dynamic items

`<menu>` and `<menu_item>` are normal view-tree nodes — you can
conditionally render them with `<Show>` / `<Switch>` / `<For>`:

```rust
<menu title="Recent files">
    <For
        each=move || recent_files.get()
        key=|path| path.clone()
        children=move |path| view! {
            <menu_item
                title={move || path.display().to_string()}
                on:action=move |_| open_file(&path) />
        } />
</menu>
```

## See also

`cocoa/examples/menu_demo/src/main.rs` — a complete menu bar
with shortcuts, checked state, and an action that updates a
window-local signal.
