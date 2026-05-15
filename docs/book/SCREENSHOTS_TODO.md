# Screenshot wishlist

This is a living checklist of screenshots that would strengthen
the book. Each entry names the chapter that wants the image, what
it should show, and the example crate (if any) to capture it
from. Filenames are suggested — actual paths will land under
`docs/book/src/images/`.

## Cross-platform / introduction

- [ ] **Counter on macOS, GTK, iOS** side-by-side, three columns.
      Same `counter_*` example built for each port.
      `images/intro_counter_three_ports.png`
- [ ] **Greeter example** running — text field + reactive
      label. From `cocoa/examples/greeter` or any port.
      `images/intro_greeter.png`

## Getting Started

- [ ] **macOS Counter window** — the very first
      `mount_to_window` result.
      `images/getting_started_macos_counter.png`
- [ ] **GTK Counter window** under GNOME and (optionally) another
      desktop shell, to show shell-rendered title bar.
      `images/getting_started_gtk_counter_gnome.png`
- [ ] **iOS Counter in the simulator** — full simulator chrome
      visible.
      `images/getting_started_ios_simulator.png`

## Part 1: UI building

- [ ] **`<For>` keyed iteration** — `counters` example with
      multiple rows, including a screenshot mid-shuffle if
      possible to convey moves vs rebuilds.
      `images/for_iteration_counters.png`
- [ ] **Forms / login form** — empty state, partially-filled
      state, and the moment `enabled=Memo` flips to enabled.
      Three frames.
      `images/forms_login_states.png`
- [ ] **ErrorBoundary fallback** — `error_boundary` example with
      invalid input showing the fallback view.
      `images/error_boundary_fallback.png`

## Part 3: Layout

- [ ] **vstack + hstack composition** — annotated screenshot
      with arrows showing axis direction. Built from a contrived
      small demo.
      `images/layout_vstack_hstack.png`
- [ ] **Grid dashboard** — the `grid_cocoa` example screenshot
      with row/column overlay annotations.
      `images/layout_grid_dashboard.png`
- [ ] **Scroll view** — `scroll_view_cocoa` showing both
      scrolled-top and scrolled-mid states.
      `images/layout_scroll_view.png`

## Part 4: Element reference

One small focused screenshot per element page. Capture the
element in a typical state:

- [ ] `button` — default and hovered/pressed/disabled.
- [ ] `label` — default and bold/coloured.
- [ ] `text_field` — empty (placeholder) and filled.
- [ ] `secure_text_field` — filled, showing dots.
- [ ] `text_view` — multi-line content with selection.
- [ ] `checkbox` (Cocoa, GTK) — unchecked and checked.
- [ ] `switch` (iOS) — off and on.
- [ ] `slider` — at a mid-range value.
- [ ] `stepper` — visible up/down chevrons.
- [ ] `segmented_control` — three options with one selected.
- [ ] `pop_up_button` — collapsed and open menu.
- [ ] `date_picker` — Cocoa textual + stepper style and iOS
      compact style.
- [ ] `color_well` — with a chosen colour visible.
- [ ] `progress_indicator` — determinate (mid-progress) and
      indeterminate (spinning).
- [ ] `image_view` — bundled image rendered.
- [ ] `scroll_view` — with content scrolled.
- [ ] `stack`/`vstack`/`hstack`/`view` — minimal demo of each.
- [ ] `grid` — small 3x3 demo.

## Part 5a: macOS platform features

- [ ] **`<window>` builder result** — vanilla NSWindow with
      title.
- [ ] **Multi-window** — `two_windows_cocoa` example.
- [ ] **Native menu bar** — `menu_demo_cocoa`. Capture the
      menu bar with one menu open showing items, shortcuts,
      and a separator.
      `images/cocoa_menus_open.png`
- [ ] **Menu item with checked state** — same example, item
      with `.checked()=true` visible (✓ next to its title).
- [ ] **Toolbar** — `toolbar_demo_cocoa`:
  - [ ] Default toolbar with several `<toolbar_item>`s.
  - [ ] Toolbar with `<toolbar_search_item>` expanded.
  - [ ] Toolbar with `<toolbar_toggle_sidebar/>` and split-view
        collapsed vs expanded — two screenshots.
  - [ ] Toolbar with `.navigational()=true` items styled
        differently.
- [ ] **Split view** — `pages_cocoa`:
  - [ ] Three-column layout: sidebar + main + inspector.
  - [ ] Sidebar collapsed via `<toolbar_toggle_sidebar/>`.
  - [ ] Inspector hidden state.
- [ ] **SF Symbols** — a button with `.sf_symbol("plus")` and
      an `<image_view sf_symbol=...>` next to each other.

## Part 5b: GTK platform features

- [ ] **GTK window** under GNOME, showing default chrome.
- [ ] **GTK menu bar integration** — `menu_demo_gtk`, captured
      both on a shell that renders an app menu (e.g. AppMenu
      extension) and one that doesn't (showing the hamburger).
- [ ] **GSettings demo** — `settings_gtk` running with a
      checkbox, slider, popup, and the on-disk effect visible
      via `dconf-editor` side-by-side. Optional.

## Part 5c: iOS platform features

- [ ] **iOS app full-screen** — `counter_ios` in portrait,
      with status bar.
- [ ] **Safe-area padding** — same app with notch-class device
      so the safe-area inset is visible.
- [ ] **Keyboard avoidance** — text-field app with software
      keyboard up, showing content shifted above it. Use
      `controls_ios` or `todomvc_ios`.

## Notes

Capture order: layout chapters first, then per-element pages,
then platform-features (the most photogenic). Use the same OS
appearance / theme / scale across screenshots in a chapter so
they look like one set.
