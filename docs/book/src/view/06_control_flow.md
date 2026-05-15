# Control Flow

Three control-flow components cover almost everything: `<Show>`,
`<Switch>` / `<Match>`, and `<For>` (covered in
[Iteration](./04_iteration.md)).

For tiny one-shot branches, a closure returning an
`Option<impl IntoView>` is also fine.

## Reactive closures

Anywhere a view tree expects a child, you can give it a closure
that returns a view. The closure is reactive — when a signal
inside it changes, the view rebuilds.

```rust
view! {
    <vstack>
        {move || if logged_in.get() {
            view! { <label>"Welcome back"</label> }
        } else {
            view! { <label>"Please sign in"</label> }
        }}
    </vstack>
}
```

Both branches must have the *same* concrete type. If they don't,
the closure won't type-check (this fork has no `AnyView` to
paper over the mismatch). Use `<Switch>` / `<Match>` (below) for
mixed-type branches.

## `<Show>`

`<Show>` toggles its children based on a `when` predicate:

```rust
<Show when=move || logged_in.get()>
    <label>"You're signed in."</label>
</Show>
```

When `when()` is true, the children render; when false, nothing.

```admonish note
`<Show>` in this fork does **not** have a `fallback` prop —
upstream Leptos's fallback relied on `AnyView` type erasure,
which the native renderer doesn't provide. For an else-branch,
use `<Switch>`/`<Match>` (below) or two `<Show>` components with
inverted predicates.
```

## `<Switch>` and `<Match>`

For a multi-way branch with different concrete view types per
arm, use `<Switch>`:

```rust
<Switch>
    <Match when=move || page.get() == Page::Home>
        <HomeView />
    </Match>
    <Match when=move || page.get() == Page::Settings>
        <SettingsView />
    </Match>
    <Match when=move || page.get() == Page::About>
        <AboutView />
    </Match>
</Switch>
```

The first `Match` whose `when` returns true wins. If none match,
nothing renders. Up to eight `<Match>` arms per `<Switch>`.

This is the workhorse for navigation in apps without a router.

## A worked example: empty-state fallback

`<For>` doesn't itself accept a fallback, but `<Switch>` makes the
pattern clean:

```rust
<Switch>
    <Match when=move || items.with(|v| v.is_empty())>
        <vstack padding=24.0 gap=8.0>
            <label>"No items yet."</label>
            <button on:click=move |_| /* add */ {}>"Add one"</button>
        </vstack>
    </Match>
    <Match when=move || !items.with(|v| v.is_empty())>
        <For each=move || items.get() key=|i| i.id children=move |i| view! { <Row item=i /> } />
    </Match>
</Switch>
```

## When to reach for what

- One condition, single concrete branch → `<Show>`.
- Multiple conditions or branches with different types →
  `<Switch>` / `<Match>`.
- Dynamic list with stable identity → `<For>`.
- Tiny inline branch with one type → `move || if cond { … } else { … }`.
