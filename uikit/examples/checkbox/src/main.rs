//! Demonstrates `<switch bind:checked=...>` plus `on:input` +
//! `on:change` + `bind:value` all coexisting on a single
//! `<text_field>`.
//!
//! - The switch two-way binds to a `bool` signal.
//! - The text field uses `bind:value` for two-way state AND
//!   `on:input` to count keystrokes AND `on:commit` to record the
//!   most recently committed value (return key / focus loss).
//!   This verifies all three coexist on the same field — they
//!   share one fan-out delegate.

#[cfg(target_os = "ios")]
mod app {
    use leptos_native::prelude::*;

    #[component]
    pub fn Demo() -> impl IntoView {
        let subscribe = RwSignal::new(false);
        let email = RwSignal::new(String::new());
        let keystroke_count = RwSignal::new(0_u32);
        let last_committed = RwSignal::new(String::new());

        view! {
            <vstack padding=16.0 gap=12.0>
                <label font_size=22.0>"Newsletter signup"</label>

                <text_field
                    bind:value=email
                    placeholder="you@example.com"
                    on:input=move |_v: String| {
                        keystroke_count.update(|c| *c += 1);
                    }
                    on:commit=move |v: String| {
                        last_committed.set(v);
                    } />

                <label>{move || format!(
                    "Keystrokes: {}",
                    keystroke_count.get(),
                )}</label>
                <label>{move || format!(
                    "Last committed (return / blur): \"{}\"",
                    last_committed.get(),
                )}</label>

                <hstack gap=12.0>
                    <label flex_grow=1.0>"Subscribe to weekly updates"</label>
                    <switch bind:checked=subscribe />
                </hstack>

                <label>{move || {
                    if subscribe.get() {
                        format!("\u{2713} {} will receive updates.", email.get())
                    } else {
                        "Not subscribed.".to_string()
                    }
                }}</label>
            </vstack>
        }
    }

    pub fn main() {
        leptos_native::mount_ios::run(|| view! { <Demo /> });
    }

}

#[cfg(target_os = "ios")]
fn main() { app::main() }

#[cfg(not(target_os = "ios"))]
fn main() {}
