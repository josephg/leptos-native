# Migrating from Web Leptos

This appendix is for readers familiar with upstream
[Leptos](https://leptos.dev). It lists what's different in this
native fork, what's missing, and what to use instead.

## What still works the same

- **Reactivity primitives.** `RwSignal`, `signal()`, `Memo`,
  `Effect`, `ArcRwSignal`, `AsyncDerived` — identical.
- **`view!{}` macro.** Same syntax. Angle-bracket tags, dynamic
  attributes via `{move || ...}` closures, `on:event=` for
  events, `bind:key=` for two-way binding.
- **`#[component]` and props.** Identical. Same `#[prop(...)]`
  attributes. `TypedChildren<V>` / `TypedChildrenFn<V>` for
  children.
- **Control flow components.** `<For>`, `<Show>`, `<Switch>`,
  `<Match>`, `<ErrorBoundary>` — all present.
- **Stores.** `reactive_stores::{Store, Patch}` works the same.
- **NodeRef and directives.** Same API.
- **Context.** `provide_context` / `use_context` unchanged.

## What's removed

### SSR / hydration / WASM

Gone entirely. There's no `RenderHtml` trait, no `hydrate_*`
calls, no `to_html`, no `IsomorphicView`. This is a native
binary, not a webapp.

### Router (`leptos_router`)

Removed. There's no URL-driven navigation paradigm on macOS,
iOS, or GTK — navigation is platform-specific (split view +
toolbar on macOS, navigation stack on iOS, etc.). Build your own
navigation state with signals and `<Switch>` / `<Match>`.

### Meta (`leptos_meta`)

Removed. No `<head>` to write into, no `<Title>` or `<Meta>` to
manage.

### Server functions

Removed. No `#[server]`, no `ServerAction`, no
`server_fn::client`. There's no client-server boundary.

### Resources and `<Suspense>`

Removed. `Resource<T>` is gone; the closest equivalent is
`AsyncDerived<T>` from the reactive graph.

### Server-driven forms

`<ActionForm>` and the related `Action::server_action` flow are
gone. Forms in this fork are just signals plus a click handler;
see [Forms and Inputs](../view/05_forms.md).

### Type-erased views

There's no `AnyView`. Each branch of an `if`/`else`/`Switch`
needs a concrete type. This is what enables compile-time
checking of every renderable value but also what makes
`<Show fallback=...>` and `<Slots>` not yet implementable.

## What's not yet implemented

These would fit the architecture but haven't been built:

- **`<Transition>`, `<AnimatedShow>`** — would need platform
  animation integration (CoreAnimation on macOS / iOS, GTK
  transitions).
- **`<Slots>`** — `#[slot]` macro not yet exposed. Use multiple
  `TypedChildren` props as a workaround. See
  [Passing Children to Components](../view/09_component_children.md).
- **`Suspend`** for async views in the renderer — present in
  the reactive graph crate but not wired into the view pipeline.

## What's added

- **Native element tags.** `<vstack>`, `<hstack>`, `<button>`,
  `<text_field>`, `<grid>`, `<scroll_view>`, `<menu_bar>`,
  `<toolbar>`, `<split_view>`, etc. — these are the visible API
  in the native fork.
- **Native entry points.** `mount_to_window` (Cocoa, GTK),
  `mount_to_split_window` (Cocoa), `run` (all), and
  `leptos::mount_ios::run` (iOS).
- **Native attributes.** `bind:checked`, `sf_symbol`,
  `corner_radius`, `background_color`, `grid_column`,
  `flex_grow`, and many others. Documented per-element in the
  [Element Reference](../elements/README.md).

## Porting a web Leptos component

```rust
// Web Leptos
#[component]
fn Counter() -> impl IntoView {
    let count = RwSignal::new(0);
    view! {
        <div class="counter">
            <p>"Count: " {move || count.get()}</p>
            <button on:click=move |_| count.update(|n| *n += 1)>"+1"</button>
        </div>
    }
}
```

```rust
// Native Leptos
#[component]
fn Counter() -> impl IntoView {
    let count = RwSignal::new(0);
    view! {
        <vstack padding=12.0 gap=8.0>
            <label>{move || format!("Count: {}", count.get())}</label>
            <button on:click=move |_| count.update(|n| *n += 1)>"+1"</button>
        </vstack>
    }
}
```

The component body, the signal usage, the event handler are all
unchanged. The visible diff is:

- `<div>` → `<vstack>` (a flex container)
- CSS classes / inline styles → layout attributes
  (`padding=12.0 gap=8.0`)
- `<p>` → `<label>`
- Text concatenation inside JSX → explicit `format!` returning a
  `String` from a closure

## Layout differences

CSS is gone. Layout attributes are baked directly into the
elements:

| CSS                        | Native attribute                  |
|----------------------------|-----------------------------------|
| `display: flex; flex-direction: column;` | `<vstack>` |
| `padding: 16px;`           | `padding=16.0`                    |
| `margin: 8px;`             | `margin=8.0`                      |
| `gap: 12px;`               | `gap=12.0`                        |
| `flex: 1;`                 | `flex_grow=1.0`                   |
| `width: 240px;`            | `width=240.0`                     |
| `background: red;`         | `background_color=Color::RED`     |
| `border-radius: 8px;`      | `corner_radius=8.0`               |
| `display: grid;`           | `<grid>`                          |
| `grid-column: 1 / -1;`     | `grid_column=(1, -1)`             |

There's no global stylesheet. Reusable styling is done by
factoring components: a `<Card>` component holds the shared
padding/corner-radius/background, and you use it as
`<Card>...</Card>`.

## Where to read upstream docs

A lot of the upstream Leptos book still applies — the reactive
chapters, components and props, control flow, error handling.
What doesn't apply is the SSR / router / meta / progressive
enhancement / deployment chapters. The original book lives at
<https://book.leptos.dev/>.
