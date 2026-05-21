//! GTK smoke test for the `LayoutNode` / `UniversalNode` /
//! `LayoutNodeOps` impls — verifies the per-port glue that connects
//! `gtk_dom::Node` / `gtk_dom::Node` to the renderer-side
//! `apply_layout` / `apply_universal` machinery.
//!
//! **Coverage strategy:** the canonical, exhaustive coverage of
//! `apply_layout` / `apply_universal` (every LayoutAttrs field, every
//! Dim variant, every grid-placement enum, reactive vs. static, etc.)
//! lives in `cocoa/dom/tests/apply_attrs.rs`. Those tests exercise the
//! *generic* renderer code through cocoa's element type. Since the
//! generic functions don't know which port they're running against —
//! they only call methods on the trait — passing a single port is
//! enough to validate the trait-driven install loop itself.
//!
//! What this file checks is the *port-specific glue*: that
//! `gtk_dom::Node` actually routes through to the generic
//! `apply_layout` and that the trait impls behave as expected on a
//! GTK widget tree.

#![cfg(feature = "gtk")]

mod common;

use leptos_gtk::dom::{layout, GtkNode};
use renderer::attrs::{LayoutAttrs, MaybeReactive, UniversalAttrs};

fn style_of(el: &GtkNode) -> renderer::Style {
    el.as_node().with_style(|s| s.clone())
}

fn padding_static_lands_in_padding_field() {
    let el = GtkNode::create_stack();

    let mut attrs = LayoutAttrs::default();
    attrs.padding = Some(MaybeReactive::Static(renderer::attrs::Edges::all(8.0)));
    let effects = layout::apply_layout(&el, attrs);
    assert!(effects.is_empty());

    let s = style_of(&el);
    assert_eq!(s.padding.left, renderer::LengthPercentage::length(8.0));
}

fn flex_grow_static_lands_in_flex_grow() {
    let el = GtkNode::create_stack();

    let mut attrs = LayoutAttrs::default();
    attrs.flex_grow = Some(MaybeReactive::Static(1.5));
    let _ = layout::apply_layout(&el, attrs);
    assert_eq!(style_of(&el).flex_grow, 1.5);
}

fn empty_universal_attrs_no_panic() {
    let el = GtkNode::create_stack();
    let _ = layout::apply_universal(&el, UniversalAttrs::default());
}

fn alpha_static_sets_widget_opacity() {
    use gtk4::prelude::*;
    let el = GtkNode::create_stack();

    let mut attrs = UniversalAttrs::default();
    attrs.alpha = Some(MaybeReactive::Static(0.4));
    let _ = layout::apply_universal(&el, attrs);

    let opacity = el.widget().opacity();
    assert!((opacity - 0.4).abs() < 1e-6, "got opacity={opacity}");
}

fn main() {
    common::run_tests(&[
        ("padding_static_lands_in_padding_field", padding_static_lands_in_padding_field),
        ("flex_grow_static_lands_in_flex_grow", flex_grow_static_lands_in_flex_grow),
        ("empty_universal_attrs_no_panic", empty_universal_attrs_no_panic),
        ("alpha_static_sets_widget_opacity", alpha_static_sets_widget_opacity),
    ]);
}
