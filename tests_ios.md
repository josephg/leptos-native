# iOS / UIKit port — test plan

The iOS port deliberately mirrors the macOS port; many of the
test ideas in `tests_macos.md` apply verbatim. This file only
captures what's iOS-specific.

`uikit/dom/tests/layout.rs` exists today but doesn't compile with
`cargo test --workspace` (no `main()`). First action: gate it on
`#[cfg(target_os = "ios")]` or move into a `#[cfg]`-gated module so
the workspace test build is clean.

## High priority

- □ `cargo test -p ios_dom --target aarch64-apple-ios-sim` runs to
  completion in CI (or at least locally on a Mac).
- □ Element creation: every tag in `Element::create` returns a view
  of the right Objective-C class (`isKindOfClass:`).
- □ Handler stores: same drop-1000-buttons regression as cocoa.
- □ `viewDidLayoutSubviews` re-runs Taffy on bounds change.
- □ Safe-area + keyboard inset propagates to content root padding
  (this is the iOS-specific bit).
- □ `<switch>` (UISwitch) emits `ValueChanged`, not
  `TouchUpInside`.
- □ `<text_view>` delegate fan-out works (UITextView is not a
  UIControl, uses delegate).

## Parity-with-cocoa

(All of these have a cocoa equivalent; iOS coverage is thin.)

- □ `bind:value` two-way on `<text_field>`.
- □ `bind:checked` two-way on `<switch>`.
- □ `bind:selection` on `<segmented_control>`.
- □ `set_attribute` no-op-on-equal.
- □ `mark_dirty` after structural mutations.
- □ Two click handlers on one UIButton → build-time panic.

## Lower priority

- □ `node_ref` / `directives` once they're added (currently absent
  per `audit_ios.md`).
- □ `<scroll_view>` inside a bounded parent.
- □ Multi-trait control (slider + bind:value + on:change) doesn't
  drop events.
- □ App lifecycle: `application:didFinishLaunchingWithOptions:`
  invokes the user closure under a fresh `Owner`, mounts under
  the content root.
- □ Mangled class name issue: `AppDelegate::class().name()` matches
  what's passed to `uiapplication_main`.
