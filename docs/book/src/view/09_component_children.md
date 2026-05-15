# Passing Children to Components

A component declares a `children` prop to accept nested views.

```rust
#[component]
fn Card(title: String, children: TypedChildren<impl IntoView>) -> impl IntoView {
    view! {
        <vstack padding=16.0 gap=8.0>
            <label>{title}</label>
            {children.into_inner()()}
        </vstack>
    }
}

// usage
view! {
    <Card title="Notes".to_string()>
        <label>"Inside the card"</label>
        <button>"OK"</button>
    </Card>
}
```

`TypedChildren<V>` wraps a closure of type `Fn() -> V`. Call
`.into_inner()` to take the closure out, then call it to get
the view. The view's type is preserved — there's no `AnyView`
type erasure in this fork.

## The three children types

| Type                         | Purpose                                          |
|------------------------------|--------------------------------------------------|
| `TypedChildren<V>`           | Children called *once*, at component build time. |
| `TypedChildrenFn<V>`         | Children that may be called *multiple* times — e.g. inside a `<Show>` toggle or `<For>` row. |
| `TypedChildrenMut<V>`        | Mutable variant (rare; needed for some control-flow internals). |

Most components want `TypedChildren`. Reach for `TypedChildrenFn`
when the children might re-render, like inside a `<Show>`:

```rust
#[component]
fn Toggleable(
    show: ReadSignal<bool>,
    children: TypedChildrenFn<impl IntoView>,
) -> impl IntoView {
    let inner = children.into_inner();
    view! {
        <Show when=move || show.get()>
            {inner()}
        </Show>
    }
}
```

## Multiple "slots" via multiple children props

There's no `#[slot]` attribute in this fork yet, but you can take
multiple `TypedChildren` props for the same effect:

```rust
#[component]
fn SplitView(
    sidebar: TypedChildren<impl IntoView>,
    detail:  TypedChildren<impl IntoView>,
) -> impl IntoView {
    view! {
        <hstack>
            <vstack width=200.0>{sidebar.into_inner()()}</vstack>
            <vstack flex_grow=1.0>{detail.into_inner()()}</vstack>
        </hstack>
    }
}

// usage
view! {
    <SplitView
        sidebar=view! { <Sidebar /> }
        detail=view!  { <Detail /> }
    />
}
```

(See also [Cocoa Split View](../platform/cocoa/split_view.md) for
the *native* NSSplitViewController-backed version.)

## Projecting children

If your component just forwards children to a single inner
container, that's the projecting-children pattern:

```rust
#[component]
fn Padded(children: TypedChildren<impl IntoView>) -> impl IntoView {
    view! {
        <vstack padding=24.0>
            {children.into_inner()()}
        </vstack>
    }
}
```

You can stack these to compose layout primitives, the same way
you'd build small CSS utility components on the web.
