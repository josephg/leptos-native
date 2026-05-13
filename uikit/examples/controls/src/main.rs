//! Controls showcase — exercises every iOS builder ported so far:
//! `<button>`, `<label>`, `<text_field>`, `<secure_text_field>`,
//! `<switch>`, `<slider>`, `<stepper>`, `<segmented_control>`,
//! `<progress_indicator>`, `<text_view>`, `<scroll_view>`.
//!
//! `<image_view>` and `<date_picker>` are exercised by their own
//! examples (image needs a bundled asset; date_picker is its own
//! interactive thing).

#[cfg(target_os = "ios")]
mod app {
    use leptos::prelude::*;

    #[component]
    pub fn Showcase() -> impl IntoView {
        let name = RwSignal::new(String::new());
        let password = RwSignal::new(String::new());
        let notifications = RwSignal::new(false);
        let volume = RwSignal::new(0.5_f64);
        let count = RwSignal::new(5.0_f64);
        let style_idx = RwSignal::new(0_usize);
        let notes = RwSignal::new("Multi-line notes here.".to_string());

        let taps = RwSignal::new(0_u32);

        view! {
            <scroll_view flex_grow=1.0>
                <vstack padding=20.0 gap=16.0>
                    // Tappable label — exercises the
                    // `UITapGestureRecognizer` fallback in
                    // `Element::on_click` for non-UIControl views.
                    <label
                        font_size=24.0
                        text_color=Color::SYSTEM_BLUE
                        on:click=move |_| taps.update(|n| *n += 1)>
                        {move || format!(
                            "Controls demo — tap me ({} taps)",
                            taps.get(),
                        )}
                    </label>

                    // (Photo-cell demo removed — `aspect_ratio` +
                    // default `align-items: stretch` made the cell
                    // consume the row's full width and push the title
                    // off-screen. Real photo grids will set explicit
                    // `width=N` + `height=N` per cell once those
                    // builder methods land, OR pass through Taffy's
                    // grid layout once the `grid` feature is enabled.)

                    // Chrome demo — a card with rounded corners, fill,
                    // and a coloured outline. Exercises the new
                    // `background_color`/`corner_radius`/`border_width`/
                    // `border_color` builder attributes.
                    //
                    // Note: use `<vstack>` (or `<hstack>`) for chrome
                    // attributes, not `<view>` — the leptos macro lists
                    // "view" as an SVG element, so attrs on `<view>`
                    // route through `.attr(name, value)` instead of
                    // typed builder methods.
                    <vstack
                        padding=12.0
                        gap=4.0
                        corner_radius=16.0
                        background_color=Color::SECONDARY_LABEL
                        border_width=1.5
                        border_color=Color::SYSTEM_BLUE>
                        <label text_color=Color::SYSTEM_BACKGROUND>
                            "Sync pellet"
                        </label>
                        <label
                            font_size=12.0
                            text_color=Color::SYSTEM_BACKGROUND>
                            "12 unsynced photos · tap to sync"
                        </label>
                    </vstack>

                    // text fields
                    <label>"Name"</label>
                    <text_field bind:value=name placeholder="Enter your name" />
                    <label>{move || format!("Hello, {}!", {
                        let n = name.get();
                        if n.is_empty() { "stranger".to_string() } else { n }
                    })}</label>

                    <label>"Password"</label>
                    <secure_text_field bind:value=password placeholder="••••••" />
                    <label>{move || format!("Password length: {}", password.get().len())}</label>

                    // switch
                    <hstack gap=12.0>
                        <label flex_grow=1.0>"Notifications"</label>
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
                    <label>"Pick a style"</label>
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
                    <label>"Notes"</label>
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

    pub fn main() {
        leptos::mount_ios::run(|| view! { <Showcase /> });
    }

}

#[cfg(target_os = "ios")]
fn main() { app::main() }

#[cfg(not(target_os = "ios"))]
fn main() {}
