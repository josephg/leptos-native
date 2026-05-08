//! Timer — `set_interval_with_handle` + a `use_interval` hook that
//! cancels and re-schedules when its interval signal changes.
//! Sanity test for the iOS spawner (NSTimer-driven).

use leptos::prelude::*;
use std::time::Duration;

#[component]
fn TimerDemo() -> impl IntoView {
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
            <label font_size=20.0>{"Count A (fixed 1000 ms)"}</label>
            <label font_size=32.0>{move || count_a.get().to_string()}</label>

            <label font_size=20.0>{move || format!(
                "Count B (dynamic, currently {} ms)",
                interval.get()
            )}</label>
            <label font_size=32.0>{move || count_b.get().to_string()}</label>

            // Stepper drives the interval in 100ms increments.
            // Bind layer is `f64` (UIStepper.value) — convert in
            // both directions. Doing this with a text field would
            // mean the change only commits on blur/return, which is
            // awkward on iOS where dismissing the keyboard isn't
            // discoverable; the stepper is instant.
            <hstack gap=12.0>
                <label flex_grow=1.0>{"Interval (ms)"}</label>
                <stepper
                    bind:value=(
                        move || interval.get() as f64,
                        move |v: f64| interval.set(v as u64),
                    )
                    min_value=100.0
                    max_value=5000.0
                    increment=100.0 />
            </hstack>
        </vstack>
    }
}

/// Hook wrapping `set_interval_with_handle`. Reactive in the
/// interval signal — re-schedules whenever it changes.
fn use_interval<T, F>(interval_millis: T, f: F)
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

fn main() {
    leptos::mount_ios::run(|| view! { <TimerDemo /> });
}
