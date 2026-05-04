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
/// Returns the [`gtk::Application`] for the caller to drive
/// (typically by attaching `connect_activate` and then calling
/// [`run_loop`]).
///
/// [bus-name]: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-bus
pub fn init_app(application_id: &str) -> gtk4::Application {
    let _ = spawner::init();
    gtk4::Application::builder()
        .application_id(application_id)
        .build()
}

/// Run the GTK main loop. Blocks until the app terminates (last
/// window closed, etc.). The returned exit code is discarded — if a
/// caller cares, they can call `app.run()` directly instead.
pub fn run_loop(app: &gtk4::Application) {
    let _ = app.run();
}
