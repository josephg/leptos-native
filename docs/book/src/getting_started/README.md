# Getting Started

Pick a platform and follow its setup guide. The application code you
write is largely the same across all three platforms — the
prerequisites and the `main()` entry point are what differ.

- [**macOS / Cocoa**](./macos.md) — the most mature port. AppKit
  widgets, NSWindow, native menus, toolbar, split view.
- [**Linux / GTK4**](./gtk.md) — GTK4 widgets driven by Taffy
  layout. Smaller widget set than Cocoa.
- [**iOS / UIKit**](./ios.md) — UIKit single-scene app with
  safe-area and keyboard-avoidance handling. Built and run via a
  shell-script wrapper around `xcrun simctl`.

Every binary picks **exactly one** backend by depending on
`leptos_cocoa`, `leptos_gtk`, or `leptos_uikit` (aliased as
`leptos` in `Cargo.toml`). There's no feature flag to toggle
between them — your binary commits to a platform at the
dependency level.

```toml
# Cargo.toml — pick one
[dependencies]
leptos = { package = "leptos_cocoa" }
# or
leptos = { package = "leptos_gtk", features = ["gtk"] }
# or
leptos = { package = "leptos_uikit" }
```

All three give you the same `leptos::prelude::*` import surface for
your app code.
