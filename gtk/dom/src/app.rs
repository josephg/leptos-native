//! GtkApplication-level setup: building the application object and
//! running the main loop. Per-window machinery lives in
//! [`crate::window`].

use crate::spawner;
use gtk4::prelude::*;

/// Initialise the GTK application object. Sets up the main-thread
/// async executor (idempotent — see [`spawner::init`]) and constructs
/// a [`gtk::Application`] with the given application ID.
///
/// `application_id` should be a valid reverse-DNS string (per the
/// [D-Bus naming rules][bus-name]) — for example
/// `"org.example.Counter"`. GTK uses this for application uniqueness
/// (single-instance behavior, settings storage).
///
/// A standard `app.quit` action is registered with the
/// `<Primary>q` accelerator. `<Primary>` is GTK's portable
/// "Ctrl on Linux, Cmd on macOS" modifier, so this gives Ctrl+Q
/// where it's expected and Cmd+Q where it's expected — without a
/// menu bar. Idiomatic for both platforms (matching gedit,
/// gnome-terminal, etc. on Linux); the macOS-side benefit is
/// incidental but real, so the same default works for the
/// cross-platform-testing case.
///
/// Returns the [`gtk::Application`] for the caller to drive
/// (typically by attaching `connect_activate` and then calling
/// [`run_loop`]).
///
/// [bus-name]: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-bus
pub fn init_app(application_id: &str) -> gtk4::Application {
    let _ = spawner::init();
    let app = gtk4::Application::builder()
        .application_id(application_id)
        .build();

    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate({
        let app = app.clone();
        move |_, _| app.quit()
    });
    app.add_action(&quit);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);

    app
}

/// Run the GTK main loop. Blocks until the app terminates (last
/// window closed, etc.). The returned exit code is discarded — if a
/// caller cares, they can call `app.run()` directly instead.
pub fn run_loop(app: &gtk4::Application) {
    let _ = app.run();
}
