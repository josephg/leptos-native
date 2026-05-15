# Global State

There's no special "global state" primitive — application-wide
state is just a signal (or store) provided via `provide_context`
near the root of your tree.

## Pattern: a context-provided store

```rust
use leptos::prelude::*;
use reactive_stores::{Patch, Store};

#[derive(Debug, Default, Store, Patch)]
pub struct AppState {
    pub dark_mode: bool,
    pub current_user: Option<String>,
    pub notifications_enabled: bool,
}

fn main() {
    mount_to_window("My App", (640.0, 480.0), || {
        let state = Store::new(AppState::default());
        provide_context(state);
        view! { <Root /> }
    });
}

#[component]
fn Root() -> impl IntoView {
    let state = use_context::<Store<AppState>>().expect("AppState provided");
    view! {
        <vstack>
            <TitleBar />
            <Body />
        </vstack>
    }
}

#[component]
fn TitleBar() -> impl IntoView {
    let state = use_context::<Store<AppState>>().expect("AppState");
    view! {
        <hstack padding=8.0 gap=8.0>
            <label>{move || state.current_user().get().unwrap_or_else(|| "anon".into())}</label>
            <checkbox
                bind:checked=state.dark_mode()
            >
                "Dark mode"
            </checkbox>
        </hstack>
    }
}
```

Three ideas in this pattern:

- **One provider near the root.** Every descendant can read it
  via `use_context`.
- **Type-keyed lookup.** Wrap distinct logical states in their
  own types so a `Store<AppState>` doesn't collide with a
  `Store<FormState>`.
- **Field-level reactivity from `#[derive(Store)]`.** Views that
  read only `state.dark_mode()` don't re-render when
  `current_user` changes.

## Pattern: multiple slices

For larger apps, split state into a few independent contexts:

```rust
provide_context(Store::new(AuthState::default()));
provide_context(Store::new(UiState::default()));
provide_context(Store::new(NavigationState::default()));
```

Each one is independently retrieved by `use_context::<Store<...>>()`.

## Pattern: signals instead of stores

For state with just a handful of values, plain signals are
simpler:

```rust
fn main() {
    mount_to_window("App", (640.0, 480.0), || {
        provide_context(RwSignal::new(false));   // dark mode
        view! { <Root /> }
    });
}

#[component]
fn Toggle() -> impl IntoView {
    let dark = use_context::<RwSignal<bool>>().unwrap();
    view! { <checkbox bind:checked=dark>"Dark"</checkbox> }
}
```

When you have more than two or three of these, prefer wrapping
them in newtype structs so the type-keyed lookup doesn't get
ambiguous:

```rust
struct DarkMode(RwSignal<bool>);
struct Compact(RwSignal<bool>);
provide_context(DarkMode(RwSignal::new(false)));
provide_context(Compact(RwSignal::new(false)));
```

## Persistence

To persist global state, drive an `Effect` off the signal or the
store fields. Stores don't expose a single "snapshot the whole
struct" call — instead, subscribe to the fields you actually want
to persist:

```rust
let state = Store::new(AppState::load_from_disk());

Effect::new({
    let state = state.clone();
    move |_| {
        // Read each persisted field — each read subscribes
        // the effect to that field's changes.
        save_dark_mode(state.dark_mode().get());
        save_user(state.current_user().get());
    }
});

provide_context(state);
```

If you really do want a "the whole struct changed" hook, derive
`Clone` on your state and use a single `RwSignal<AppState>`
instead of a `Store<AppState>`. Stores give you per-field
granularity in exchange for losing the "snapshot" affordance.

For macOS, that "disk" is typically `UserDefaults`. For GTK,
`gio::Settings`. For iOS, also `UserDefaults` (via the
`apple_shared` crate's helpers).
