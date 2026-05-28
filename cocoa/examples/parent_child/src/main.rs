//! macOS port of `parent_child` — demonstrates four ways
//! child components communicate with their parent:
//!   1. `<ButtonA/>`: WriteSignal prop
//!   2. `<ButtonB/>`: closure prop
//!   3. `<ButtonC/>`: `on:click` on the component itself
//!      (the Tier 2.F feature)
//!   4. `<ButtonD/>`: provide_context / use_context

extern crate leptos_cocoa as leptos_platform;

#[cfg(target_os = "macos")]
mod app {
    use leptos_platform::prelude::*;

    #[derive(Copy, Clone)]
    pub struct ToggleContext(WriteSignal<bool>);

    #[component]
    pub fn App() -> impl IntoView {
        let (red, set_red) = signal(false);
        let (green, set_green) = signal(false);
        let (blue, set_blue) = signal(false);
        let (cyan, set_cyan) = signal(false);

        provide_context(ToggleContext(set_cyan));

        view! {
            <vstack padding=16.0 gap=8.0>
                <label>{move || format!(
                    "Red: {}  Green: {}  Blue: {}  Cyan: {}",
                    red.get(), green.get(), blue.get(), cyan.get()
                )}</label>

                // 1. Pass a WriteSignal as a prop
                <ButtonA setter=set_red />

                // 2. Pass a closure as a prop
                <ButtonB on_click=move |_| set_green.update(|v| *v = !*v) />

                // 3. on:click on the component itself
                <ButtonC on:click=move |_| set_blue.update(|v| *v = !*v) />

                // 4. Context-based setter
                <ButtonD />
            </vstack>
        }
    }

    #[component]
    pub fn ButtonA(setter: WriteSignal<bool>) -> impl IntoView {
        view! { <button on:click=move |_| setter.update(|v| *v = !*v)>"Toggle Red"</button> }
    }

    #[component]
    pub fn ButtonB(on_click: impl FnMut(()) + Send + 'static) -> impl IntoView {
        view! { <button on:click=on_click>"Toggle Green"</button> }
    }

    #[component]
    pub fn ButtonC() -> impl IntoView {
        view! { <button>"Toggle Blue"</button> }
    }

    #[component]
    pub fn ButtonD() -> impl IntoView {
        let setter = use_context::<ToggleContext>().unwrap().0;
        view! {
            <button on:click=move |_| setter.update(|v| *v = !*v)>
                "Toggle Cyan"
            </button>
        }
    }

    pub fn main() {
        mount_to_window("Parent-Child communication", (380.0, 260.0), || {
            view! { <App /> }
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
