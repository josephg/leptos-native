//! Low-level smoke test: open an empty GtkApplicationWindow using the
//! gtk4 crate directly. The DOM-shaped façade in `gtk_dom` itself
//! doesn't exist yet at Stage 0; this example is just confirmation
//! that the gtk4 toolchain is wired up and the workspace builds
//! end-to-end on Linux.
//!
//! Run with:
//!     cargo run -p gtk_dom --example hello_window

#[cfg(target_os = "linux")]
fn main() {
    use gtk4::prelude::*;
    use gtk4::{Application, ApplicationWindow};

    let app = Application::builder()
        .application_id("org.leptos.gtk_dom.hello_window")
        .build();

    app.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("gtk_dom — hello")
            .default_width(400)
            .default_height(220)
            .build();
        window.present();
    });

    app.run();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("gtk_dom only runs on Linux");
}
