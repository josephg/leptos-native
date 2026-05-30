//! Switch demo — `bind:checked` two-way binding on `<switch>`,
//! plus a `<slider>` with `bind:value`. Toggle the switch / drag
//! the slider; the labels reflect both directions of the wiring.

extern crate leptos_uikit as leptos_platform;

#[cfg(target_os = "ios")]
mod app {
    use leptos_platform::prelude::*;

    #[component]
    pub fn Demo() -> impl IntoView {
        let on = RwSignal::new(false);
        let volume = RwSignal::new(0.5_f64);

        view! {
            <vstack padding=20.0 gap=16.0>
                <hstack gap=12.0>
                    <label flex_grow=1.0>"Notifications"</label>
                    <switch bind:checked=on />
                </hstack>
                <label>{move || {
                    if on.get() {
                        "Notifications: ON".to_string()
                    } else {
                        "Notifications: off".to_string()
                    }
                }}</label>

                <label>"Volume"</label>
                <slider bind:value=volume min_value=0.0 max_value=1.0 />
                <label>{move || format!("{:.0}%", volume.get() * 100.0)}</label>

                <button on:click=move |_| {
                    on.update(|b| *b = !*b);
                    volume.set(0.5);
                }>"Reset"</button>
            </vstack>
        }
    }

    pub fn main() {
        leptos_platform::mount_ios::run(|| view! { <Demo /> });
    }

}

#[cfg(target_os = "ios")]
fn main() { app::main() }

#[cfg(not(target_os = "ios"))]
fn main() {}
