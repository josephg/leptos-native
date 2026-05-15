//! Native port of the upstream `timer` example. Demonstrates
//! `set_interval_with_handle` and the `use_interval` pattern that
//! cancels and re-schedules a timer when its interval signal
//! changes.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;
    use std::time::Duration;

    #[component]
    pub fn TimerDemo() -> impl IntoView {
        let count_a = RwSignal::new(0_i32);
        let count_b = RwSignal::new(0_i32);
        let interval = RwSignal::new(1000_u64);

        use_interval(1000_u64, move || {
            count_a.update(|c| *c += 1);
        });
        use_interval(interval, move || {
            count_b.update(|c| *c += 1);
        });

        view! {
            <vstack padding=16.0 gap=8.0>
                <label>"Count A (fixed 1000 ms)"</label>
                <label>{move || count_a.get().to_string()}</label>

                <label>{move || format!(
                    "Count B (dynamic, currently {} ms)",
                    interval.get()
                )}</label>
                <label>{move || count_b.get().to_string()}</label>

                <text_field
                    value=move || interval.get().to_string()
                    on:commit=move |s: String| {
                        if let Ok(v) = s.parse::<u64>() {
                            interval.set(v);
                        }
                    } />
            </vstack>
        }
    }

    /// Hook to wrap `set_interval_with_handle` and make it reactive
    /// w.r.t. interval changes.
    pub fn use_interval<T, F>(interval_millis: T, f: F)
    where
        F: Fn() + Clone + 'static,
        T: Into<Signal<u64>> + 'static,
    {
        let interval_millis = interval_millis.into();
        Effect::new(move |prev_handle: Option<IntervalHandle>| {
            if let Some(prev) = prev_handle {
                prev.clear();
            }
            let f = f.clone();
            set_interval_with_handle(
                move || f(),
                Duration::from_millis(interval_millis.get()),
            )
            .expect("could not create interval")
        });
    }

    pub fn main() {
        mount_to_window("Timer", (320.0, 240.0), || {
            view! { <TimerDemo /> }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
