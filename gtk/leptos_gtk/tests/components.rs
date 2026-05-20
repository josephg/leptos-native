//! `#[component]` macro expansion tests for `leptos_gtk`.
//!
//! Mirrors `leptos_cocoa::tests::components`.

#![cfg(feature = "gtk")]

mod common;

use leptos_gtk::prelude::*;
use leptos_gtk::gtk::element::{button, label, vstack};
use reactive_graph::owner::Owner;
use renderer::view::Render;

fn with_reactive_scope<F: FnOnce()>(f: F) {
    let _ = gtk_dom::spawner::init();
    let owner = Owner::new();
    owner.with(f);
}

// -- 1. No-prop component --------------------------------------------

#[component]
fn NoProps() -> impl IntoView {
    label().text("hello")
}

fn no_prop_component_builds() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = NoProps().build(&tree);
    });
}

// -- 2. Required prop -----------------------------------------------

#[component]
fn WithProp(initial: i32) -> impl IntoView {
    label().text(format!("init={initial}"))
}

fn required_prop_component_builds() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = WithProp(WithPropProps { initial: 42 }).build(&tree);
    });
}

// -- 3. Optional prop ------------------------------------------------

#[component]
fn WithOpt(#[prop(optional)] subtitle: Option<String>) -> impl IntoView {
    label().text(subtitle.unwrap_or_else(|| "<none>".to_string()))
}

fn optional_prop_component_builds_without_value() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = WithOpt(WithOptProps::builder().build()).build(&tree);
    });
}

fn optional_prop_component_builds_with_value() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = WithOpt(
            WithOptProps::builder()
                .subtitle("hi".to_string())
                .build(),
        )
        .build(&tree);
    });
}

// -- 4. Default prop value -------------------------------------------

#[component]
fn WithDefault(#[prop(default = 7)] value: i32) -> impl IntoView {
    label().text(format!("v={value}"))
}

fn default_prop_value_used_when_omitted() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = WithDefault(WithDefaultProps::builder().build()).build(&tree);
    });
}

// -- 5. Snake-case fn body, PascalCase generated name ----------------

#[component]
fn snake_named() -> impl IntoView {
    label().text("snake")
}

fn snake_case_fn_yields_pascal_case_component() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = SnakeNamed().build(&tree);
    });
}

// -- 6. Transparent component ----------------------------------------

#[component(transparent)]
fn Transparent() -> impl IntoView {
    label().text("transparent")
}

fn transparent_component_compiles_and_builds() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = Transparent().build(&tree);
    });
}

// -- 7. Generic component --------------------------------------------

#[component]
fn Generic<T: ToString + 'static + Send>(value: T) -> impl IntoView {
    label().text(value.to_string())
}

fn generic_component_with_int() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = Generic(GenericProps::builder().value(42i32).build()).build(&tree);
    });
}

fn generic_component_with_string() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = Generic(
            GenericProps::builder()
                .value("hi".to_string())
                .build(),
        )
        .build(&tree);
    });
}

// -- 8. Component nesting --------------------------------------------

#[component]
fn Inner() -> impl IntoView {
    label().text("inner")
}

#[component]
fn Outer() -> impl IntoView {
    vstack().child(Inner())
}

fn outer_can_invoke_inner_component() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = Outer().build(&tree);
    });
}

// -- 9. Reactive state inside component ------------------------------

#[component]
fn Counter(initial: i32) -> impl IntoView {
    let count = RwSignal::new(initial);
    vstack().child(
        button()
            .title("inc")
            .on(leptos_gtk::event_gtk::click, move |_: ()| {
                count.update(|n| *n += 1)
            }),
    )
}

fn counter_component_with_signal_compiles() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = Counter(CounterProps { initial: 0 }).build(&tree);
    });
}

// -- 10. Component with `into` prop ---------------------------------

#[component]
fn IntoProp(#[prop(into)] label_text: String) -> impl IntoView {
    label().text(label_text)
}

fn into_prop_accepts_str() {
    with_reactive_scope(|| {
        let tree = gtk_dom::layout::new_tree();
        let _ = IntoProp(
            IntoPropProps::builder().label_text("static").build(),
        )
        .build(&tree);
    });
}

fn main() {
    common::run_tests(&[
        ("no_prop_component_builds", no_prop_component_builds),
        ("required_prop_component_builds", required_prop_component_builds),
        (
            "optional_prop_component_builds_without_value",
            optional_prop_component_builds_without_value,
        ),
        (
            "optional_prop_component_builds_with_value",
            optional_prop_component_builds_with_value,
        ),
        (
            "default_prop_value_used_when_omitted",
            default_prop_value_used_when_omitted,
        ),
        (
            "snake_case_fn_yields_pascal_case_component",
            snake_case_fn_yields_pascal_case_component,
        ),
        (
            "transparent_component_compiles_and_builds",
            transparent_component_compiles_and_builds,
        ),
        ("generic_component_with_int", generic_component_with_int),
        ("generic_component_with_string", generic_component_with_string),
        ("outer_can_invoke_inner_component", outer_can_invoke_inner_component),
        (
            "counter_component_with_signal_compiles",
            counter_component_with_signal_compiles,
        ),
        ("into_prop_accepts_str", into_prop_accepts_str),
    ]);
}
