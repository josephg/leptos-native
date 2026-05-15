# App Lifecycle

An iOS app's entry point is `run`:

```rust
fn main() {
    leptos::mount_ios::run(|| view! { <Root /> });
}
```

`run` doesn't return — it calls `UIApplicationMain`, which owns
the main loop for the rest of the process's life.

## What happens at startup

1. **`main()`** stores your view closure in a thread-local and
   calls `UIApplicationMain`.
2. **UIKit instantiates `AppDelegate`** and asks it for a scene
   configuration. The delegate hands back a programmatic config
   pointing at our `SceneDelegate` (no Info.plist scene
   manifest needed).
3. **`SceneDelegate::scene:willConnectToSession:options:`**:
   - Creates the UIWindow sized to the device screen.
   - Builds a `RootViewController` and sets it as the window's
     root.
   - Creates a fresh reactive `Owner` and runs your stored view
     closure inside it.
   - Mounts the result into the RootViewController's content
     root.
4. **`makeKeyAndVisible`** — the app becomes visible.
5. **Run loop** — UIKit takes over, dispatching touches and
   events.

## `RootViewController`'s job

The `RootViewController` overrides `viewDidLayoutSubviews` to:

1. Run Taffy's `compute_layout` against the current bounds.
2. Push the current `safeAreaInsets` (notch, home indicator,
   status bar) onto the content root's padding.
3. Push the current `keyboardLayoutGuide().layoutFrame()` as a
   bottom inset.

This means your `<vstack>` at the top of the view tree is
already correctly inset for the device chrome; you don't need to
add manual padding for the notch or keyboard.

## Why there's no `<window>` builder

iOS apps run as a single fullscreen scene by default (multi-window
on iPad is scene-based and a different shape entirely). There's
nothing to set a title or position on. `run` is the only entry
point.

If you need to detect orientation, size class, or insets at the
Rust level, look up the active `UIWindow` via the
`UIApplication.sharedApplication` ObjC bridge or via a
`NodeRef` to your content root.

## Lifecycle hooks

The `AppDelegate` / `SceneDelegate` lifecycle callbacks
(`application:didFinishLaunching:`,
`sceneDidEnterBackground:`, etc.) are not currently exposed to
user code as event types. To hook in, modify
`uikit/dom/src/app.rs` and add your own callback dispatch.

This is a known limitation; on the roadmap is exposing
background / foreground transitions as something like
`on:background` / `on:foreground` events on a future `<app>`
element.

## See also

- `uikit/dom/src/app.rs` — AppDelegate / SceneDelegate /
  RootViewController source.
- `uikit/leptos_uikit/src/mount.rs` — `run()` source.
