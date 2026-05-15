# Menus

The GTK port's menu API mirrors Cocoa's surface
(`<menu_bar>` / `<menu>` / `<menu_item>` / `<menu_separator/>`),
but the underlying model is different: GTK menus are not part of
the widget tree. They're a `gio::Menu` data model, and the
**desktop shell** renders them — as a hamburger button on GNOME,
a classic title-bar menu on Cinnamon/XFCE, or a global menu via
extensions on macOS-style overlays.

When you build `<menu_item>`s outside the `view!{}` macro (e.g.
from a `Vec` in a loop), import the event marker explicitly so
`on(event::action, …)` resolves:

```rust
use leptos::tachys::html::event;
```

```rust
use leptos::prelude::*;

fn main() {
    run("org.example.menu_demo", |app| {
        let count = RwSignal::new(0);

        let bar = view! {
            <menu_bar>
                <menu title="File">
                    <menu_item
                        title="New"
                        shortcut="n"
                        on:action=move |_| count.update(|n| *n += 1) />
                    <menu_separator/>
                    <menu_item
                        title="Reset"
                        shortcut="r"
                        modifiers=Modifiers::CMD_SHIFT
                        on:action=move |_| count.set(0) />
                </menu>
                <menu title="View">
                    <menu_item title="About" />
                </menu>
            </menu_bar>
        };

        let win = window()
            .application(app.clone())
            .title("Menu demo")
            .size(420, 240)
            .child(view! {
                <label>{move || format!("Count: {}", count.get())}</label>
            });

        (bar, win)
    });
}
```

## API

The element set and attribute names are identical to Cocoa's;
see [Cocoa Menus](../cocoa/menus.md) for the full per-element
reference. The visible differences:

- The menu bar is a **sibling** of the window in the closure's
  return tuple — not a child of `<window>`.
- `gio::SimpleAction`s are generated under the hood, named
  `app.menuitem_N`. The desktop shell uses these for shortcut
  routing.
- Modifiers use the same `Modifiers::CMD | Modifiers::SHIFT`
  pattern. A `Modifiers::CMD_SHIFT` convenience exists in the
  GTK prelude.

## Reactive menu items

Reactive `title=`, `enabled=`, and `checked=` work the same way
as on Cocoa — the underlying `gio::Menu` is rebuilt when the
title changes; `enabled` / `checked` flow through the
`SimpleAction` state.

## Dynamic submenus

Currently, dynamic add/remove of items inside a `<menu>` is
limited. You can statically declare a `Vec<MenuItem>` from a
known list:

```rust
let recent: Vec<_> = recent_titles.iter().map(|&t| {
    menu_item().title(t).on(event::action, move |_| open(t))
}).collect();

view! {
    <menu title="Open Recent">
        {recent}
    </menu>
}
```

Wiring a `<For>` driven by a signal is on the roadmap.

## Desktop integration

- **GNOME** — menu bar appears as a hamburger button.
- **Cinnamon / XFCE / KDE** — usually shown as a classic
  title-bar menu via the AppMenu protocol.
- **macOS host (running a GTK app)** — global menu bar via
  the AppMenu extension.

Test on your target compositor — the visual presentation varies
significantly. The behaviour (actions, shortcuts) is consistent.

## See also

`gtk/examples/menu_demo/src/main.rs` — full working example.
