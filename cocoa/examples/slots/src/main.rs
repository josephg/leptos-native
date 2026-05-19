//! macOS port of `slots` — demonstrates the `#[slot]` attribute
//! for creating named child slots in components.

use leptos_native::prelude::*;

#[slot]
struct Then {
    children: ChildrenFn,
}

#[slot]
struct ElseIf {
    cond: Signal<bool>,
    children: ChildrenFn,
}

#[slot]
struct Fallback {
    children: ChildrenFn,
}

#[component]
fn SlotIf(
    cond: Signal<bool>,
    then: Then,
    #[prop(default=vec![])] else_if: Vec<ElseIf>,
    #[prop(optional)] fallback: Option<Fallback>,
) -> impl IntoView {
    move || {
        if cond.get() {
            (then.children)().into_any()
        } else if let Some(ei) =
            else_if.iter().find(|i| i.cond.get())
        {
            (ei.children)().into_any()
        } else if let Some(fb) = &fallback {
            (fb.children)().into_any()
        } else {
            ().into_any()
        }
    }
}

#[component]
fn App() -> impl IntoView {
    let (count, set_count) = signal(0_i32);
    let is_even = Signal::derive(move || count.get() % 2 == 0);
    let is_div5 = Signal::derive(move || count.get() % 5 == 0);

    view! {
        <vstack padding=16.0 gap=8.0>
            <button on:click=move |_| set_count.update(|n| *n += 1)>"+1"</button>
            <label>{move || format!("{} is", count.get())}</label>
            <SlotIf cond=is_even>
                <Then slot>"even"</Then>
                <ElseIf slot cond=is_div5>"divisible by 5"</ElseIf>
                <Fallback slot>"odd"</Fallback>
            </SlotIf>
        </vstack>
    }
}

fn main() {
    mount_to_window("Slots", (320.0, 180.0), || {
        view! { <App /> }
    }).run();
}
