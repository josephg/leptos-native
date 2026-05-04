//! Counters — dynamic add/remove of counter rows. Native port of the
//! Leptos `counters` example.
//!
//! This is the stress test for `<For>` keyed iteration and dynamic
//! children — it exercises the `mount_before` / `try_mount_before`
//! code paths in tachys::renderer::gtk.

use leptos::prelude::*;

#[component]
fn Counters() -> impl IntoView {
    let counters = RwSignal::new(Vec::<(usize, RwSignal<i32>)>::new());
    let next_id = RwSignal::new(0_usize);

    let add = move || {
        let id = next_id.get_untracked();
        next_id.update(|n| *n += 1);
        let value = RwSignal::new(0);
        counters.update(move |cs| cs.push((id, value)));
    };

    let clear = move || counters.update(|cs| cs.clear());

    view! {
        <vstack padding=16.0 gap=8.0>
            <hstack gap=8.0>
                <button on:click=move |_| add()>"Add"</button>
                <button on:click=move |_| clear()>"Clear"</button>
            </hstack>
            <label>{move || {
                let total: i32 = counters
                    .with(|cs| cs.iter().map(|(_, v)| v.get()).sum());
                format!(
                    "Total: {} from {} counter(s)",
                    total,
                    counters.with(|cs| cs.len())
                )
            }}</label>
            <For
                each=move || counters.get()
                key=|(id, _)| *id
                children=move |(_id, value)| view! { <Row value/> }
            />
        </vstack>
    }
}

#[component]
fn Row(value: RwSignal<i32>) -> impl IntoView {
    view! {
        <hstack gap=8.0>
            <button on:click=move |_| value.update(|n| *n -= 1)>"-1"</button>
            <label>{move || value.get().to_string()}</label>
            <button on:click=move |_| value.update(|n| *n += 1)>"+1"</button>
        </hstack>
    }
}

fn main() {
    mount_to_window(
        "org.leptos.counters_gtk",
        "Counters — dynamic",
        (360, 520),
        || view! { <Counters /> },
    );
}
