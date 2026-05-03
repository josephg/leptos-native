# cocoa_examples — aspirational reference

The files in this directory are **not yet runnable**. They sketch what
Leptos-on-Cocoa should look like once the macOS port is far enough along.
Treat them as a target spec — when each Stage lands, we adapt the closest
example until it compiles and runs, then promote it.

## What's missing for these to compile

As of today (mid Stage 2), the renderer (`cocoa_dom`) and the tachys
view core compile, but:

- **Stage 3 — events + main-thread spawner**: `on:click`, `on:input`,
  `mount_to(window, …)`, the AppKit run loop integration. Needed by
  every example.
- **Stage 4 — taffy layout**: `<stack_view>` actually flows its children;
  `width`/`height`/`spacing` actually do something.
- **Stage 5 — Cocoa-flavoured element module + macro support**: the
  `<text_field>`, `<button>`, `<stack_view>`, etc. element builder
  functions and the `view!{}` macro accepting them.
- **Stage 5+ — `bind:` rebuild**: `bind:value`, `bind:state`,
  `bind:color`, `bind:selection`. See implementation_log.md.
- **Stage 5+ — Cocoa NodeRef**: needed by `05_bind_vs_manual.rs` to
  show the manual workaround.

## Naming conventions assumed by these examples

- **Tag names** drop the `NS` prefix and are snake_case:
  `<view>` → `NSView`, `<text_field>` → `NSTextField`,
  `<stack_view>` → `NSStackView`, `<color_well>` → `NSColorWell`.
- **Attributes** are the Cocoa property name in snake_case:
  `string_value`, `min_value`, `max_value`, `placeholder`,
  `orientation`, `spacing`, `enabled`. (`enabled` not `disabled` — we
  match `NSControl::setEnabled:`, not the HTML inversion.)
- **Events** are HTML-style `on:click`, `on:input`, `on:change`. Stage 3
  decides exactly how these dispatch.
- **`bind:`** mirrors the web shape but each impl picks the right
  Cocoa hook (target/action, delegate, NSControlTextDidChange).

## Files

| File | Demonstrates | First stage that can run it |
|---|---|---|
| `01_counter.rs` | signals, `on:click`, dynamic text | Stage 5 |
| `02_greeter.rs` | `bind:value` on a text field | Stage 5+ (after bind) |
| `03_settings.rs` | slider, checkbox, popup, color well | Stage 5+ (after bind) |
| `04_login_form.rs` | a real form with Memo gating a button | Stage 5+ (after bind) |
| `05_bind_vs_manual.rs` | the same form written with and without `bind:` | Stage 5+ (after bind + NodeRef) |
