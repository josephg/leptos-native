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
mount_to_window("My App", (640.0, 480.0), || view! { <Root /> });
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
});
```

`run` accepts any `Render<Dom>`. Use `<window>` builders directly
when you want multiple windows or a menu bar.
