//! macOS port of `error_boundary` — type a number into the field;
//! a parse error triggers the `<ErrorBoundary/>` fallback.

use leptos::prelude::*;

#[component]
fn App() -> impl IntoView {
    // value is `Result<i32, ParseIntError>`. Parse errors flow into
    // the ErrorBoundary below.
    let text = RwSignal::new(String::from("0"));
    let value = move || text.get().parse::<i32>();

    view! {
        <vstack padding=20.0 gap=12.0>
            <label>{"Type an integer (or something that's not an integer)"}</label>
            <text_field bind:value=text />

            <ErrorBoundary fallback=|errors| {
                let errors = errors.clone();
                view! {
                    <vstack gap=4.0>
                        <label>{"Not an integer! Errors:"}</label>
                        <label>{move || {
                            errors.read()
                                .iter()
                                .map(|(_, e)| e.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        }}</label>
                    </vstack>
                }
            }>
                // Use <view> (a generic container, accepts arbitrary
                // Render children) rather than <label> here, because
                // <label> has a custom child setter that requires
                // String. Result<T, E>: Render is the mechanism that
                // lets ErrorBoundary catch the parse error.
                <view>
                    {move || value().map(|n| format!("You entered: {n}"))}
                </view>
            </ErrorBoundary>
        </vstack>
    }
}

fn main() {
    mount_to_window("Error Boundary", (380.0, 220.0), || {
        view! { <App /> }
    });
}
