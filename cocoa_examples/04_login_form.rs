//! Login form — a realistic use of `bind:` plus a derived `Memo`
//! gating the submit button.
//!
//! Demonstrates: `<text_field>`, `<secure_text_field>`, `<checkbox>`
//! all bound to signals; a `Memo` that derives whether the form is
//! valid; the `enabled` attribute on `<button>` driven by a closure.
//!
//! Earliest stage that can run this: **Stage 5+** (after `bind:`).
//!
//! Status: aspirational — won't compile yet.

use leptos::prelude::*;

#[component]
fn LoginForm() -> impl IntoView {
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let remember = RwSignal::new(false);

    let can_submit = Memo::new(move |_| {
        !username.get().is_empty() && password.get().len() >= 8
    });

    let on_submit = move |_| {
        sign_in(username.get(), password.get(), remember.get());
    };

    view! {
        <stack_view orientation="vertical" spacing=8.0>
            <text_field
                bind:value=username
                placeholder="Username" />
            <secure_text_field
                bind:value=password
                placeholder="Password (8+ chars)" />
            <checkbox bind:state=remember>"Remember me on this device"</checkbox>

            <button
                enabled={move || can_submit.get()}
                on:click=on_submit>
                "Sign in"
            </button>
        </stack_view>
    }
}

fn sign_in(_username: String, _password: String, _remember: bool) {
    // would call out to your auth system
}

fn main() {
    leptos::mount::mount_to_window(|| view! { <LoginForm /> });
}
