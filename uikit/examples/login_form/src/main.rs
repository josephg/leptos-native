//! Login form — `bind:value` on text fields, `bind:checked` on a
//! switch, and a `Memo`-gated submit button via `enabled=`. Also
//! exercises the on-screen keyboard avoidance: the form sits at
//! the top, but as soon as the keyboard slides up the bottom
//! padding kicks in so the submit button stays visible.

extern crate leptos_uikit as leptos_platform;

#[cfg(target_os = "ios")]
mod app {
    use leptos_platform::prelude::*;

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
            status.set(format!(
                "Signed in as {} (remember={})",
                username.get_untracked(),
                remember.get_untracked()
            ));
        };

        view! {
            <vstack padding=16.0 gap=12.0>
                <label font_size=24.0>"Sign in"</label>

                <text_field
                    bind:value=username
                    placeholder="Username" />

                <secure_text_field
                    bind:value=password
                    placeholder="Password (8+ chars)" />

                // UISwitch has no title — pair with a label.
                <hstack gap=12.0>
                    <label flex_grow=1.0>"Remember me on this device"</label>
                    <switch bind:checked=remember />
                </hstack>

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
        leptos_platform::mount_ios::run(|| view! { <LoginForm /> });
    }

}

#[cfg(target_os = "ios")]
fn main() { app::main() }

#[cfg(not(target_os = "ios"))]
fn main() {}
