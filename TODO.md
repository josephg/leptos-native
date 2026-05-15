# TODO

Top-level cross-port wishlist. Per-port lists live in
[`TODO_ios.md`](./TODO_ios.md), [`tests_macos.md`](./tests_macos.md),
[`tests_gtk.md`](./tests_gtk.md). Detailed API cleanups are in
[`API_REVIEW.md`](./API_REVIEW.md).

## Examples

- Reactivity fuzz tests
- Complex app layout examples
  - Apple Pages (cocoa/examples/pages — shipped)
  - Spotify (cocoa/examples/spotify — shipped)
  - iOS settings app
  - Discord-shaped chat UI
  - HackerNews reader

## Features

- Global overrides for default font and styles
- All the Cocoa properties (text shadow, transform, paragraph style, …)
- Tokio / etc runtime integration + worked examples
- macOS: sane way to bundle a binary into an `.app` (something
  like `cargo bundle`, but tuned for this fork)
- macOS: app icon support
- macOS: NSDocument-style `DocumentView` (NSView subclass with
  print / save panels wired)
- iOS: `UINavigationController` / nav stack (tracked in
  [`TODO_ios.md`](./TODO_ios.md))
- Linux: app icon support
- ImageView/Button: collapse `sf_symbol=` / `source=` setters onto
  the unified `icon=Icon::…` enum the toolbar and menu items
  already use

## Big features

- Animation primitive — see the discussion in `API_REVIEW.md`
  (deferred to P3 there)
- Native `<table>` / `<list>` (NSTableView / UICollectionView)
- Drag and drop, clipboard, accessibility, printing
- Android support
- Windows support

## Dev tooling

- Hot module reloading
- Chrome / DevTools introspection protocol
- Layout debug overlay across all ports (cocoa has it behind the
  `debug-overlay` feature; port to gtk / uikit)

## Deployment / pre-1.0

- Clean up git / GitHub
- Website
- Rename → `pachys`
- Attribute-level rustdoc pass on every builder
- Layout-engine documentation (Taffy + how the ports plug in)
