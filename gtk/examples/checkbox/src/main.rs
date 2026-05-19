//! Demonstrates `<checkbox bind:checked=...>`, `on:input`, and
//! `on:change` working together on the same `<text_field>`.
//!
//! - The checkbox's state two-way binds to a `bool` signal; toggling
//!   it updates the displayed status.
//! - The text field uses `bind:value` for two-way state AND
//!   `on:input` to count keystrokes AND `on:commit` to record the
//!   most recently committed value (return key). This verifies all
//!   three coexist on the same entry.

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
                <label>"Newsletter signup"</label>

                <text_field
                    bind:value=email
                    on:input=move |_v: String| {
                        keystroke_count.update(|c| *c += 1);
                    }
                    on:commit=move |v: String| {
                        last_committed.set(v);
                    } />

                <label>{move || format!("Keystrokes: {}", keystroke_count.get())}</label>
                <label>{move || format!(
                    "Last committed (return): \"{}\"",
                    last_committed.get()
                )}</label>

                <checkbox bind:checked=subscribe>
                    "Subscribe to weekly updates"
                </checkbox>

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
        mount_to_window(
            "org.leptos.checkbox_gtk",
            "Checkbox + events demo",
            (420, 320),
            || view! { <Demo /> },
        );
    }
}

fn main() { app::main() }
