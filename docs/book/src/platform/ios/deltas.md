# Differences from macOS

The iOS port mirrors the macOS port wherever possible. This page
lists the visible deltas you'll trip on when porting code
between the two.

## Element name changes

| macOS              | iOS                  | Why                                       |
|--------------------|----------------------|-------------------------------------------|
| `<checkbox>`       | `<switch>`           | UIKit's toggle widget is UISwitch.        |
| —                  | (none for popup)     | No UIKit equivalent for inline popup.     |
| —                  | (none for color)     | No UIKit equivalent for inline color well.|

`<switch>` is a Rust keyword, so the macro emits `r#switch`. You
write `<switch>` normally in `view!{}`.

## Elements not on iOS

- **`<pop_up_button>`** — UIMenu and UIPickerView are different
  shapes (UIMenu is a popover, UIPickerView is a spinner).
- **`<color_well>`** — UIColorPickerViewController is a modal
  sheet.
- **`<checkbox>`** — use `<switch>`.

If you need cross-port code that doesn't break compilation on
either side, gate the platform-specific bits with
`cfg(target_os = ...)`:

```rust
#[cfg(target_os = "macos")]
fn theme_picker(idx: RwSignal<usize>) -> impl IntoView {
    view! { <pop_up_button items=vec!["Light", "Dark"] bind:value=idx /> }
}

#[cfg(target_os = "ios")]
fn theme_picker(idx: RwSignal<usize>) -> impl IntoView {
    view! { <segmented_control items=vec!["Light", "Dark"] bind:selection=idx /> }
}
```

## Entry points

| macOS                         | iOS                          |
|-------------------------------|------------------------------|
| `mount_to_window`             | — (only `run`)               |
| `mount_to_split_window`       | — (no split-view backing)    |
| `run`                         | `leptos::mount_ios::run`     |

## Window / scene model

- **macOS**: `<window>` builder, multi-window, `WindowHandle`,
  programmatic title / size / position.
- **iOS**: no `<window>`. One UIWindow, one fullscreen scene,
  no title, no positioning. The simulator and the actual device
  decide screen size.

## Menu bar

- **macOS**: `<menu_bar>` / `<menu>` / `<menu_item>` builds the
  system menu bar.
- **iOS**: no menu bar. iPadOS has a hidden menu bar that
  appears via Cmd-key, but that surface isn't currently exposed.

## Toolbar / split view

- **macOS**: `<toolbar>` / `<split_view>` available.
- **iOS**: not in this fork. UIKit's analogues
  (UINavigationBar / UISplitViewController) are scene-driven and
  haven't been wrapped yet.

## Decoration attributes

Both ports support `background_color`, `corner_radius`,
`border_width`, `border_color`, `clip` on container elements.
The implementations differ (CALayer vs UIKit equivalents); the
end result looks the same.

## SF Symbols

Both ports support `sf_symbol=` on `<button>` and
`<image_view>`. The same symbol names work on both. See
[SF Symbols](../cocoa/sf_symbols.md).

## Events

`on:click`, `on:input`, `on:change`, `on:focus`, `on:blur`,
`on:keydown`, `on:keyup` are all available on iOS. Click events
on container `<view>` / `<vstack>` / `<hstack>` are wired via
UITapGestureRecognizer — that's iOS-only; Cocoa requires a
proper button.

## File picker, share sheet, alerts

iOS-specific UI patterns (alerts, action sheets, share sheets,
photo picker) aren't yet exposed. For now, drop down to objc2 and
call UIKit APIs directly via a NodeRef.
