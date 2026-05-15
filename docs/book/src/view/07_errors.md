# Error Handling

`<ErrorBoundary>` catches errors from any `Result<T, E>` rendered
inside it and shows a fallback view instead.

```rust
use leptos::prelude::*;

#[component]
fn ParseDemo() -> impl IntoView {
    let text  = RwSignal::new(String::from("0"));
    let value = move || text.get().parse::<i32>();

    view! {
        <vstack padding=20.0 gap=12.0>
            <label>"Type an integer (or something that's not)"</label>
            <text_field bind:value=text />

            <ErrorBoundary fallback=|errors| {
                let errors = errors.clone();
                view! {
                    <vstack gap=4.0>
                        <label>"Not an integer! Errors:"</label>
                        <label>{move || {
                            errors.read()
                                .iter()
                                .map(|(_, e)| e.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        }}</label>
                    </vstack>
                }
            }>
                <stack>
                    {move || value().map(|n| format!("You entered: {n}"))}
                </stack>
            </ErrorBoundary>
        </vstack>
    }
}
```

This is the `error_boundary_cocoa` example, abbreviated.

## How it works

- `Result<T, E>` implements `Render` when `T: Render` and `E:
  Into<Error>`. A successful `Ok(value)` renders `value` normally;
  an `Err(e)` registers the error with the nearest enclosing
  `<ErrorBoundary>` and renders nothing.
- The boundary tracks an `ArcRwSignal<Errors>` (an indexed map of
  error-id → boxed error). When non-empty, it renders the
  `fallback` instead of the children; when emptied (the next
  successful re-render), it goes back to the children.
- The `fallback` signature is `Fn(ArcRwSignal<Errors>) -> impl
  IntoView`. The closure can read `errors.read()` to iterate /
  format each error.

## Gotcha: use `<stack>`, not `<label>`

`<label>`'s child accepts only `String` (and `IntoMaybeReactive<String>`
in general). It does **not** accept `Result<T, _>`, so it can't
hand errors off to the boundary. Wrap a closure-returning-Result
in `<stack>` (or any generic container), whose `.child()` is
generic over any `Render` value:

```rust
// Wrong — won't compile; Result<String, _> isn't String.
<label>{move || something_that_can_fail()}</label>

// Right — <stack> accepts arbitrary Render children, including
// Result<T, E>, so the error flows to the boundary.
<stack>{move || something_that_can_fail()}</stack>
```

## Multiple errors

A single boundary can catch errors from anywhere in its subtree —
including from sibling branches. The `Errors` map is keyed by an
internal id, so you'll see every active error in the fallback,
not just the first.

```rust
let a = RwSignal::new(String::from("1"));
let b = RwSignal::new(String::from("2"));

view! {
    <ErrorBoundary fallback=|errors| {
        let errors = errors.clone();
        view! {
            <vstack>
                <label>"Errors:"</label>
                <For
                    each=move || errors.read().iter()
                        .map(|(_, e)| e.to_string())
                        .collect::<Vec<_>>()
                    key=|s| s.clone()
                    children=|s| view! { <label>{s}</label> } />
            </vstack>
        }
    }>
        <vstack gap=8.0>
            <text_field bind:value=a />
            <text_field bind:value=b />
            <stack>{move || a.get().parse::<i32>().map(|x| x.to_string())}</stack>
            <stack>{move || b.get().parse::<i32>().map(|x| x.to_string())}</stack>
        </vstack>
    </ErrorBoundary>
}
```

Type a non-number into both fields and the fallback shows both
errors at once.

## Custom error types

Any error that `Into`s into `leptos::Error` works:

```rust
#[derive(Debug, thiserror::Error)]
enum MyError {
    #[error("network: {0}")] Network(String),
    #[error("parse: {0}")]   Parse(#[from] std::num::ParseIntError),
}

// `thiserror::Error` already gives you Display, so .map_err and
// `?` propagate fine. Convert to leptos::Error at the boundary.
```

## What boundaries don't do

`<ErrorBoundary>` only catches errors that flow through the
**view tree**. Panics inside event handlers, async tasks, or
non-Result computations aren't caught here — they fall through
to whatever the platform does with them (typically a crash). Use
ordinary Rust error handling (`Result`, `?`, `match`) inside
event handlers; the boundary covers the view-render path.
