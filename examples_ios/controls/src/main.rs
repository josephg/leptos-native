//! Controls showcase — exercises every iOS builder ported so far:
//! `<button>`, `<label>`, `<text_field>`, `<secure_text_field>`,
//! `<switch>`, `<slider>`, `<stepper>`, `<segmented_control>`,
//! `<progress_indicator>`, `<text_view>`, `<scroll_view>`.
//!
//! `<image_view>` and `<date_picker>` are exercised by their own
//! examples (image needs a bundled asset; date_picker is its own
//! interactive thing).

use leptos::prelude::*;

#[component]
fn Showcase() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let notifications = RwSignal::new(false);
    let volume = RwSignal::new(0.5_f64);
    let count = RwSignal::new(5.0_f64);
    let style_idx = RwSignal::new(0_usize);
    let notes = RwSignal::new("Multi-line notes here.".to_string());

    view! {
        <scroll_view flex_grow=1.0>
            <vstack padding=20.0 gap=16.0>
                <label font_size=24.0>{"Controls demo"}</label>

                // text fields
                <label>{"Name"}</label>
                <text_field bind:value=name placeholder="Enter your name" />
                <label>{move || format!("Hello, {}!", {
                    let n = name.get();
                    if n.is_empty() { "stranger".to_string() } else { n }
                })}</label>

                <label>{"Password"}</label>
                <secure_text_field bind:value=password placeholder="••••••" />
                <label>{move || format!("Password length: {}", password.get().len())}</label>

                // switch
                <hstack gap=12.0>
                    <label flex_grow=1.0>{"Notifications"}</label>
                    <switch bind:checked=notifications />
                </hstack>

                // slider + progress
                <label>{move || format!("Volume: {:.0}%", volume.get() * 100.0)}</label>
                <slider bind:value=volume min_value=0.0 max_value=1.0 />
                <progress_indicator value=move || volume.get() />

                // stepper
                <hstack gap=12.0>
                    <label flex_grow=1.0>{move || format!("Count: {:.0}", count.get())}</label>
                    <stepper bind:value=count min_value=0.0 max_value=99.0 increment=1.0 />
                </hstack>

                // segmented control
                <label>{"Pick a style"}</label>
                <segmented_control
                    items=vec!["Cozy", "Standard", "Spacious"]
                    bind:selection=style_idx
                />
                <label>{move || format!("Style: {}", match style_idx.get() {
                    0 => "Cozy",
                    1 => "Standard",
                    _ => "Spacious",
                })}</label>

                // multi-line text view
                <label>{"Notes"}</label>
                <text_view bind:value=notes />
                <label>{move || format!("{} chars", notes.get().len())}</label>

                <button on:click=move |_| {
                    name.set(String::new());
                    password.set(String::new());
                    notifications.set(false);
                    volume.set(0.5);
                    count.set(5.0);
                    style_idx.set(0);
                    notes.set("Multi-line notes here.".to_string());
                }>"Reset everything"</button>
            </vstack>
        </scroll_view>
    }
}

fn main() {
    leptos::mount_ios::run(|| view! { <Showcase /> });
}
