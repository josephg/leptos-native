# SF Symbols

[SF Symbols](https://developer.apple.com/sf-symbols/) is Apple's
icon set, integrated into the system on macOS 11+ and iOS 13+.
You reference a symbol by name; the OS renders it at any size and
weight, in any colour.

## Where SF Symbols can appear

| Element            | Attribute        | Notes                                          |
|--------------------|------------------|-----------------------------------------------|
| `<button>`         | `sf_symbol=...`  | Replaces the text title with the icon.        |
| `<image_view>`     | `sf_symbol=...`  | Standalone icon image.                        |
| `<toolbar_item>`   | `sf_symbol=...`  | Toolbar icon. Shorthand for `icon=Icon::SfSymbol(...)`. |
| `<menu_item>`      | `icon=...`       | Use `Icon::SfSymbol(name)`.                   |

```rust
view! {
    <hstack gap=8.0>
        <button sf_symbol="plus".to_string()       on:click=move |_| add()    />
        <button sf_symbol="minus".to_string()      on:click=move |_| remove() />
        <button sf_symbol="square.and.pencil".to_string() on:click=move |_| edit() />
    </hstack>
}
```

## Tinting

`<image_view>` supports `tint=Color::...`:

```rust
<image_view sf_symbol="star.fill".to_string() tint=Color::YELLOW />
```

For buttons, set `text_color=` — the icon honours it the same
way it would honour a foreground colour on text:

```rust
<button sf_symbol="trash".to_string() text_color=Color::RED on:click=delete>
    "Delete"
</button>
```

## Finding symbol names

Apple ships the **SF Symbols.app** browser (free; available from
the [SF Symbols page](https://developer.apple.com/sf-symbols/)).
It shows the full catalogue and the canonical names — that's
what to pass as the `sf_symbol=` string.

Examples used by the bundled demos:
`plus`, `minus`, `square.and.pencil`, `trash`, `gearshape`,
`magnifyingglass`, `chevron.left`, `chevron.right`,
`person.crop.circle`, `house.fill`, `star.fill`,
`paintbrush.pointed`, `slider.horizontal.3`.

## Versioning

SF Symbols added many symbols in later macOS releases. Symbols
introduced in macOS 14 won't render on macOS 11. AppKit will
substitute a "question mark in a square" fallback. Test on the
minimum OS version you support.

## iOS

Same API on iOS — `<button sf_symbol=...>`,
`<image_view sf_symbol=...>` work identically. The same name
strings.

## GTK

Not applicable. GTK uses its own theme-based icon system; SF
Symbols are macOS / iOS only.
