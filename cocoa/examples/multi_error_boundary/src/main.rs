//! Two text fields, two parses, one `<ErrorBoundary>`. When
//! either parses fails, the boundary catches its error. When
//! both fail, the boundary shows both errors in its fallback —
//! the `Errors` map is keyed by an internal id and aggregates
//! every active error in the subtree.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;

    #[component]
    pub fn App() -> impl IntoView {
        let a = RwSignal::new(String::from("1"));
        let b = RwSignal::new(String::from("2"));

        view! {
            <vstack padding=20.0 gap=12.0>
                <label bold=true>"Two integers — type non-numbers to see two errors at once"</label>

                <hstack gap=8.0>
                    <text_field bind:value=a />
                    <text_field bind:value=b />
                </hstack>

                <ErrorBoundary fallback=|errors| {
                    let errors = errors.clone();
                    view! {
                        <vstack
                            padding=12.0
                            gap=4.0
                            background_color=Color::rgb(1.0, 0.94, 0.94)
                            corner_radius=8.0
                            clip=true>
                            <label bold=true text_color=Color::RED>"Errors:"</label>
                            <label>{move || {
                                errors.read()
                                    .iter()
                                    .map(|(_, e)| format!("• {}", e))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            }}</label>
                        </vstack>
                    }
                }>
                    <vstack gap=4.0>
                        <stack>{move || a.get().parse::<i32>().map(|n| format!("a parses to: {n}"))}</stack>
                        <stack>{move || b.get().parse::<i32>().map(|n| format!("b parses to: {n}"))}</stack>
                        <label text_color=Color::GRAY>
                            {move || {
                                match (a.get().parse::<i32>(), b.get().parse::<i32>()) {
                                    (Ok(x), Ok(y)) => format!("sum: {}", x + y),
                                    _ => String::new(),
                                }
                            }}
                        </label>
                    </vstack>
                </ErrorBoundary>
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Multi-error boundary", (480.0, 280.0), || {
            view! { <App /> }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
