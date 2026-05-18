# Split View

`<split_view>` wraps NSSplitViewController. Each pane has its
own Taffy layout tree and can be independently collapsed,
resized, or pinned.

```rust
use leptos::prelude::*;

mount_to_split_window("Notes", (900.0, 600.0), || view! {
    <split_view>
        <split_pane
            behavior=PaneBehavior::Sidebar
            preferred_thickness=220.0
            minimum_thickness=180.0
            can_collapse=true>
            <Sidebar />
        </split_pane>
        <split_pane>
            <Editor />
        </split_pane>
        <split_pane
            behavior=PaneBehavior::Inspector
            preferred_thickness=280.0
            can_collapse=true>
            <Inspector />
        </split_pane>
    </split_view>
})
.run();
```

## `mount_to_split_window`

Split views need an NSSplitViewController as the window's
`contentViewController`, which is different from
`mount_to_window`'s plain content view setup. Use this entry
point:

```rust
mount_to_split_window(title, (width, height), || view! { <split_view>...</split_view> }).run();
```

Like the other cocoa mount entry points, `mount_to_split_window`
returns an `AppHandle` — see [the mount entry-point
overview](./README.md#apphandle-and-the-run-chain).

The closure **must** return a `<split_view>`.

You can also use `run(|| view! { <window>...</window> })` and
put a `<split_view>` inside a regular `<window>` — that creates
a child NSSplitView, not a split-controller-backed window. The
distinction matters for native sidebar behaviour and the toolbar
toggle.

## `<split_view>`

| Attribute  | Type   | Notes                                |
|------------|--------|--------------------------------------|
| `vertical` | `bool` | Vertical split (default: horizontal).|

Children are `<split_pane>`s. Two or three panes is normal;
more is supported but rare.

## `<split_pane>`

| Attribute              | Type                | Notes                                                |
|------------------------|---------------------|------------------------------------------------------|
| `behavior`             | `PaneBehavior`      | `Default`, `Sidebar`, `Inspector`. Affects native styling and toggle wiring. |
| `preferred_thickness`  | `f64`               | Initial width (or height for vertical splits).      |
| `minimum_thickness`    | `f64`               | Lower resize bound.                                  |
| `maximum_thickness`    | `f64`               | Upper resize bound.                                  |
| `holding_priority`     | `f32`               | NSLayoutPriority for the pane's resistance to resize.|
| `can_collapse`         | `bool`              | Allow programmatic / user collapse.                  |
| `collapsed`            | `bool`              | Reactive — pane is collapsed when `true`.            |
| `collapse_behavior`    | `CollapseBehavior`  | Animation style on collapse.                         |

`behavior=PaneBehavior::Sidebar` is what makes
`<toolbar_toggle_sidebar/>` (covered in
[Toolbar](./toolbar.md)) work — that toolbar item collapses
whichever pane is the sidebar.

`behavior=PaneBehavior::Inspector` does the same for inspector
panes (typically the right-hand panel).

## Reactive collapsing

`collapsed=` is reactive, so a button anywhere in the app can
toggle a pane:

```rust
let show_inspector = RwSignal::new(true);

view! {
    <split_view>
        <split_pane behavior=PaneBehavior::Sidebar>...</split_pane>
        <split_pane>...</split_pane>
        <split_pane
            behavior=PaneBehavior::Inspector
            collapsed=move || !show_inspector.get()>
            <Inspector />
        </split_pane>
    </split_view>

    <button on:click=move |_| show_inspector.update(|s| *s = !*s)>
        "Toggle inspector"
    </button>
}
```

The toolbar's sidebar toggle interacts with the same reactive
state — clicking it updates the underlying NSSplitViewItem
collapsed state, which AppKit drives back through the pane's
`collapsed=` binding when present.

## Per-pane Taffy trees

Each pane is its own Taffy layout tree. Sizing inside one pane
doesn't affect the others. The split-view divider position is
managed by NSSplitViewController, not by Taffy.

## See also

- `cocoa/examples/pages/src/main.rs` — Pages-style three-pane
  app with toolbar.
