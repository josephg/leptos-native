# Application and Windows

GTK4 apps are built around `gtk::Application`. The application
ID is a reverse-DNS string used for single-instance behaviour
and desktop integration.

## `mount_to_window`

The simplest entry point:

```rust
use leptos::prelude::*;

fn main() {
    mount_to_window(
        "org.example.my_app",   // application ID
        "My App",               // window title
        (640, 480),             // size in pixels
        || view! { <Root /> },
    );
}
```

## `run`

When you need more than a single window — multiple windows, a
menu bar, custom app-startup wiring — use `run` and accept the
`gtk::Application` reference:

```rust
use leptos::prelude::*;

fn main() {
    run("org.example.my_app", |app| view! {
        <menu_bar>...</menu_bar>
        {window().application(app.clone())
                 .title("Main")
                 .size(640, 480)
                 .child(view! { <Root /> })}
    });
}
```

The closure can return any `Render<Dom>`. Note that
`<window>` builders inside `run` need `.application(app.clone())`
explicitly — `mount_to_window` does this for you.

## `<window>` builder

| Method            | Argument           | Notes                                |
|-------------------|--------------------|--------------------------------------|
| `.application(app)` | `gtk::Application` | Required for windows built under `run`. |
| `.title(s)`       | `String`           | Window title.                        |
| `.size(w, h)`     | `(i32, i32)`       | Initial content size in pixels.      |
| `.child(view)`    | `Render<Dom>`      | The window's content view.           |

The builder is a chainable Rust API; you'll often use it from
inside a `view!{}` block via `{ ... }` interpolation rather than
the angle-bracket form, since the macro's `<window>` tag doesn't
know about the leading `application` argument.

## Application ID conventions

- Use reverse-DNS: `org.example.app_name`.
- Match the directory you own (the org/example part should be a
  domain you control).
- The same ID is used for `gio::Settings` schema paths if you
  use the settings integration — see [Settings and Theming](./settings.md).
