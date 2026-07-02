//! Event-handler installation for GTK widgets.
//!
//! Mirrors `cocoa_dom::event` in shape but is much simpler: each
//! `connect_*` returns a handler closure owned by the GTK signal
//! connection itself (which is owned by the widget). When the widget
//! drops, all its handler closures drop with it. No thread-local
//! handler store.
//!
//! Multiple handlers stack: every `on_click` call appends a new
//! `clicked` connection. Cocoa overwrites; GTK doesn't. Nothing in
//! the rest of the port relies on the single-handler limitation.

use gtk4::prelude::*;
use std::cell::RefCell;

/// Wire a click handler. No-op if `widget` isn't a `gtk::Button`-
/// flavoured control (or a `CheckButton`/`DropDown` — see
/// [`on_action`] for those).
pub fn on_click(widget: &gtk4::Widget, cb: impl FnMut() + 'static) {
    if let Some(button) = widget.downcast_ref::<gtk4::Button>() {
        let cb = RefCell::new(cb);
        button.connect_clicked(move |_| {
            if let Ok(mut cb) = cb.try_borrow_mut() {
                cb();
            } else {
                eprintln!("[gtk_dom] reentrant click handler skipped");
            }
        });
        return;
    }
    // CheckButton "toggled" reads naturally as a click.
    if let Some(check) = widget.downcast_ref::<gtk4::CheckButton>() {
        let cb = RefCell::new(cb);
        check.connect_toggled(move |_| {
            if let Ok(mut cb) = cb.try_borrow_mut() {
                cb();
            }
        });
        return;
    }
    // DropDown selection change presents like a click in our model.
    if let Some(dd) = widget.downcast_ref::<gtk4::DropDown>() {
        let cb = RefCell::new(cb);
        dd.connect_selected_notify(move |_| {
            if let Ok(mut cb) = cb.try_borrow_mut() {
                cb();
            }
        });
    }
}

/// Wire a generic "value changed" handler. For sliders (Scale),
/// dropdowns, checkboxes — the cross-control equivalent of
/// `on_click`.
pub fn on_action(widget: &gtk4::Widget, cb: impl FnMut() + 'static) {
    let cb = std::sync::Arc::new(std::sync::Mutex::new(cb));

    if let Some(scale) = widget.downcast_ref::<gtk4::Scale>() {
        let cb = cb.clone();
        scale.connect_value_changed(move |_| {
            if let Ok(mut cb) = cb.try_lock() {
                cb();
            }
        });
        return;
    }
    if let Some(check) = widget.downcast_ref::<gtk4::CheckButton>() {
        let cb = cb.clone();
        check.connect_toggled(move |_| {
            if let Ok(mut cb) = cb.try_lock() {
                cb();
            }
        });
        return;
    }
    if let Some(dd) = widget.downcast_ref::<gtk4::DropDown>() {
        dd.connect_selected_notify(move |_| {
            if let Ok(mut cb) = cb.try_lock() {
                cb();
            }
        });
        return;
    }
    if let Some(button) = widget.downcast_ref::<gtk4::Button>() {
        button.connect_clicked(move |_| {
            if let Ok(mut cb) = cb.try_lock() {
                cb();
            }
        });
    }
}

/// Wire a callback that fires whenever the text content of an entry
/// changes. No-op on non-Entry widgets.
pub fn on_text_change(widget: &gtk4::Widget, cb: impl FnMut(String) + 'static) {
    if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
        let cb = RefCell::new(cb);
        entry.connect_changed(move |e| {
            if let Ok(mut cb) = cb.try_borrow_mut() {
                cb(e.text().to_string());
            }
        });
        return;
    }
    if let Some(entry) = widget.downcast_ref::<gtk4::PasswordEntry>() {
        let cb = RefCell::new(cb);
        entry.connect_changed(move |e| {
            if let Ok(mut cb) = cb.try_borrow_mut() {
                cb(e.text().to_string());
            }
        });
    }
}

/// Wire a callback that fires when the user commits an edit (Return
/// key, focus loss). No-op on non-entry widgets.
///
/// Uses `connect_activate` for the Return-key path. Focus-loss
/// commits aren't currently fired (would need a focus controller);
/// add when needed.
pub fn on_text_end_editing(
    widget: &gtk4::Widget,
    cb: impl FnMut(String) + 'static,
) {
    if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
        let cb = RefCell::new(cb);
        entry.connect_activate(move |e| {
            if let Ok(mut cb) = cb.try_borrow_mut() {
                cb(e.text().to_string());
            }
        });
        return;
    }
    if let Some(entry) = widget.downcast_ref::<gtk4::PasswordEntry>() {
        let cb = RefCell::new(cb);
        entry.connect_activate(move |e| {
            if let Ok(mut cb) = cb.try_borrow_mut() {
                cb(e.text().to_string());
            }
        });
    }
}

/// Wire a focus-gained callback. Uses an `EventControllerFocus`
/// attached to the widget. Multiple installs stack.
pub fn on_text_focus(widget: &gtk4::Widget, cb: impl FnMut() + 'static) {
    let controller = gtk4::EventControllerFocus::new();
    let cb = RefCell::new(cb);
    controller.connect_enter(move |_| {
        if let Ok(mut cb) = cb.try_borrow_mut() {
            cb();
        }
    });
    widget.add_controller(controller);
}

/// Wire a focus-lost callback. Uses an `EventControllerFocus`.
pub fn on_text_blur(widget: &gtk4::Widget, cb: impl FnMut() + 'static) {
    let controller = gtk4::EventControllerFocus::new();
    let cb = RefCell::new(cb);
    controller.connect_leave(move |_| {
        if let Ok(mut cb) = cb.try_borrow_mut() {
            cb();
        }
    });
    widget.add_controller(controller);
}

// ---------------------------------------------------------------------
// Event — the payload delivered to `on:` handlers (moved here from the
// old `dom/renderer.rs` stub module).
// ---------------------------------------------------------------------

use send_wrapper::SendWrapper;
use std::fmt;

/// A GTK event delivered to a handler. Currently a placeholder
/// wrapper around an optional `gdk::Event`.
#[derive(Clone)]
pub struct Event {
    inner: Option<SendWrapper<gtk4::gdk::Event>>,
}

impl Event {
    pub fn new(ev: gtk4::gdk::Event) -> Self {
        Event {
            inner: Some(SendWrapper::new(ev)),
        }
    }

    pub fn synthetic() -> Self {
        Event { inner: None }
    }

    pub fn gdk_event(&self) -> Option<&gtk4::gdk::Event> {
        self.inner.as_deref()
    }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Event")
            .field("has_gdk_event", &self.inner.is_some())
            .finish()
    }
}
