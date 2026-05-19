//! `bind:value=(getter, setter)` — two-way binding with a
//! transform applied on the way back. Type into the email field;
//! the stored value is always trimmed and lowercased. The
//! displayed field still shows what you literally typed (and
//! commits the transformed value on Return / blur).
//!
//! Compare against the simpler `bind:value=signal` pattern from
//! the `greeter` example — this is what you reach for when one
//! direction needs filtering.

#[cfg(target_os = "macos")]
mod app {
    use leptos_native::prelude::*;

    #[component]
    pub fn App() -> impl IntoView {
        let email = RwSignal::new(String::new());

        view! {
            <vstack padding=20.0 gap=12.0>
                <label bold=true>"Email"</label>
                <text_field
                    bind:value=(
                        move || email.get(),
                        move |v: String| {
                            email.set(v.trim().to_lowercase());
                        },
                    )
                    placeholder="USER@example.com" />

                <label text_color=Color::GRAY>
                    {move || format!("stored as: {:?}", email.get())}
                </label>
                <label text_color=Color::GRAY>
                    "(Whitespace trimmed, case lowered on commit.)"
                </label>
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("bind tuple", (380.0, 200.0), || {
            view! { <App /> }
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
