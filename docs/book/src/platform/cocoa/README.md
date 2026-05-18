# macOS / Cocoa

The Cocoa port is the most mature of the three, with extra
features that take advantage of AppKit specifically:

- [Windows](./windows.md) — the `<window>` builder, multi-window
  apps, the `WindowHandle` for programmatic control.
- [Menus](./menus.md) — the native menu bar, submenus, items
  with shortcuts and checked state.
- [Toolbar](./toolbar.md) — NSToolbar with items, search field,
  spacers, sidebar toggle.
- [Split View](./split_view.md) — NSSplitViewController-backed
  multi-pane layout with collapsing sidebars/inspectors.
- [SF Symbols](./sf_symbols.md) — using Apple's icon set in
  buttons and image views.

These features are Cocoa-only. Apps that want a single window
with no menu bar or toolbar can ignore this whole section —
`mount_to_window` plus the [Element Reference](../../elements/README.md)
is enough.

## When to use `run` vs `mount_to_window`

```rust
// Simple — one window, no menu bar, no toolbar.
mount_to_window("My App", (640.0, 480.0), || view! { <Root /> }).run();
```

vs

```rust
// Full — multi-window, menu bar, custom Toolbar wired into the
// window from the view tree.
run(|| view! {
    <menu_bar>...</menu_bar>
    <window title="Main" size=WindowSize(640.0, 480.0)>
        <toolbar>...</toolbar>
        <Root />
    </window>
}).run();
```

`run` accepts any `Render<Dom>`. Use `<window>` builders directly
when you want multiple windows or a menu bar.

## `AppHandle` and the `.run()` chain

Every mount entry point — `run`, `mount`, `mount_to_window`,
`mount_to_split_window` — returns an `AppHandle`. The handle
owns the things that need to outlive the build closure but die
with the process:

- the shared `NSApplication` retain,
- the AppDelegate retain (kept alive because NSApplication holds
  it weakly),
- the root reactive `Owner` your view's signals live under,
- the built view `State` (NSWindows, Taffy trees, RenderEffects,
  …).

Calling `.run()` consumes the handle, enters the AppKit run
loop (blocks until the app terminates), and then drops the
handle in declared field order — view state → owner → app →
delegate. Cleanup happens **in scope** rather than being
skipped via `mem::forget`.

`AppHandle` is `#[must_use]`, so forgetting `.run()` is a
compiler warning, not a silent leak. If you want to build the
app without showing it (test setup, headless validation), bind
the handle to a local and drop it explicitly:

```rust
let app = mount_to_window("Tests", (320.0, 200.0), || view! { <Root /> });
do_something(&app);
drop(app);  // tears the app down without ever calling run()
```

You can also do work after `.run()` returns:

```rust
fn main() {
    let _telemetry = init_telemetry();
    mount_to_window("My App", (640.0, 480.0), || view! { <Root /> }).run();
    save_user_settings();  // runs after the user quits
}
```
