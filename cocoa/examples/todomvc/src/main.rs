//! macOS port of the upstream `todomvc` example. Demonstrates the
//! Tier-1 features added in this branch:
//!   * `on:keydown` for Enter (commit) / Escape (cancel) on
//!     text fields
//!   * `Element::focus()` for auto-focusing the new-todo input
//!     on launch
//!   * `local_storage()` persistence (via `NSUserDefaults`)
//!
//! Differences from the web original:
//!   * No URL/hash routing for filter modes — uses a popup instead.
//!   * No double-click-to-edit. Each todo row is always editable
//!     in-place (Enter saves, Escape reverts). This is simpler
//!     and arguably nicer macOS UX than the web pattern.
//!   * No CSS-based "hidden" toggling; we hide via
//!     `hidden=move || …`.

use leptos_native::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const STORAGE_KEY: &str = "todomvc-leptos-macos";

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
        // `get_untracked` here: the click handler that calls this
        // isn't a reactive tracking context, and dragging the
        // completed signals' subscribers in via `.get()` would tie
        // unrelated effects to "user clicked Clear completed."
        // Snapshot the values explicitly.
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
fn TodoMVC() -> impl IntoView {
    let todos = RwSignal::new(Todos::default());
    // Make the signal available to <TodoRow/> for its Delete
    // button without threading it through props.
    provide_context(todos);

    let new_todo = RwSignal::new(String::new());

    // node_ref for the new-todo input — used to autofocus on
    // launch.
    let new_todo_ref = NodeRef::new();
    new_todo_ref.on_load(|el| {
        let _ = el.focus();
    });

    let add_on_enter = move |ev: KeyEvent| {
        if ev.key != "Enter" {
            return;
        }
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
            <label>"todos"</label>
            <text_field
                node_ref=new_todo_ref
                placeholder="What needs to be done?"
                bind:value=new_todo
                on:keydown=add_on_enter
            />

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
                <label>{remaining_label}</label>
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
fn TodoRow(todo: Todo) -> impl IntoView {
    let title = todo.title;
    let completed = todo.completed;
    let id = todo.id;

    // Per-row outer signal that owns the editable text. Initialised
    // from the todo's title; commits back via Enter or implicit
    // (`change` = focus loss).
    let edit_text = RwSignal::new(title.get_untracked());

    // Push title → edit_text whenever the underlying signal changes
    // (e.g. from another row mutating, though our UI doesn't do that
    // today; defensive).
    Effect::new(move |_| edit_text.set(title.get()));

    let commit = move || {
        let v = edit_text.get_untracked();
        let trimmed = v.trim();
        if trimmed.is_empty() {
            // Empty title removes the row.
            if let Some(setter) = use_context::<RwSignal<Todos>>() {
                setter.update(|t| t.remove(id));
            }
        } else if trimmed != title.get_untracked() {
            title.set(trimmed.to_string());
        }
    };

    let on_key = move |ev: KeyEvent| {
        if ev.key == "Enter" {
            commit();
        } else if ev.key == "Escape" {
            // Revert: pull the canonical title back into the
            // editable buffer.
            edit_text.set(title.get_untracked());
        }
    };

    let parent_todos = use_context::<RwSignal<Todos>>().expect(
        "TodoRow must be used inside TodoMVC (which provides the \
         Todos context)",
    );

    view! {
        <hstack gap=8.0>
            <checkbox bind:checked=completed />
            <text_field
                bind:value=edit_text
                on:keydown=on_key
                on:change=move |_| commit()
                flex_grow=1.0
            />
            <button on:click=move |_| parent_todos.update(|t| t.remove(id))>
                "Delete"
            </button>
        </hstack>
    }
}

fn main() {
    mount_to_window("Todos", (520.0, 600.0), || {
        view! { <TodoMVC /> }
    })
}
