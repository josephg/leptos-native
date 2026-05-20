//! Low-level counter built without tachys' Render machinery.
//! Uses cocoa_dom directly + reactive_graph signals/effects.
//!
//! For the higher-level builder version see `counter_v2.rs`.
//!
//! Run with:
//!     cargo run -p cocoa_dom --example counter

#[cfg(target_os = "macos")]
fn main() {
    use cocoa_dom::{
        app::{init_app, run_loop},
        layout::compute_layout,
        window::open_window,
        Element, MainThreadMarker,
    };
    use objc2_app_kit::NSView;
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use reactive_graph::{
        effect::Effect, owner::Owner, signal::RwSignal, traits::*,
    };

    let mtm = MainThreadMarker::new().expect("must run on main thread");
    let (app, _delegate) = init_app(mtm);

    let owner = Owner::new();
    owner.set();
    std::mem::forget(owner);

    let opened = open_window("cocoa_dom — counter", (320.0, 200.0), mtm);

    // ---- the reactive state ----
    let count = RwSignal::new(0_i32);

    // ---- build the view (hand-placed frames; no taffy at this level) ----
    let label = Element::create_text_with("Count: 0", mtm);
    set_frame(&label.as_node().ns_view(), 20.0, 140.0, 280.0, 24.0);
    opened.content_root.insert_node(label.as_node(), None);

    let dec = Element::create_button().0;
    dec.set_title("-1");
    set_frame(&dec.ns_view(), 20.0, 80.0, 80.0, 32.0);
    opened.content_root.insert_node(dec.as_node(), None);

    let reset = Element::create_button().0;
    reset.set_title("Reset");
    set_frame(&reset.ns_view(), 120.0, 80.0, 80.0, 32.0);
    opened.content_root.insert_node(reset.as_node(), None);

    let inc = Element::create_button().0;
    inc.set_title("+1");
    set_frame(&inc.ns_view(), 220.0, 80.0, 80.0, 32.0);
    opened.content_root.insert_node(inc.as_node(), None);

    // ---- wire signal → label ----
    let label_for_effect = label.clone();
    Effect::new(move |_| {
        let c = count.get();
        label_for_effect.set_text(&format!("Count: {c}"));
    });

    // ---- wire button clicks ----
    dec.on_click(move || count.update(|n| *n -= 1));
    reset.on_click(move || count.set(0));
    inc.on_click(move || count.update(|n| *n += 1));

    let size = opened.content_root.ns_view().frame().size;
    compute_layout(opened.content_root.as_node(), size);
    opened.show(mtm);
    run_loop(&app);

    fn set_frame(view: &NSView, x: f64, y: f64, w: f64, h: f64) {
        view.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(w, h)));
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("cocoa_dom only runs on macOS");
}
