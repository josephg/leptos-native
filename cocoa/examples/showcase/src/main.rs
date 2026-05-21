//! Showcase — every control the macOS port currently supports,
//! laid out in a single scrolling panel. Each row pairs an
//! interactive control with a label that displays its current
//! value, demonstrating the `bind:` / `value=` reactivity.

#[cfg(target_os = "macos")]
mod app {
    use leptos_native::prelude::*;
    use leptos_native::core::children::TypedChildrenFn;
    use std::marker::PhantomData;

    // Section takes typed children. With this fork's no-AnyView design,
    // the untyped `Children = Box<dyn FnOnce() -> AnyView>` from upstream
    // isn't available — components carry the concrete child view type as
    // a generic parameter instead. The Renderer is pinned to Dom (cocoa)
    // because the body uses cocoa-specific builders (vstack, label).
    #[component]
    pub fn Section<C>(
        title: &'static str,
        children: TypedChildrenFn<C, CocoaDom>,
        #[prop(optional)] _marker: PhantomData<C>,
    ) -> impl IntoView
    where
        C: IntoView + 'static,
    {
        let children = children.into_inner();
        view! {
            <vstack gap=6.0>
                <label>{title}</label>
                {children()}
            </vstack>
        }
    }

    #[component]
    pub fn App() -> impl IntoView {
        // Per-control state.
        let name = RwSignal::new(String::from("World"));
        let password = RwSignal::new(String::new());
        let agreed = RwSignal::new(false);
        let volume = RwSignal::new(0.42_f64);
        let priority = RwSignal::new(1_usize);
        let theme = RwSignal::new(0_usize);
        let color = RwSignal::new(Color::rgb(0.2, 0.6, 1.0));
        let date = RwSignal::new(Date::now());
        let count = RwSignal::new(5.0_f64);
        let progress = RwSignal::new(0.35_f64);
        let notes = RwSignal::new(
            "Multi-line plain text. Try editing!".to_string(),
        );

        let priorities = vec!["Low", "Normal", "High", "Critical"];
        let themes = vec!["Light", "Dark", "Auto"];

        view! {
            <scroll_view flex_grow=1.0>
                <vstack padding=20.0 gap=20.0>
                    <label>"Leptos macOS — control showcase"</label>

                    <Section title="Buttons">
                        <hstack gap=8.0>
                            <button on:click=move |_| name.set(String::new())>"Reset name"</button>
                            <button enabled=move || !name.get().is_empty()>"Greet"</button>
                            <button enabled=false>"Disabled"</button>
                        </hstack>
                    </Section>

                    <Section title="Text input — bind:value">
                        <text_field bind:value=name placeholder="Your name" />
                        <label>{move || format!("Hello, {}!", name.get())}</label>
                    </Section>

                    <Section title="Secure text — bind:value">
                        <secure_text_field bind:value=password placeholder="Password" />
                        <label>{move || format!("({} chars)", password.get().len())}</label>
                    </Section>

                    <Section title="Checkbox — bind:checked">
                        <checkbox bind:checked=agreed>"I agree to the terms"</checkbox>
                        <label>{move || if agreed.get() { "✓ agreed".to_string() } else { "not agreed".to_string() }}</label>
                    </Section>

                    <Section title="Slider — bind:value">
                        <slider bind:value=volume min_value=0.0 max_value=1.0 />
                        <label>{move || format!("Volume: {:.0}%", volume.get() * 100.0)}</label>
                    </Section>

                    <Section title="Pop-up button — bind:value (usize)">
                        <pop_up_button items=priorities.clone() bind:value=priority />
                        <label>{move || format!("Priority idx: {}", priority.get())}</label>
                    </Section>

                    <Section title="Segmented control — bind:value (usize)">
                        <segmented_control items=themes.clone() bind:value=theme />
                        <label>{move || format!("Theme idx: {}", theme.get())}</label>
                    </Section>

                    <Section title="Color well — bind:value">
                        <color_well bind:value=color />
                        <label>{move || {
                            match color.get() {
                                Color::Rgba { r, g, b, .. } => format!(
                                    "rgb({:.0}, {:.0}, {:.0})",
                                    r * 255.0,
                                    g * 255.0,
                                    b * 255.0,
                                ),
                                Color::System(_) => "(system color)".to_string(),
                            }
                        }}</label>
                    </Section>

                    <Section title="Date picker — bind:value">
                        <date_picker bind:value=date />
                        <label>{move || {
                            format!(
                                "Unix secs: {:.0}",
                                date.get().seconds_since_epoch,
                            )
                        }}</label>
                    </Section>

                    <Section title="Stepper — bind:value">
                        <stepper
                            bind:value=count
                            min_value=0.0
                            max_value=20.0
                            increment=1.0
                        />
                        <label>{move || format!("Count: {:.0}", count.get())}</label>
                    </Section>

                    <Section title="Progress indicator (determinate)">
                        <progress_indicator value=move || progress.get() max_value=1.0 />
                        <hstack gap=8.0>
                            <button on:click=move |_| progress.update(|p| *p = (*p - 0.1).max(0.0))>"-10%"</button>
                            <button on:click=move |_| progress.update(|p| *p = (*p + 0.1).min(1.0))>"+10%"</button>
                            <label>{move || format!("{:.0}%", progress.get() * 100.0)}</label>
                        </hstack>
                    </Section>

                    <Section title="Progress indicator (indeterminate spinner)">
                        <progress_indicator indeterminate=true />
                    </Section>

                    <Section title="Multi-line text — bind:value">
                        <text_view bind:value=notes flex_grow=1.0 />
                        <label>{move || format!("{} chars", notes.get().len())}</label>
                    </Section>
                </vstack>
            </scroll_view>
        }
    }

    pub fn main() {
        mount_to_window("Showcase", (520.0, 720.0), || {
            view! { <App /> }
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
