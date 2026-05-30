//! Settings panel — slider, switch, and segmented_control all
//! driven by `bind:`. The slider's `enabled=` is gated by the mute
//! switch (a derived `enabled=move || !mute.get()` closure).

extern crate leptos_uikit as leptos_platform;

#[cfg(target_os = "ios")]
mod app {
    use leptos_platform::prelude::*;

    #[component]
    pub fn Settings() -> impl IntoView {
        let volume = RwSignal::new(50.0_f64);
        let mute = RwSignal::new(false);
        let theme = RwSignal::new(0_usize);

        view! {
            <vstack padding=20.0 gap=16.0>
                <label font_size=24.0>"Settings"</label>

                // --- Volume slider ---
                <vstack gap=6.0>
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
                <hstack gap=12.0>
                    <label flex_grow=1.0>"Mute audio"</label>
                    <switch bind:checked=mute />
                </hstack>

                // --- Theme picker ---
                <vstack gap=6.0>
                    <label>"Theme"</label>
                    <segmented_control
                        items=vec!["System", "Light", "Dark"]
                        bind:selection=theme />
                </vstack>

                <label>{move || {
                    let names = ["System", "Light", "Dark"];
                    let idx = theme.get().min(names.len() - 1);
                    format!("Selected theme: {}", names[idx])
                }}</label>
            </vstack>
        }
    }

    pub fn main() {
        leptos_platform::mount_ios::run(|| view! { <Settings /> });
    }

}

#[cfg(target_os = "ios")]
fn main() { app::main() }

#[cfg(not(target_os = "ios"))]
fn main() {}
