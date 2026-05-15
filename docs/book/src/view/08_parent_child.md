# Parent–Child Communication

When a child component needs to push state back up to a parent,
there are four common shapes. Pick the smallest one that fits.

This chapter follows the `parent_child_cocoa` example, which
demonstrates all four side-by-side.

## 1. Pass a `WriteSignal` as a prop

The most explicit: the parent owns the signal and hands its
setter half to the child.

```rust
#[component]
fn App() -> impl IntoView {
    let (red, set_red) = signal(false);
    view! {
        <ButtonA setter=set_red />
        <label>{move || format!("Red: {}", red.get())}</label>
    }
}

#[component]
fn ButtonA(setter: WriteSignal<bool>) -> impl IntoView {
    view! { <button on:click=move |_| setter.update(|v| *v = !*v)>"Toggle"</button> }
}
```

Use this when the parent both reads and writes the value but the
child only needs the write side. It also makes the dependency
explicit in the type signature.

## 2. Pass a closure as a prop

If the child shouldn't know about the signal at all, pass a
callback:

```rust
#[component]
fn ButtonB(on_click: impl FnMut(()) + Send + 'static) -> impl IntoView {
    view! { <button on:click=on_click>"Click"</button> }
}

// usage
<ButtonB on_click=move |_| set_green.update(|v| *v = !*v) />
```

Best when the child is fully generic and could be wired to
anything — a reusable button, a row in a list, etc.

## 3. `on:` directly on the component invocation

A component invocation can accept `on:click` (and other events)
if it forwards them to a child element. The macro spreads matching
event attributes through to the component's root.

```rust
#[component]
fn ButtonC() -> impl IntoView {
    view! { <button>"Click"</button> }
}

// usage — the on:click is wired through to ButtonC's <button>
<ButtonC on:click=move |_| set_blue.update(|v| *v = !*v) />
```

Use this for thin, "look like an element" wrappers. Don't combine
it with an internal `on:click` on the same control — on Cocoa,
that panics at build time (NSControl has a single target/action
slot; see [button](../elements/button.md)).

## 4. `provide_context` / `use_context`

For state that crosses many levels, use context. The parent
provides a value, and any descendant can look it up.

```rust
#[derive(Copy, Clone)]
struct ToggleContext(WriteSignal<bool>);

#[component]
fn App() -> impl IntoView {
    let (cyan, set_cyan) = signal(false);
    provide_context(ToggleContext(set_cyan));
    view! { <DeepInTree /> }
}

#[component]
fn ButtonD() -> impl IntoView {
    let setter = use_context::<ToggleContext>().unwrap().0;
    view! {
        <button on:click=move |_| setter.update(|v| *v = !*v)>"Toggle"</button>
    }
}
```

Use this for shared state — theming, auth, navigation, a
selection model — that's needed in many distant components and
would be tedious to thread through every prop.

`provide_context` keys by the value's type. Wrap your signal in a
newtype struct so multiple `WriteSignal<bool>`s don't collide:

```rust
struct DarkMode(RwSignal<bool>);
struct Compact(RwSignal<bool>);
```

## When to use which

| Shape                       | When                                            |
|-----------------------------|-------------------------------------------------|
| `WriteSignal` prop          | Tight coupling; one signal, one child.          |
| Closure prop                | Reusable child, parent decides what to do.      |
| `on:` on component          | Element-like wrapper; one event.                |
| Context                     | Cross-cutting state; many distant consumers.    |

## A reading signal pattern

Reading state goes the other way: the parent gives the child a
`Signal<T>` (read-only) or an `RwSignal<T>` (read+write). The
child reads it via `.get()` and reacts to changes inside its
closures. That's already the standard pattern shown in
[A Basic Component](./01_basic_component.md).
