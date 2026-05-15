# NodeRef and Directives

Most of the time, you don't need to touch the underlying widget —
attribute closures and event handlers cover everything declaratively.
For the rare cases that need imperative access (focus a field,
scroll to a row, register a custom event listener), there are two
escape hatches.

## `NodeRef`

`NodeRef<E>` holds a reference to the underlying element after
build. Create one, attach it to an element via the `node_ref=`
attribute, and read it inside an `Effect`:

```rust
let field_ref = NodeRef::<TextField>::new();

view! {
    <text_field node_ref=field_ref />
}

Effect::new(move |_| {
    if let Some(field) = field_ref.get() {
        field.focus();
    }
});
```

`get()` returns `Option<E>` — it's `None` until the element has
been built. Run your imperative code in an `Effect`, which will
re-run once the element is in the tree.

NodeRef is most useful for:

- Focus management on initial mount.
- Programmatic scrolling.
- Talking to platform APIs the high-level builder doesn't expose
  (e.g. setting a custom NSResponder chain).

The element types live in each port's element module (e.g.
`leptos_cocoa::cocoa::element::TextField`,
`leptos_uikit::ios::element::TextField`).

## Directives (`use:`)

Directives are functions that run **at element build time**,
receiving the constructed element as an argument. They're useful
for one-shot setup that doesn't fit naturally as an attribute.

```rust
fn log_build(_el: Element) {
    eprintln!("[directive] log_build: element built");
}

fn with_param(_el: Element, msg: &'static str) {
    eprintln!("[directive] with_param: {msg}");
}

view! {
    <button use:log_build use:with_param="button built">
        "Click me"
    </button>
    <text_field use:log_build use:with_param="field built" />
}
```

The macro expands `use:name` into a call to the function `name`,
passing the built element. `use:name=arg` passes a second
argument.

This is from `cocoa/examples/directives/src/main.rs`.

## When to choose which

- **`NodeRef`** for *later* — you need the element at some point
  after build (in an effect, in an event handler closure that
  fires after user interaction).
- **`use:`** for *now* — one-shot setup that runs immediately at
  build time and doesn't need to keep a handle around.

Both are escape hatches. If you find yourself reaching for them
constantly, consider whether the same effect could be achieved by
adding the missing attribute to the element builder upstream.
