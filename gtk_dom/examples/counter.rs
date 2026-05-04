//! Low-level counter built without tachys' Render machinery.
//! Uses gtk_dom directly + reactive_graph signals/effects.
//!
//! For the higher-level builder version see `counter_v2.rs` (added
//! once tachys::gtk lands in Stage 5).
//!
//! Run with:
//!     cargo run -p gtk_dom --example counter

#[cfg(target_os = "linux")]
fn main() {
    use gtk_dom::{
        app::{init_app, run_loop},
        gtk::{prelude::*, Box as GtkBox},
        window::open_window,
        Element, Text,
    };
    use reactive_graph::{
        effect::Effect, owner::Owner, signal::RwSignal, traits::*,
    };

    let app = init_app("org.leptos.gtk_dom.counter");

    app.connect_activate(|app| {
        // Reactive scope rooted for the window's lifetime. Leaking
        // the Owner is intentional — we don't have a clean teardown
        // story for the counter example.
        let owner = Owner::new();
        owner.set();
        std::mem::forget(owner);

        let opened = open_window(app, "gtk_dom — counter", (320, 200));

        // ---- the reactive state ----
        let count = RwSignal::new(0_i32);

        // ---- build the view ----
        // content_root is already a vstack from open_window; add some
        // spacing and margin for readability.
        let root_box = opened
            .content_root
            .widget()
            .downcast_ref::<GtkBox>()
            .expect("content_root is a Box");
        root_box.set_spacing(12);
        root_box.set_margin_top(16);
        root_box.set_margin_bottom(16);
        root_box.set_margin_start(16);
        root_box.set_margin_end(16);

        let label = Text::create("Count: 0");
        opened.content_root.insert_node(label.as_node(), None);

        // Horizontal row of buttons.
        let row = Element::create("hstack");
        let row_box = row
            .widget()
            .downcast_ref::<GtkBox>()
            .expect("hstack is a Box");
        row_box.set_spacing(8);
        row_box.set_homogeneous(true);
        opened.content_root.insert_node(row.as_node(), None);

        let dec = Element::create("button");
        dec.set_attribute("title", "-1");
        row.insert_node(dec.as_node(), None);

        let reset = Element::create("button");
        reset.set_attribute("title", "Reset");
        row.insert_node(reset.as_node(), None);

        let inc = Element::create("button");
        inc.set_attribute("title", "+1");
        row.insert_node(inc.as_node(), None);

        // ---- wire signal → label ----
        let label_for_effect = label.clone();
        Effect::new(move |_| {
            let c = count.get();
            label_for_effect.set_text(&format!("Count: {c}"));
        });

        // ---- wire button clicks ----
        // Each on_click moves a copy of the (Copy) signal into its
        // closure; reactive_graph's RwSignal is `Copy`, so this
        // works without further cloning.
        dec.on_click(move || count.update(|n| *n -= 1));
        reset.on_click(move || count.set(0));
        inc.on_click(move || count.update(|n| *n += 1));

        opened.show();
    });

    run_loop(&app);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("gtk_dom only runs on Linux");
}
