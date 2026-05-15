# Iteration

Static iteration is just a Rust expression that produces a `Vec`
of views:

```rust
view! {
    <vstack gap=4.0>
        {(0..5).map(|i| view! { <label>{format!("Item {i}")}</label> }).collect::<Vec<_>>()}
    </vstack>
}
```

For a fixed, known-at-build-time set of children, that's fine. But
for *dynamic* lists — where rows are added, removed, or
reordered as signals change — you want `<For>`.

## `<For>`

```rust
<For
    each=move || items.get()
    key=|item| item.id
    children=move |item| view! { <Row item /> }
/>
```

`<For>` is a keyed-diff iterator. On each change to `each()`, it
compares the new sequence against the old by `key` and:

- **inserts** new children at the right position,
- **removes** children whose keys disappeared,
- **moves** children whose keys are in a different position, and
- **leaves alone** the children whose keys haven't moved — so
  their inner state (signals, focus, scroll position) survives a
  reorder.

The three props:

| Prop       | Type                                | Notes                                                                    |
|------------|-------------------------------------|--------------------------------------------------------------------------|
| `each`     | `Fn() -> impl IntoIterator<Item=T>` | The reactive source. Re-read whenever a tracked signal inside it changes.|
| `key`      | `Fn(&T) -> K where K: Eq + Hash`    | Stable identifier per item. Don't use the index unless items are append-only and never reorder. |
| `children` | `Fn(T) -> impl IntoView`            | Per-item view. Called once per *new* key; not re-called for moves.       |

## A real example

From `cocoa/examples/counters/src/main.rs`:

```rust
let counters = RwSignal::new(Vec::<(usize, RwSignal<i32>)>::new());

view! {
    <For
        each=move || counters.get()
        key=|(id, _)| *id
        children=move |(_id, value)| view! { <Row value /> }
    />
}
```

Each row owns its own `RwSignal<i32>` for the inner counter. The
outer `RwSignal<Vec<...>>` only tracks the membership of the list;
mutating the per-row value doesn't re-render the list, only that
row.

## Choosing a key

The key has to be **stable across the lifetime of the item** —
not the index, not a hash of the displayed content. Indices
change when you remove or reorder; content hashes collide.

```rust
// good — id assigned at insertion time, never changes
key=|item| item.id

// bad — index changes on every reorder; you'll rebuild every row
key=|item| /* index */ 0
```

If you don't have a natural ID, allocate one when you push the
item:

```rust
let next_id = RwSignal::new(0_usize);
let add = move || {
    let id = next_id.get_untracked();
    next_id.update(|n| *n += 1);
    items.update(|v| v.push((id, /* ... */)));
};
```

## Reactive `each`

`each` is a closure. Anything you read inside it is tracked. So
filtering and sorting reactively is just normal Rust:

```rust
<For
    each=move || {
        let mut v = items.get();
        v.retain(|item| item.visible);
        v.sort_by_key(|item| item.priority);
        v
    }
    key=|item| item.id
    children=move |item| view! { <Row item /> }
/>
```

The filtered list is computed on every signal change. If the
computation is expensive, wrap it in a `Memo`.

## Limitations

- `<For>` requires owned `T`. If your items are large, store them
  behind `Rc<…>` or `Arc<…>`, or pass IDs and look the row up
  inside the child.
- `<For>` doesn't accept a `fallback` for the empty case in this
  fork — wrap it in a `<Show>` or a `<Switch>` instead:
  ```rust
  <Switch>
      <Match when=move || items.with(|v| v.is_empty())>
          <label>"No items."</label>
      </Match>
      <Match when=move || !items.with(|v| v.is_empty())>
          <For each=... key=... children=.../>
      </Match>
  </Switch>
  ```
