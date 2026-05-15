# iOS / UIKit

The iOS port mirrors Cocoa closely under the hood — same Taffy
layout, same target/action handler pattern, same overall element
shape. The platform-specific differences come from UIKit's
single-scene app model and the absence of windowing, menus, and
some controls.

- [App Lifecycle](./lifecycle.md) — how an iOS app boots,
  `AppDelegate` / `SceneDelegate`, the `run()` entry point.
- [Safe Area and Keyboard](./safe_area.md) — automatic padding
  for notches/home-indicators and keyboard avoidance.
- [Building and Launching](./building.md) — the `run_ios.sh`
  script, `-t` for non-interactive builds.
- [Differences from macOS](./deltas.md) — name changes
  (`<switch>` instead of `<checkbox>`), missing controls,
  things that don't apply.

## What you don't get

- **No `mount_to_window`** — only `run`. iOS apps are a single
  fullscreen scene.
- **No menu bar / `<menu_bar>`** — UIKit has no menu bar in the
  desktop sense.
- **No `<toolbar>` or `<split_view>`** — UIKit's equivalents
  (UINavigationBar, UISplitViewController) are scene-style
  navigation paradigms; they're a different shape from the
  declarative Cocoa toolbar and aren't yet exposed in this fork.
- **No `<checkbox>`** — use `<switch>` (UISwitch).
- **No `<pop_up_button>` or `<color_well>`** — both are macOS
  patterns without inline iOS equivalents.

## What you get for free

- **Safe-area insets** automatically applied as padding on the
  content root, so layouts stay clear of the notch and home
  indicator.
- **Keyboard avoidance** — the keyboard layout guide pushes
  content up when the software keyboard appears.
- **Taffy layout** with the same `<vstack>` / `<hstack>` /
  `<grid>` semantics as on macOS.
