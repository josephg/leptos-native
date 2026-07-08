//! Small system-services helpers: bundle resource lookup and opening
//! URLs in the system browser. Safe wrappers so app crates don't need
//! objc2 directly.

use std::path::PathBuf;

use objc2_foundation::{NSBundle, NSString, NSURL};

/// Absolute path of the app bundle's resource directory (the bundle
/// root for flat iOS bundles). `None` outside a bundle context.
pub fn resource_path() -> Option<PathBuf> {
    let bundle = NSBundle::mainBundle();
    let path = bundle.resourcePath()?;
    Some(PathBuf::from(path.to_string()))
}

/// Paint the app window itself. The window shows through wherever
/// the content root's safe-area padding insets the user's views
/// (status bar, home indicator) — painting it lets an app's
/// background reach the physical display edges.
pub fn set_window_background(color: crate::dom::Color) {
    if let Some(window) = crate::dom::app::main_window() {
        window.setBackgroundColor(Some(&color.to_uicolor()));
    }
}

/// Open `url` with `UIApplication` (Safari for http/https). Invalid
/// URLs are ignored.
pub fn open_url(url: &str) {
    use objc2_ui_kit::UIApplication;

    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    let ns = NSString::from_str(url);
    let Some(nsurl) = NSURL::URLWithString(&ns) else {
        return;
    };
    let app = UIApplication::sharedApplication(mtm);
    unsafe {
        app.openURL_options_completionHandler(
            &nsurl,
            &objc2_foundation::NSDictionary::new(),
            None,
        );
    }
}
