//! A long form to demonstrate iOS keyboard avoidance. Wraps a
//! many-field form in a `<scroll_view>`. Tap any field; the
//! keyboard rises, the safe-area-aware root padding pushes
//! content up, and the scroll view's bounds shrink so you can
//! scroll the form into view.
//!
//! Auto-scroll-to-focused-field is not yet implemented in this
//! fork — you may need to scroll manually to bring the active
//! field above the keyboard.

extern crate leptos_uikit as leptos_platform;

#[cfg(target_os = "ios")]
mod app {
    use leptos_platform::prelude::*;

    #[component]
    pub fn App() -> impl IntoView {
        let first    = RwSignal::new(String::new());
        let last     = RwSignal::new(String::new());
        let email    = RwSignal::new(String::new());
        let phone    = RwSignal::new(String::new());
        let street   = RwSignal::new(String::new());
        let city     = RwSignal::new(String::new());
        let state    = RwSignal::new(String::new());
        let zip      = RwSignal::new(String::new());
        let country  = RwSignal::new(String::new());
        let notes    = RwSignal::new(String::new());

        view! {
            <vstack flex_grow=1.0>
                <scroll_view flex_grow=1.0>
                    <vstack padding=20.0 gap=12.0>
                        <label font_size=22.0>"Long form"</label>

                        <label>"First name"</label>
                        <text_field bind:value=first placeholder="Jane" />

                        <label>"Last name"</label>
                        <text_field bind:value=last placeholder="Doe" />

                        <label>"Email"</label>
                        <text_field bind:value=email placeholder="jane@example.com" />

                        <label>"Phone"</label>
                        <text_field bind:value=phone placeholder="+1 555 0100" />

                        <label>"Street address"</label>
                        <text_field bind:value=street />

                        <label>"City"</label>
                        <text_field bind:value=city />

                        <label>"State / Province"</label>
                        <text_field bind:value=state />

                        <label>"Zip / Postal"</label>
                        <text_field bind:value=zip />

                        <label>"Country"</label>
                        <text_field bind:value=country />

                        <label>"Notes"</label>
                        <text_view bind:value=notes min_height=120.0 />

                        <button>"Submit"</button>
                    </vstack>
                </scroll_view>
            </vstack>
        }
    }

    pub fn main() {
        leptos_platform::mount_ios::run(|| view! { <App /> });
    }
}

#[cfg(target_os = "ios")]
fn main() { app::main() }

#[cfg(not(target_os = "ios"))]
fn main() {}
