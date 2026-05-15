# Components and Props

A component is a function annotated with `#[component]` whose
arguments are its props. The function body returns
`impl IntoView`.

```rust
#[component]
fn Greeting(name: String, exclaim: bool) -> impl IntoView {
    view! {
        <label>{move || if exclaim {
            format!("Hello, {name}!")
        } else {
            format!("Hello, {name}.")
        }}</label>
    }
}
```

Invoke it with JSX-like syntax inside `view!{}`:

```rust
<Greeting name="World".to_string() exclaim=true />
```

The prop names line up with parameter names. Props are positional
in source code but order-independent at the call site (the macro
generates a builder, so `name=…` and `exclaim=…` can appear in
any order).

## Prop attributes

`#[prop(...)]` modifies how a prop behaves:

```rust
#[component]
fn Settings(
    #[prop(default = 16.0)] padding: f32,
    #[prop(optional)] subtitle: Option<String>,
    #[prop(into)] title: String,
) -> impl IntoView {
    /* ... */
}
```

- `#[prop(default = expr)]` — supplies a default; the caller can
  omit the prop.
- `#[prop(optional)]` — equivalent to `default = Default::default()`.
- `#[prop(into)]` — calls `Into::into` on the caller's argument.
  Useful for props typed as `String` so callers can pass `&str`.

## Reactive props

Props can be signals if your component needs to react to changes
from the parent:

```rust
#[component]
fn Counter(value: RwSignal<i32>) -> impl IntoView {
    view! {
        <hstack gap=8.0>
            <button on:click=move |_| value.update(|n| *n -= 1)>"-1"</button>
            <label>{move || value.get().to_string()}</label>
            <button on:click=move |_| value.update(|n| *n += 1)>"+1"</button>
        </hstack>
    }
}
```

The parent owns the signal; the child reads and writes it. This is
the standard pattern for components that need to participate in
two-way state. See [Parent–Child Communication](./08_parent_child.md)
for the four common shapes.

## Components are not nodes

A component invocation expands to inline code. There's no wrapper
view, no extra DOM node, no host element. `<Counter />` in your
view tree is literally the contents that `Counter`'s body returns.

One subtle consequence: spread attributes — including
`on:click=` written directly on a component invocation — work
on **leaf** root elements only. If your component returns
`view! { <button>...</button> }`, the caller can attach
`on:click=` to your invocation and it flows through to the
button. If it returns `view! { <stack>...</stack> }`, attaching
`on:click=` (or any spread attribute) at the call site is a
compile error.

See [Parent–Child Communication](./08_parent_child.md), Method 3,
for the working pattern.

## Naming

The `view!{}` macro routes tags by name. Element tags are
`snake_case` (`<text_field>`, `<scroll_view>`). Component
invocations are `PascalCase` (`<MyComponent />`). The macro uses
the case to decide whether to look up an element builder or treat
the name as a component.

## Children

A component receives children by declaring a `children` prop:

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
    <Card title="Hello".to_string()>
        <label>"Inside the card"</label>
        <button>"Click"</button>
    </Card>
}
```

`TypedChildren<V>` runs the children closure once. Use
`TypedChildrenFn<V>` if your component might need to render the
children multiple times (for example, inside a `<Show>` toggle).

See [Passing Children to Components](./09_component_children.md)
for the full prop types.

## Returning `impl IntoView`

Always return `impl IntoView`. Don't return a concrete element
type (`Button<...>`, `Stack<...>`, etc.) — `IntoView` is the
trait that ties everything together, and the concrete types are
deliberately not part of the public API. They change as the
renderer evolves.

## Slots

The `#[slot]` macro from upstream Leptos is not yet exposed on
this fork. If you need the equivalent of named slots, model them
as props that accept `TypedChildren`:

```rust
#[component]
fn ButtonRow(
    primary: TypedChildren<impl IntoView>,
    secondary: TypedChildren<impl IntoView>,
) -> impl IntoView {
    view! {
        <hstack gap=8.0>
            {primary.into_inner()()}
            {secondary.into_inner()()}
        </hstack>
    }
}
```
