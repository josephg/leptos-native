//! Layout regression tests for the leptos_gtk builders. Mirrors
//! `leptos_cocoa/tests/layout.rs`.
//!
//! GTK's measure/allocate cycle isn't run in unit tests (no display);
//! we exercise the Taffy bridge directly via
//! `gtk_dom::layout::compute_layout` and assert the per-element
//! Taffy-computed sizes.

#![cfg(feature = "gtk")]

mod common;

use leptos_gtk::gtk::element::{button, hstack, label, vstack};
use reactive_graph::owner::Owner;
use renderer::attrs::WithLayout;
use renderer::view::{Mountable, Render};
use leptos_gtk::dom::{layout, layout::GtkBackend, spawner, window, GtkNode};
use renderer::LayoutBackend;
use leptos_gtk::gtk4::prelude::*;

fn with_reactive_scope<F: FnOnce()>(body: F) {
    let _ = spawner::init();
    let owner = Owner::new();
    owner.with(body);
}

/// Open a window (without `present()`), build `view`, mount it under
/// the content_root, run compute_layout against the given size, then
/// pass the content_root to `f`.
fn with_mounted_view<V, F>(view: V, size: (f32, f32), f: F)
where
    V: Render<leptos_gtk::Dom>,
    V::State: Mountable<leptos_gtk::Dom>,
    F: FnOnce(&GtkNode),
{
    let app = common::init_app_registered("org.test.leptos_gtk.layout");
    let opened = window::open_window(
        &app,
        "test",
        (size.0 as i32, size.1 as i32),
    );
    let mut state = view.build();
    state.mount(opened.content_root, None);

    layout::compute_layout(opened.content_root, size);
    f(&opened.content_root);

    drop(state);
    opened.close();
}

/// Walk the Taffy tree under `root` and collect the layouts of all
/// leaf widgets that match `pred`.
fn find_leaf_widgets<P>(
    root: &GtkNode,
    mut pred: P,
) -> Vec<layout::Layout>
where
    P: FnMut(&gtk4::Widget) -> bool,
{
    let mut out = Vec::new();
    walk(root.id(), &mut |id, w| {
        if pred(w) {
            if let Some(layout) = GtkBackend::layout(id) {
                out.push(layout);
            }
        }
    });
    out
}

fn walk<F>(id: layout::NodeId, f: &mut F)
where
    F: FnMut(layout::NodeId, &gtk4::Widget),
{
    let widget = GtkBackend::view(id);
    let children = GtkBackend::children(id);
    if let Some(w) = widget {
        f(id, &w);
    }
    for c in children {
        walk(c, f);
    }
}

fn leaf_controls_have_nonzero_intrinsic_size() {
    with_reactive_scope(|| {
        let view = vstack().padding(16.0).gap(12.0).child(
            label().text("Hello"),
        );
        with_mounted_view(view, (320.0, 200.0), |root| {
            let labels = find_leaf_widgets(root, |w| {
                w.is::<gtk4::Label>()
            });
            assert!(!labels.is_empty(), "no Label found in tree");
            for l in &labels {
                assert!(
                    l.size.height > 0.0,
                    "label height was zero: {:?}",
                    l
                );
                assert!(
                    l.size.width > 0.0,
                    "label width was zero: {:?}",
                    l
                );
            }
        });
    });
}

fn buttons_in_hstack_have_natural_size() {
    with_reactive_scope(|| {
        let view = hstack()
            .gap(8.0)
            .child(button().title("OK"))
            .child(button().title("Cancel"));
        with_mounted_view(view, (320.0, 100.0), |root| {
            let buttons = find_leaf_widgets(root, |w| {
                w.is::<gtk4::Button>()
            });
            assert_eq!(buttons.len(), 2, "expected 2 buttons in tree");
            for (i, b) in buttons.iter().enumerate() {
                assert!(
                    b.size.height > 0.0,
                    "button[{i}] height zero"
                );
                assert!(
                    b.size.width > 0.0,
                    "button[{i}] width zero"
                );
            }
        });
    });
}

fn vstack_label_plus_hstack_has_full_height() {
    with_reactive_scope(|| {
        let view = vstack()
            .padding(16.0)
            .gap(12.0)
            .child(label().text("Count: 0"))
            .child(
                hstack()
                    .gap(8.0)
                    .child(button().title("-1"))
                    .child(button().title("Reset"))
                    .child(button().title("+1")),
            );
        with_mounted_view(view, (320.0, 200.0), |root| {
            // First child of the content_root is the outer vstack.
            let kids = GtkBackend::children(root.id());
            assert!(!kids.is_empty(), "content_root has no children");
            let outer_id = kids[0];
            let outer_layout =
                GtkBackend::layout(outer_id).expect("outer layout missing");
            assert!(
                outer_layout.size.height >= 60.0,
                "vstack height {} suspiciously small",
                outer_layout.size.height
            );
            assert!(
                (outer_layout.size.width - 320.0).abs() < 1.0,
                "vstack should fill its parent width, got {}",
                outer_layout.size.width
            );
        });
    });
}

fn main() {
    common::run_tests(&[
        (
            "leaf_controls_have_nonzero_intrinsic_size",
            leaf_controls_have_nonzero_intrinsic_size,
        ),
        (
            "buttons_in_hstack_have_natural_size",
            buttons_in_hstack_have_natural_size,
        ),
        (
            "vstack_label_plus_hstack_has_full_height",
            vstack_label_plus_hstack_has_full_height,
        ),
    ]);
}
