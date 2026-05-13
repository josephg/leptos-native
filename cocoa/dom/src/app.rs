//! NSApplication-level setup: activation policy, menu bar,
//! AppDelegate, run loop. Per-window machinery lives in
//! [`crate::window`] / `tachys::cocoa::window` instead.

use crate::spawner;
use objc2::{
    define_class, rc::Retained, runtime::ProtocolObject, sel,
    MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSEventModifierFlags, NSMenu, NSMenuItem,
};
use objc2_foundation::{
    NSObject, NSObjectProtocol, NSProcessInfo, NSString,
};

/// Initialise the AppKit application: activation policy, default
/// menu bar (`App > Quit ⌘Q`), AppDelegate (terminate on last-window
/// close), and the [`crate::spawner`] main-thread executor.
///
/// Idempotent on the spawner; calling repeatedly otherwise replaces
/// the menu/delegate (probably not what you want — call once at app
/// startup).
///
/// Returns the shared [`NSApplication`] for the caller to drive
/// (typically by calling [`run_loop`]).
pub fn init_app(mtm: MainThreadMarker) -> Retained<NSApplication> {
    let _ = spawner::init();
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    install_default_menu(&app, mtm);

    let delegate = AppDelegate::new(mtm);
    let delegate_proto: &ProtocolObject<dyn NSApplicationDelegate> =
        ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(delegate_proto));
    // The app holds the delegate weakly; leak ours so it lives forever.
    std::mem::forget(delegate);

    app
}

/// Run the AppKit run loop. Blocks until the app terminates (via
/// Cmd-Q or last-window-close).
pub fn run_loop(app: &NSApplication) {
    app.run();
}

/// Programmatically quit the application. Calls
/// `NSApplication::terminate:` on the shared app object, which goes
/// through the normal AppKit shutdown sequence (asks the delegate
/// via `applicationShouldTerminate:`, posts `windowWillClose:` on
/// each window, etc.) before exiting.
///
/// Use this from `on:action` handlers on a Quit menu item:
///
/// ```ignore
/// <menu_item title="Quit MyApp" shortcut="q" on:action=move |_| quit() />
/// ```
///
/// Must run on the main thread (panics otherwise).
pub fn quit() {
    let mtm = objc2::MainThreadMarker::new()
        .expect("cocoa_dom::app::quit must run on the main thread");
    NSApplication::sharedApplication(mtm).terminate(None);
}

// ---------------------------------------------------------------------
// AppDelegate — quit on last-window-close
// ---------------------------------------------------------------------

define_class!(
    /// Tiny NSApplicationDelegate that quits the app when the user
    /// closes the last open window. Mirrors the default behaviour of
    /// document-based AppKit apps. No other delegate methods are
    /// implemented yet — extend here when we need lifecycle hooks
    /// (open file, sleep/wake, etc.).
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn should_terminate_after_last_window_closed(
            &self,
            _sender: &NSApplication,
        ) -> bool {
            true
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let alloc = Self::alloc(mtm).set_ivars(());
        unsafe { objc2::msg_send![super(alloc), init] }
    }
}

// ---------------------------------------------------------------------
// Menu
// ---------------------------------------------------------------------

/// Install a minimal main menu so the app behaves like a real macOS
/// app:
///   - **App** menu (its title is auto-replaced by the running
///     process's name): `Quit ⌘Q`
///   - **Edit** menu wired to first-responder selectors so standard
///     text-field shortcuts work in NSTextField/NSSecureTextField
///     (Undo ⌘Z, Redo ⇧⌘Z, Cut ⌘X, Copy ⌘C, Paste ⌘V, Select All ⌘A,
///     Delete).
///
/// "First-responder" wiring: each Edit item has `target: nil` and a
/// selector like `selectAll:` / `cut:` / `copy:`. AppKit dispatches
/// the selector through the responder chain, so the focused
/// NSTextField (which implements all of these natively) handles it.
/// We don't need to implement anything ourselves — only the menu
/// items are required for the keyboard shortcuts to bind.
fn install_default_menu(app: &NSApplication, mtm: MainThreadMarker) {
    let main_menu = NSMenu::new(mtm);

    // ---- App menu ----
    let app_menu_item = NSMenuItem::new(mtm);
    main_menu.addItem(&app_menu_item);

    let app_menu = NSMenu::new(mtm);
    let process_name = NSProcessInfo::processInfo().processName();
    let quit_title =
        NSString::from_str(&format!("Quit {}", process_name.to_string()));
    let quit_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &quit_title,
            Some(sel!(terminate:)),
            &NSString::from_str("q"),
        )
    };
    app_menu.addItem(&quit_item);
    app_menu_item.setSubmenu(Some(&app_menu));

    // ---- Edit menu ----
    let edit_menu_item = NSMenuItem::new(mtm);
    main_menu.addItem(&edit_menu_item);

    let edit_menu = NSMenu::new(mtm);
    edit_menu.setTitle(&NSString::from_str("Edit"));

    add_edit_item(&edit_menu, mtm, "Undo", sel!(undo:), "z", false);
    add_edit_item(&edit_menu, mtm, "Redo", sel!(redo:), "z", true);
    edit_menu.addItem(&NSMenuItem::separatorItem(mtm));
    add_edit_item(&edit_menu, mtm, "Cut", sel!(cut:), "x", false);
    add_edit_item(&edit_menu, mtm, "Copy", sel!(copy:), "c", false);
    add_edit_item(&edit_menu, mtm, "Paste", sel!(paste:), "v", false);
    add_edit_item(&edit_menu, mtm, "Delete", sel!(delete:), "", false);
    add_edit_item(
        &edit_menu,
        mtm,
        "Select All",
        sel!(selectAll:),
        "a",
        false,
    );

    edit_menu_item.setSubmenu(Some(&edit_menu));

    app.setMainMenu(Some(&main_menu));
}

/// Add a first-responder-targeted item to an Edit-style menu.
/// Target is implicitly nil (responder chain dispatch); pass
/// `shifted=true` to add the Shift modifier to the key equivalent.
fn add_edit_item(
    menu: &NSMenu,
    mtm: MainThreadMarker,
    title: &str,
    action: objc2::runtime::Sel,
    key: &str,
    shifted: bool,
) {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(action),
            &NSString::from_str(key),
        )
    };
    if shifted {
        item.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
        );
    }
    menu.addItem(&item);
}
