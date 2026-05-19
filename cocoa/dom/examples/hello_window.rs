//! Low-level smoke test: open a window with a hand-built tree using
//! the cocoa_dom API directly (no tachys, no Render). Exercises
//! create_element, create_text_node, insert_node, set_attribute,
//! set_text, remove_child.
//!
//! Run with:
//!     cargo run -p cocoa_dom --example hello_window

#[cfg(target_os = "macos")]
fn main() {
    use cocoa_dom::{
        app::{init_app, run_loop},
        layout::compute_layout,
        window::open_window,
        Element, MainThreadMarker,
    };

    let mtm = MainThreadMarker::new().expect("must run on main thread");
    let (app, _delegate) = init_app(mtm);

    let opened = open_window("cocoa_dom — hello", (400.0, 220.0), mtm);

    let button = Element::create_button(&opened.tree).0;
    button.set_title("Click me");

    let label = Element::create_label(&opened.tree).0;
    label.set_value("(initial label)");

    let text_node = Element::create_text_with(&opened.tree, "text-node says hi", mtm);

    opened.content_root.insert_node(button.as_node(), None);
    opened.content_root.insert_node(label.as_node(), None);
    opened.content_root.insert_node(text_node.as_node(), None);

    // Demonstrate insertion before a marker, then remove_child.
    let middle_label = Element::create_label(&opened.tree).0;
    middle_label.set_value("(inserted before text-node)");
    opened
        .content_root
        .insert_node(middle_label.as_node(), Some(text_node.as_node()));

    text_node.set_text("text-node has been updated");

    let _ = opened.content_root.remove_child(middle_label.as_node());

    let size = opened.content_root.ns_view().frame().size;
    compute_layout(opened.content_root.as_node(), size);
    opened.show(mtm);

    run_loop(&app);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("cocoa_dom only runs on macOS");
}
