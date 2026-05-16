//! Demonstrates `<Show fallback=...>` — render one of two
//! branches depending on a boolean. Toggle the checkbox; the
//! main label flips between the children and the fallback.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;

    #[component]
    pub fn App() -> impl IntoView {
        let signed_in = RwSignal::new(false);

        view! {
            <vstack padding=20.0 gap=16.0>
                <checkbox bind:checked=signed_in>
                    "Signed in"
                </checkbox>

                <Show
                    when=move || signed_in.get()
                    fallback=|| view! {
                        <vstack
                            padding=12.0
                            gap=4.0
                            background_color=Color::rgb(0.98, 0.92, 0.85)
                            corner_radius=8.0
                            overflow=Overflow::Clip>
                            <label bold=true>"Please sign in."</label>
                            <label>"Tick the checkbox above to continue."</label>
                        </vstack>
                    }>
                    <vstack
                        padding=12.0
                        gap=4.0
                        background_color=Color::rgb(0.85, 0.95, 0.85)
                        corner_radius=8.0
                        overflow=Overflow::Clip>
                        <label bold=true>"Welcome back."</label>
                        <label>"You can see this view because you're signed in."</label>
                    </vstack>
                </Show>
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Show fallback", (380.0, 220.0), || {
            view! { <App /> }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
