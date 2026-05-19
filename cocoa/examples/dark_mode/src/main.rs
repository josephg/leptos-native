//! Manually-driven dark mode using reactive `background_color`
//! and `text_color`. Toggle the checkbox to swap the palette.
//!
//! Until first-party system-colour constants ship (tracked
//! separately), this is the working pattern for theme-aware
//! Cocoa apps. The same shape applies on iOS, with that port's
//! `Color::SYSTEM_BACKGROUND` / `LABEL` constants you can swap
//! in directly.

#[cfg(target_os = "macos")]
mod app {
    use leptos_native::prelude::*;

    #[derive(Copy, Clone)]
    struct Palette {
        background: Color,
        surface:    Color,
        text:       Color,
        subtle:     Color,
        accent:     Color,
    }

    const LIGHT: Palette = Palette {
        background: Color::Rgba { r: 0.96, g: 0.96, b: 0.97, a: 1.0 },
        surface:    Color::WHITE,
        text:       Color::Rgba { r: 0.10, g: 0.10, b: 0.12, a: 1.0 },
        subtle:     Color::GRAY,
        accent:     Color::Rgba { r: 0.20, g: 0.50, b: 0.90, a: 1.0 },
    };

    const DARK: Palette = Palette {
        background: Color::Rgba { r: 0.10, g: 0.10, b: 0.12, a: 1.0 },
        surface:    Color::Rgba { r: 0.18, g: 0.18, b: 0.21, a: 1.0 },
        text:       Color::Rgba { r: 0.95, g: 0.95, b: 0.97, a: 1.0 },
        subtle:     Color::Rgba { r: 0.65, g: 0.65, b: 0.70, a: 1.0 },
        accent:     Color::Rgba { r: 0.45, g: 0.75, b: 1.00, a: 1.0 },
    };

    #[component]
    pub fn App() -> impl IntoView {
        let dark = RwSignal::new(false);
        let p = move || if dark.get() { DARK } else { LIGHT };

        view! {
            <vstack
                padding=20.0
                gap=12.0
                background_color=move || p().background
                flex_grow=1.0>

                <hstack gap=12.0 align=AlignItems::Center>
                    <label
                        bold=true
                        font_size=18.0
                        text_color=move || p().text>
                        "Dark mode demo"
                    </label>
                    <stack flex_grow=1.0 />
                    <checkbox bind:checked=dark text_color=move || p().text>
                        "Dark"
                    </checkbox>
                </hstack>

                <vstack
                    background_color=move || p().surface
                    corner_radius=8.0
                    overflow=Overflow::Clip
                    padding=16.0
                    gap=8.0>
                    <label text_color=move || p().text bold=true>
                        "Card heading"
                    </label>
                    <label text_color=move || p().subtle>
                        "Subtitle in the muted ink colour."
                    </label>
                    <hstack gap=8.0>
                        <button text_color=move || p().accent>
                            "Primary action"
                        </button>
                        <button text_color=move || p().subtle>
                            "Secondary"
                        </button>
                    </hstack>
                </vstack>

                <stack flex_grow=1.0 />

                <label text_color=move || p().subtle>
                    "Toggle the checkbox to swap palettes."
                </label>
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Dark mode", (480.0, 320.0), || {
            view! { <App /> }
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
