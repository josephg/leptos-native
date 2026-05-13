//! Settings panel — exercises slider, checkbox, and pop_up_button
//! all driven by `bind:`. Plus a derived label that reads multiple
//! signals and a slider whose `enabled=` is gated by the mute
//! checkbox.
//!
//! (color_well is deferred — needs an NSColor signal type and its
//! own bind impl.)

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;

    #[component]
    pub fn Settings() -> impl IntoView {
        let volume = RwSignal::new(50.0_f64);
        let mute = RwSignal::new(false);
        let theme = RwSignal::new(0_usize);

        view! {
            <vstack padding=16.0 gap=12.0>
                // --- Volume slider ---
                <vstack gap=4.0>
                    <label>"Volume"</label>
                    <slider
                        bind:value=volume
                        min_value=0.0
                        max_value=100.0
                        enabled=move || !mute.get() />
                    <label>{move || {
                        if mute.get() {
                            "Muted".to_string()
                        } else {
                            format!("{:.0}%", volume.get())
                        }
                    }}</label>
                </vstack>

                // --- Mute toggle ---
                <checkbox bind:checked=mute>"Mute audio"</checkbox>

                // --- Theme picker ---
                <hstack gap=8.0>
                    <label>"Theme:"</label>
                    <pop_up_button
                        items=vec!["System", "Light", "Dark"]
                        bind:selection=theme />
                </hstack>

                <label>{move || {
                    let names = ["System", "Light", "Dark"];
                    let idx = theme.get().min(names.len() - 1);
                    format!("Selected theme: {}", names[idx])
                }}</label>
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Settings", (380.0, 300.0), || {
            view! { <Settings /> }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
