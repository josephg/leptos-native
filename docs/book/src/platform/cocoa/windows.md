# Windows

The `<window>` builder mounts an NSWindow declaratively as part
of your view tree.

```rust
use leptos::prelude::*;

fn main() {
    run(|| view! {
        <window title="My App".to_string() size=WindowSize(800.0, 600.0)>
            <Root />
        </window>
    });
}
```

## Attributes

| Attribute        | Type                | Notes                                                       |
|------------------|---------------------|-------------------------------------------------------------|
| `title`          | `String`            | Title bar text.                                             |
| `size`           | `WindowSize`        | Initial content size in points: `WindowSize(width, height)`.|
| `position`       | `WindowPosition`    | Initial top-left corner.                                    |
| `toolbar_style`  | `ToolbarStyle`      | Title-bar styling: unified, automatic, expanded, etc.       |
| `handle`         | `WindowHandle`      | Get programmatic control of the window — see below.        |

All four are reactive: passing a closure as the value re-runs
the setter whenever a signal changes.

## Events

| Event       | Payload | Notes                                |
|-------------|---------|--------------------------------------|
| `on:close`  | `()`    | Fires when the window is closing.    |

```rust
<window
    title="Editor"
    size=WindowSize(960.0, 600.0)
    on:close=move |_| save_state()>
    <Editor />
</window>
```

## `WindowHandle`

`WindowHandle::new()` gives you a reactive handle you can pass to
the `handle=` attribute. Once the window is built, you can call:

```rust
let handle = WindowHandle::new();

view! {
    <window handle=handle.clone() title="Pop-up">...</window>

    <button on:click=move |_| handle.close()>
        "Close window"
    </button>
}
```

`close()` programmatically closes the window. Useful for
"Cancel" buttons in modal-like windows, or for keyboard-shortcut
handlers in a menu.

## Multi-window apps

Render multiple `<window>`s under your root:

```rust
run(|| view! {
    <window title="Main"  size=WindowSize(800.0, 600.0)><Main /></window>
    <window title="Tools" size=WindowSize(280.0, 600.0)><Tools /></window>
});
```

Each window has its own Taffy layout tree — the windows are
completely independent for measure/relayout purposes.

You can also build windows conditionally with `<Show>` /
`<Switch>`:

```rust
let show_inspector = RwSignal::new(false);

view! {
    <window title="Main"><Main /></window>
    <Show when=move || show_inspector.get()>
        <window title="Inspector" size=WindowSize(280.0, 600.0)>
            <Inspector />
        </window>
    </Show>
}
```

When `show_inspector` flips false, the window is destroyed and
its NSWindow disposed.

## Quitting behavior

By default, closing the last visible window quits the app —
the bundled `AppDelegate` returns `true` from
`applicationShouldTerminateAfterLastWindowClosed:`.

For menu-bar / status-item apps that should keep running with
no windows open, disable this before mounting:

```rust
use leptos::prelude::*;

fn main() {
    set_quit_on_last_window_close(false);

    run(|| view! {
        <menu_bar>...</menu_bar>
        // The app will keep running after this window closes;
        // re-open it from a menu-bar item.
        <window title="Inspector" size=WindowSize(280.0, 480.0)>
            <Inspector />
        </window>
    });
}
```

You can toggle this at runtime too — call
`set_quit_on_last_window_close(true)` later if you want the app
to quit the next time the last window closes.

## See also

- `cocoa/examples/two_windows/src/main.rs` — minimal
  multi-window demo.
- `cocoa/examples/pages/src/main.rs` — `<window>` paired with
  `<toolbar>` and `<split_view>`.
