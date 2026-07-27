//! Modal file open/save panels — thin wrappers over NSOpenPanel /
//! NSSavePanel.
//!
//! Both helpers are **blocking**: `runModal` spins its own event
//! loop until the user confirms or cancels, then the function
//! returns. Call them from event handlers (button clicks, menu
//! actions) on the main thread; they panic off-main like the rest
//! of the port.
//!
//! Extension filtering uses the deprecated `setAllowedFileTypes:`
//! rather than `setAllowedContentTypes:` — the UTType-based API
//! needs the `objc2-uniform-type-identifiers` crate, which the
//! workspace doesn't depend on. `allowedFileTypes` accepts plain
//! extension strings, still works on current macOS, and keeps the
//! dependency surface unchanged. Swap to UTType if/when that crate
//! joins the workspace.

use objc2::{rc::Retained, MainThreadMarker};
use objc2_app_kit::{NSModalResponseOK, NSOpenPanel, NSSavePanel};
use objc2_foundation::{NSArray, NSString};
use std::path::PathBuf;

fn mtm(ctx: &'static str) -> MainThreadMarker {
    MainThreadMarker::new()
        .unwrap_or_else(|| panic!("{ctx} must run on the main thread"))
}

/// Apply the extension filter to a panel (NSOpenPanel IS-A
/// NSSavePanel, so one helper serves both). An empty list means
/// "allow everything" — we leave the panel's default (nil) in
/// place, since an empty `allowedFileTypes` array raises an ObjC
/// exception.
fn apply_allowed_extensions(panel: &NSSavePanel, extensions: &[&str]) {
    if extensions.is_empty() {
        return;
    }
    let types: Vec<Retained<NSString>> =
        extensions.iter().map(|e| NSString::from_str(e)).collect();
    let array = NSArray::from_retained_slice(&types);
    #[allow(deprecated)] // see module docs — UTType isn't in the workspace.
    panel.setAllowedFileTypes(Some(&array));
}

/// Read the panel's chosen URL back as a filesystem path.
fn panel_path(panel: &NSSavePanel) -> Option<PathBuf> {
    let url = panel.URL()?;
    let path = url.path()?;
    Some(PathBuf::from(path.to_string()))
}

/// Run a modal save panel. Returns the chosen path, or None if cancelled.
/// `default_name` pre-fills the filename field. Main thread only.
pub fn save_panel(
    default_name: &str,
    allowed_extensions: &[&str],
) -> Option<PathBuf> {
    let mtm = mtm("save_panel");
    let panel = NSSavePanel::savePanel(mtm);
    panel.setNameFieldStringValue(&NSString::from_str(default_name));
    apply_allowed_extensions(&panel, allowed_extensions);
    if panel.runModal() == NSModalResponseOK {
        panel_path(&panel)
    } else {
        None
    }
}

/// Run a modal open panel restricted to the given extensions. Main thread only.
pub fn open_panel(allowed_extensions: &[&str]) -> Option<PathBuf> {
    let mtm = mtm("open_panel");
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    apply_allowed_extensions(&panel, allowed_extensions);
    if panel.runModal() == NSModalResponseOK {
        panel_path(&panel)
    } else {
        None
    }
}
