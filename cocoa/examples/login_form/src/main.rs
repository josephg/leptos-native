//! Login form — realistic use of `bind:` plus a derived `Memo`
//! gating the submit button via `enabled=`.

#[cfg(target_os = "macos")]
mod app {
    use leptos_native::prelude::*;

    #[component]
    pub fn LoginForm() -> impl IntoView {
        let username = RwSignal::new(String::new());
        let password = RwSignal::new(String::new());
        let remember = RwSignal::new(false);
        let status = RwSignal::new(String::new());

        let can_submit = Memo::new(move |_| {
            !username.get().is_empty() && password.get().len() >= 8
        });

        let on_submit = move |_| {
            // get_untracked: this is an event handler outside any
            // reactive tracking context — we just want to snapshot
            // the values at click time.
            status.set(format!(
                "Signed in as {} (remember={})",
                username.get_untracked(),
                remember.get_untracked()
            ));
        };

        view! {
            <vstack padding=16.0 gap=8.0>
                <label>"Sign in"</label>

                <text_field
                    bind:value=username
                    placeholder="Username" />

                <secure_text_field
                    bind:value=password
                    placeholder="Password (8+ chars)" />

                <checkbox bind:checked=remember>
                    "Remember me on this device"
                </checkbox>

                <button
                    enabled=move || can_submit.get()
                    on:click=on_submit>
                    "Sign in"
                </button>

                <label>{move || status.get()}</label>
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Login", (360.0, 260.0), || {
            view! { <LoginForm /> }
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
