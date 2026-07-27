//! `<canvas>` element tests — scene storage (static + reactive) and
//! mouse-event dispatch.
//!
//! Uses the custom main-thread harness (see `tests/common/mod.rs`) —
//! AppKit view construction needs the actual main thread. Mouse
//! events are synthesised via `CanvasView::fire_mouse_*_for_test`
//! (a real `mouseDown:` needs an NSEvent, which can't be built
//! without a window — same reason `fire_action` invokes
//! `actionFired:` directly for buttons).

#![cfg(target_os = "macos")]

extern crate leptos_cocoa as leptos_platform;

mod common;

use leptos_cocoa::cocoa::element::canvas;
use leptos_cocoa::dom::{
    CanvasPoint, CanvasView, CocoaElem, CocoaMakeView, CocoaNodeExt, Color,
    DrawCmd,
};
use leptos_cocoa::event_macos as event;
use leptos_native::renderer::view::Render;
use objc2::runtime::AnyObject;
use objc2::Message;
use reactive_graph::owner::Owner;
use reactive_graph::signal::RwSignal;
use reactive_graph::traits::{Get, Set};
use std::sync::{Arc, Mutex};

fn with_reactive_scope<F: FnOnce()>(f: F) {
    // `init` is process-global; the custom harness runs every test in one
    // process, so only the first call succeeds. Ignore the `AlreadySet` the
    // rest return — it just means the executor is already wired up.
    let _ = leptos_cocoa::dom::spawner::init();
    let owner = Owner::new();
    owner.with(f);
}

/// Downcast a built element's NSView to CanvasView, panicking with
/// context on mismatch.
fn canvas_view_of(el: CocoaElem) -> objc2::rc::Retained<CanvasView> {
    let view = el.ns_view();
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<CanvasView>()
        .expect("canvas element should be backed by CanvasView")
        .retain()
}

fn sample_scene(offset: f64) -> Vec<DrawCmd> {
    vec![
        DrawCmd::FillRect {
            x: offset,
            y: 10.0,
            w: 40.0,
            h: 20.0,
            color: Color::RED,
        },
        DrawCmd::StrokeEllipse {
            x: 5.0,
            y: 5.0,
            w: 30.0,
            h: 30.0,
            color: Color::BLUE,
            width: 2.0,
            dashed: true,
        },
        DrawCmd::Polyline {
            points: vec![(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)],
            color: Color::BLACK,
            width: 1.5,
        },
        DrawCmd::Text {
            x: 2.0,
            y: 40.0,
            text: "hi".to_string(),
            color: Color::LABEL,
            size: 13.0,
        },
    ]
}

// -- dom layer --------------------------------------------------------

fn canvas_is_canvas_view_flipped_and_accepts_first_mouse() {
    let _mtm = common::test_mtm();
    let (el, view) = CocoaElem::create_canvas();

    // Same class the element's ns_view reports.
    let via_el = canvas_view_of(el);
    assert_eq!(&*via_el as *const _, &*view as *const _);

    assert!(view.isFlipped(), "canvas must be flipped (top-left origin)");
    assert!(
        view.acceptsFirstMouse(None),
        "canvas must accept the window-activating click"
    );
    el.remove();
}

fn canvas_scene_round_trips_through_node_setter() {
    let _mtm = common::test_mtm();
    let (el, view) = CocoaElem::create_canvas();

    assert!(el.canvas_scene().is_empty(), "fresh canvas has empty scene");
    let scene = sample_scene(0.0);
    el.set_canvas_scene(scene.clone());
    assert_eq!(view.scene(), scene);
    assert_eq!(el.canvas_scene(), scene);

    // Non-canvas nodes: setter no-ops, reader yields empty.
    let label = CocoaElem::create_label().0;
    label.set_canvas_scene(scene);
    assert!(label.canvas_scene().is_empty());
    label.remove();
    el.remove();
}

// -- builder layer ----------------------------------------------------

fn canvas_builder_stores_static_scene() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let scene = sample_scene(3.0);
        let state = canvas().scene(scene.clone()).build();
        let view = canvas_view_of(state.el);
        assert_eq!(view.scene(), scene);
    });
}

fn canvas_scene_reactive_updates() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let offset = RwSignal::new(0.0_f64);
        let state = canvas()
            .scene(move || sample_scene(offset.get()))
            .build();
        let view = canvas_view_of(state.el);

        // RenderEffect runs the closure once synchronously at install.
        assert_eq!(view.scene(), sample_scene(0.0));

        offset.set(25.0);
        common::pump_run_loop(0.1);
        assert_eq!(view.scene(), sample_scene(25.0));
    });
}

fn canvas_mouse_handlers_fire_with_points() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        // (event name, point) log shared with the handlers.
        let log: Arc<Mutex<Vec<(&'static str, CanvasPoint)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let (down, drag, up) = (log.clone(), log.clone(), log.clone());

        let state = canvas()
            .on(event::mouse_down, move |p: CanvasPoint| {
                down.lock().unwrap().push(("down", p));
            })
            .on(event::mouse_drag, move |p: CanvasPoint| {
                drag.lock().unwrap().push(("drag", p));
            })
            .on(event::mouse_up, move |p: CanvasPoint| {
                up.lock().unwrap().push(("up", p));
            })
            .build();
        let view = canvas_view_of(state.el);

        view.fire_mouse_down_for_test(CanvasPoint { x: 3.0, y: 4.0 });
        view.fire_mouse_drag_for_test(CanvasPoint { x: 5.0, y: 6.0 });
        view.fire_mouse_up_for_test(CanvasPoint { x: 7.0, y: 8.0 });

        let got = log.lock().unwrap().clone();
        assert_eq!(
            got,
            vec![
                ("down", CanvasPoint { x: 3.0, y: 4.0 }),
                ("drag", CanvasPoint { x: 5.0, y: 6.0 }),
                ("up", CanvasPoint { x: 7.0, y: 8.0 }),
            ]
        );
    });
}

// -- view!{} macro path -----------------------------------------------

fn canvas_via_view_macro_builds() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        use leptos_platform::prelude::*;

        let last = RwSignal::new(None::<CanvasPoint>);
        let v = view! {
            <canvas
                flex_grow=1.0
                background_color=Color::rgb(1.0, 1.0, 1.0)
                scene=move || sample_scene(1.0)
                on:mouse_down=move |p: CanvasPoint| last.set(Some(p))
                on:mouse_drag=move |_p: CanvasPoint| {}
                on:mouse_up=move |_p: CanvasPoint| {}
            />
        };
        let state = v.build();
        let view = canvas_view_of(state.el);
        assert_eq!(view.scene(), sample_scene(1.0));

        view.fire_mouse_down_for_test(CanvasPoint { x: 11.0, y: 12.0 });
        assert_eq!(
            last.get_untracked(),
            Some(CanvasPoint { x: 11.0, y: 12.0 })
        );
    });
}

fn main() {
    common::run_tests(&[
        (
            "canvas_is_canvas_view_flipped_and_accepts_first_mouse",
            canvas_is_canvas_view_flipped_and_accepts_first_mouse,
        ),
        (
            "canvas_scene_round_trips_through_node_setter",
            canvas_scene_round_trips_through_node_setter,
        ),
        ("canvas_builder_stores_static_scene", canvas_builder_stores_static_scene),
        ("canvas_scene_reactive_updates", canvas_scene_reactive_updates),
        (
            "canvas_mouse_handlers_fire_with_points",
            canvas_mouse_handlers_fire_with_points,
        ),
        ("canvas_via_view_macro_builds", canvas_via_view_macro_builds),
    ]);
}
