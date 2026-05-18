//! Counters — dynamic add/remove of counter rows. Native port of the
//! Leptos `counters` example, stripped to the controls we have on
//! macOS.
//!
//! This is the stress test for `<For>` keyed iteration and dynamic
//! children — it'll exercise the `mount_before` / `try_mount_before`
//! code paths in tachys::renderer::cocoa that have so far been
//! unexercised.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;

    #[component]
    pub fn Counters() -> impl IntoView {
        let counters = RwSignal::new(Vec::<(usize, RwSignal<i32>)>::new());
        let next_id = RwSignal::new(0_usize);

        let add = move || {
            let id = next_id.get_untracked();
            next_id.update(|n| *n += 1);
            let value = RwSignal::new(0);
            counters.update(move |cs| cs.push((id, value)));
        };

        let clear = move || counters.update(|cs| cs.clear());

        // Pseudo-random shuffle. Reorders the underlying Vec — exercises
        // the `<For>` keyed-diff *move* path (vs. the append/clear paths
        // that Add/Clear hit).
        let shuffle = move || {
            counters.update(|cs| {
                let len = cs.len();
                if len < 2 {
                    return;
                }
                // xorshift seeded off the current contents — deterministic
                // per state, no extra deps.
                let mut seed: u64 = cs.iter().map(|(id, _)| *id as u64 + 1).sum();
                for i in (1..len).rev() {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    let j = (seed as usize) % (i + 1);
                    cs.swap(i, j);
                }
            });
        };

        view! {
            <vstack padding=16.0 gap=8.0>
                <hstack gap=8.0 align=AlignItems::Center>
                    <button on:click=move |_| add()>"Add"</button>
                    <button on:click=move |_| shuffle()>"Shuffle"</button>
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
    pub fn Row(value: RwSignal<i32>) -> impl IntoView {
        view! {
            <hstack gap=8.0 align=AlignItems::Baseline>
                <button on:click=move |_| value.update(|n| *n -= 1)>"-1"</button>
                <label>{move || value.get().to_string()}</label>
                <button on:click=move |_| value.update(|n| *n += 1)>"+1"</button>
            </hstack>
        }
    }

    pub fn main() {
        mount_to_window("Counters — dynamic", (360.0, 480.0), || {
            view! { <Counters /> }
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
