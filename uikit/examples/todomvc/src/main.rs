//! iOS port of the upstream `todomvc` example. Demonstrates:
//!   * `<For>` keyed iteration with mount/unmount cycles
//!   * `local_storage()` persistence (NSUserDefaults-backed)
//!   * `node_ref` + `on_load` + `Element::focus()` for autofocus
//!   * `<switch>` for completion toggles
//!
//! UI deltas from the cocoa original:
//!   * Add via an explicit "+" button rather than the Return key —
//!     iOS has no `on:keydown` (deferred until `UIKeyCommand`
//!     wiring lands), and the soft keyboard's Return key isn't a
//!     reliable trigger. An explicit button is also more
//!     iOS-idiomatic.
//!   * No Escape-to-cancel on row edits; commit-on-blur via
//!     `on:change`. Same reason.
//!   * `<switch>` instead of `<checkbox>`.

extern crate leptos_uikit as leptos_platform;

#[cfg(target_os = "ios")]
mod app {
    use leptos_platform::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    pub const STORAGE_KEY: &str = "todomvc-leptos-ios";

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Todos(pub Vec<Todo>);

    impl Default for Todos {
        fn default() -> Self {
            let starting = local_storage()
                .ok()
                .flatten()
                .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
                .and_then(|v| serde_json::from_str::<Vec<Todo>>(&v).ok())
                .unwrap_or_default();
            Self(starting)
        }
    }

    impl Todos {
        fn add(&mut self, todo: Todo) {
            self.0.push(todo);
        }
        fn remove(&mut self, id: Uuid) {
            self.0.retain(|t| t.id != id);
        }
        fn remaining(&self) -> usize {
            self.0.iter().filter(|t| !t.completed.get()).count()
        }
        fn completed_count(&self) -> usize {
            self.0.iter().filter(|t| t.completed.get()).count()
        }
        fn clear_completed(&mut self) {
            self.0.retain(|t| !t.completed.get_untracked());
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Todo {
        pub id: Uuid,
        pub title: RwSignal<String>,
        pub completed: RwSignal<bool>,
    }

    impl Todo {
        fn new(title: String) -> Self {
            Self {
                id: Uuid::new_v4(),
                title: RwSignal::new(title),
                completed: RwSignal::new(false),
            }
        }
    }

    #[component]
    pub fn TodoMVC() -> impl IntoView {
        let todos = RwSignal::new(Todos::default());
        provide_context(todos);

        let new_todo = RwSignal::new(String::new());

        let new_todo_ref = NodeRef::new();
        new_todo_ref.on_load(|el| {
            let _ = el.focus();
        });

        let add = move || {
            let title = new_todo.get_untracked();
            let trimmed = title.trim();
            if !trimmed.is_empty() {
                let new = Todo::new(trimmed.to_string());
                todos.update(|t| t.add(new));
                new_todo.set(String::new());
            }
        };

        // Persist on every change.
        Effect::new(move |_| {
            let snapshot = todos.with(|t| t.0.clone());
            if let Ok(Some(storage)) = local_storage() {
                if let Ok(json) = serde_json::to_string(&snapshot) {
                    let _ = storage.set_item(STORAGE_KEY, &json);
                }
            }
        });

        let remaining_label = move || {
            let n = todos.with(|t| t.remaining());
            format!("{} item{} remaining", n, if n == 1 { "" } else { "s" })
        };

        view! {
            <vstack padding=20.0 gap=12.0>
                <label font_size=28.0>"todos"</label>

                <hstack gap=8.0>
                    <text_field
                        node_ref=new_todo_ref
                        placeholder="What needs to be done?"
                        bind:value=new_todo
                        flex_grow=1.0
                    />
                    <button
                        on:click=move |_| add()
                        enabled=move || !new_todo.get().trim().is_empty()>
                        "Add"
                    </button>
                </hstack>

                <scroll_view flex_grow=1.0>
                    <vstack gap=4.0>
                        <For
                            each=move || todos.with(|t| t.0.clone())
                            key=|t| t.id
                            children=move |todo| view! { <TodoRow todo /> }
                        />
                    </vstack>
                </scroll_view>

                <hstack gap=12.0>
                    <label flex_grow=1.0>{remaining_label}</label>
                    <button
                        on:click=move |_| todos.update(|t| t.clear_completed())
                        enabled=move || todos.with(|t| t.completed_count() > 0)
                    >
                        "Clear completed"
                    </button>
                </hstack>
            </vstack>
        }
    }

    #[component]
    pub fn TodoRow(todo: Todo) -> impl IntoView {
        let title = todo.title;
        let completed = todo.completed;
        let id = todo.id;

        // Per-row outer signal that owns the editable text. Initialised
        // from the todo's title; commits back via `on:change` (blur).
        let edit_text = RwSignal::new(title.get_untracked());

        Effect::new(move |_| edit_text.set(title.get()));

        let commit = move |_: String| {
            let v = edit_text.get_untracked();
            let trimmed = v.trim();
            if trimmed.is_empty() {
                if let Some(setter) = use_context::<RwSignal<Todos>>() {
                    setter.update(|t| t.remove(id));
                }
            } else if trimmed != title.get_untracked() {
                title.set(trimmed.to_string());
            }
        };

        let parent_todos = use_context::<RwSignal<Todos>>().expect(
            "TodoRow must be used inside TodoMVC (which provides the \
             Todos context)",
        );

        view! {
            <hstack gap=8.0>
                <switch bind:checked=completed />
                <text_field
                    bind:value=edit_text
                    on:commit=commit
                    flex_grow=1.0
                />
                <button on:click=move |_| parent_todos.update(|t| t.remove(id))>
                    "Delete"
                </button>
            </hstack>
        }
    }

    pub fn main() {
        leptos_platform::mount_ios::run(|| view! { <TodoMVC /> });
    }

}

#[cfg(target_os = "ios")]
fn main() { app::main() }

#[cfg(not(target_os = "ios"))]
fn main() {}
