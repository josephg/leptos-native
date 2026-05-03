//! Side-by-side: the same form written with `bind:` and without.
//!
//! Shows what the `bind:` infrastructure saves you — the manual version
//! needs an explicit `NodeRef` per field, an `Effect` to push signal
//! changes back into the control, and an `on:input` handler to push
//! user changes back into the signal.
//!
//! Earliest stage that can run the `bind:` version: **Stage 5+**.
//! Earliest stage that can run the manual version: **Stage 5+** (it
//! still needs Cocoa NodeRef and on:input wiring).
//!
//! Status: aspirational — won't compile yet.

use leptos::prelude::*;

// ---- Idiomatic version using bind: -------------------------------------

#[component]
fn FormWithBind() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let agreed = RwSignal::new(false);

    view! {
        <stack_view orientation="vertical" spacing=8.0>
            <text_field bind:value=name placeholder="Name" />
            <checkbox bind:state=agreed>"I agree to the terms"</checkbox>
            <button
                enabled={move || !name.get().is_empty() && agreed.get()}
                on:click=move |_| submit(name.get())>
                "Submit"
            </button>
        </stack_view>
    }
}

// ---- Manual version: explicit Effect + NodeRef + on:input ---------------

#[component]
fn FormWithoutBind() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let agreed = RwSignal::new(false);

    // Need a NodeRef per field so the Effect can reach back into the
    // NSView and call setStringValue: / setState: as the signal changes.
    let name_field: NodeRef<TextField> = NodeRef::new();
    let agree_box: NodeRef<Checkbox> = NodeRef::new();

    // Push: signal → control. Runs on every name change.
    Effect::new(move |_| {
        if let Some(field) = name_field.get() {
            field.set_string_value(&name.get());
        }
    });
    Effect::new(move |_| {
        if let Some(cb) = agree_box.get() {
            cb.set_state(agreed.get());
        }
    });

    view! {
        <stack_view orientation="vertical" spacing=8.0>
            <text_field
                node_ref=name_field
                placeholder="Name"
                // Pull: control → signal. NSControlTextDidChange under the hood.
                on:input=move |ev| name.set(event_target_value(&ev)) />

            <checkbox
                node_ref=agree_box
                on:click=move |_| agreed.update(|b| *b = !*b)>
                "I agree to the terms"
            </checkbox>

            <button
                enabled={move || !name.get().is_empty() && agreed.get()}
                on:click=move |_| submit(name.get())>
                "Submit"
            </button>
        </stack_view>
    }
}

fn submit(_name: String) {}

fn main() {
    // Run whichever you want to compare.
    leptos::mount::mount_to_window(|| view! { <FormWithBind /> });
}
