# Linux / GTK4

The GTK port covers fewer platform-specific features than Cocoa,
but it does ship:

- [Application and Windows](./windows.md) — the `<window>`
  builder and `gtk::Application` integration.
- [Menus](./menus.md) — `<menu_bar>` / `<menu>` /
  `<menu_item>` backed by `gio::Menu` and rendered by the
  desktop shell.
- [Settings and Theming](./settings.md) — persisting state to
  `gio::Settings` and styling via GTK CSS.

## What's not in the GTK port (yet)

Compared to Cocoa, GTK doesn't have builders for:

- `scroll_view`, `segmented_control`, `stepper`, `color_well`,
  `date_picker`, `progress_indicator`, `image_view`, `text_view`
- `toolbar`, `split_view`

You can compose roughly equivalent results with what's available
— a `<grid>` with `flex_grow=1.0` panes approximates a split
view, GTK CSS can style a `<vstack>` like a card — but the
native equivalents aren't there yet. See the
[implementation log](https://github.com/josephg/leptos-mac/blob/main/gtk_implementation_log.md)
for the current priorities.
