//! Settings panel — exercises a wider range of NSControl-based inputs.
//!
//! Demonstrates: `<slider>`, `<checkbox>`, `<pop_up_button>`,
//! `<color_well>`, all driven by `bind:`. Plus a derived label that
//! reads several signals.
//!
//! Earliest stage that can run this: **Stage 5+** (after each control
//! kind has its `BindValue` impl — see implementation_log.md).
//!
//! Status: aspirational — won't compile yet.

use leptos::prelude::*;
use objc2_app_kit::NSColor;

#[component]
fn Settings() -> impl IntoView {
    let volume = RwSignal::new(50.0);
    let mute = RwSignal::new(false);
    let theme = RwSignal::new(0_usize);  // index into themes
    let accent = RwSignal::new(NSColor::system_blue());

    let themes = vec!["System", "Light", "Dark"];

    view! {
        <stack_view orientation="vertical" spacing=12.0>
            // --- Volume slider ---
            <stack_view orientation="vertical" spacing=4.0>
                <label>"Volume"</label>
                <slider
                    bind:value=volume
                    min_value=0.0
                    max_value=100.0
                    enabled={move || !mute.get()} />
                <label>{move || {
                    if mute.get() {
                        "Muted".to_string()
                    } else {
                        format!("{:.0}%", volume.get())
                    }
                }}</label>
            </stack_view>

            // --- Mute toggle ---
            <checkbox bind:state=mute>"Mute audio"</checkbox>

            // --- Theme picker ---
            <stack_view orientation="horizontal" spacing=8.0>
                <label>"Theme:"</label>
                <pop_up_button bind:selection=theme items=themes.clone() />
            </stack_view>

            // --- Accent color ---
            <stack_view orientation="horizontal" spacing=8.0>
                <label>"Accent color:"</label>
                <color_well bind:color=accent />
            </stack_view>
        </stack_view>
    }
}

fn main() {
    leptos::mount::mount_to_window(|| view! { <Settings /> });
}
