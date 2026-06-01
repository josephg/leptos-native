//! `GtkElem` — the GTK port's view handle, now a thin alias for the
//! renderer-agnostic [`Node<GtkBackend>`].
//!
//! The handle itself (a `Copy + Send` generational [`NodeId`]) and its
//! generic accessor surface (`id`, `view`, `with_style`, `with_tag`,
//! `ptr_eq`, …) live in core (`leptos_native::renderer::node`). This file
//! supplies the **GTK-specific** widget operations via the [`GtkNodeExt`]
//! extension trait — inherent methods on `Node<GtkBackend>` aren't possible
//! from this crate (inherent impls must live in the defining crate), so a
//! local trait carries them. `impl GtkNodeExt for GtkElem` is orphan-safe:
//! the trait is local even though `Node` is foreign.

use crate::dom::layout::{GtkBackend, NodeId, Style};
use crate::dom::taffy_layout::TaffyLayout;
use crate::dom::{event, layout};
use gtk4::glib;
use gtk4::prelude::*;
use leptos_native::renderer::{LayoutBackend, Node};

/// The GTK port's node handle: a [`Node`] tagged with [`GtkBackend`].
/// Generic methods come from [`Node`]; GTK widget ops from [`GtkNodeExt`].
pub type GtkElem = Node<GtkBackend>;

/// GTK-specific widget operations on a [`GtkElem`]. Bring this trait into
/// scope to call `.set_title()`, `.on_click()`, `GtkElem::create_text()`,
/// etc. (The generic `.id()` / `.view()` / `.with_style()` surface needs no
/// import — it's inherent on [`Node`].)
pub trait GtkNodeExt: Copy {
    fn new_from_widget<W>(widget: W, default_style: Style) -> Self
    where
        W: IsA<gtk4::Widget>;
    fn widget(self) -> gtk4::Widget;
    fn try_widget(self) -> Option<gtk4::Widget>;
    fn into_widget(self) -> gtk4::Widget;
    fn teardown(self);
    fn create_container() -> Self;
    fn insert_node(self, child: GtkElem, marker: Option<GtkElem>);
    fn try_insert_node(self, child: GtkElem, marker: Option<GtkElem>) -> bool;
    fn remove_child(self, child: GtkElem) -> Option<GtkElem>;
    fn clear_children(self);
    fn set_title(self, value: &str);
    fn set_value(self, value: &str);
    fn set_placeholder(self, value: &str);
    fn set_enabled(self, value: bool);
    fn set_checked(self, value: bool);
    fn on_click(self, cb: impl FnMut() + 'static);
    fn on_action(self, cb: impl FnMut() + 'static);
    fn on_value_change(self, cb: impl FnMut() + Send + 'static);
    fn on_text_change(self, cb: impl FnMut(String) + 'static);
    fn on_text_end_editing(self, cb: impl FnMut(String) + 'static);
    fn on_text_focus(self, cb: impl FnMut() + 'static);
    fn on_text_blur(self, cb: impl FnMut() + 'static);
    fn checked(self) -> bool;
    fn double_value(self) -> f64;
    fn set_double_value(self, v: f64);
    fn set_slider_min(self, v: f64);
    fn set_slider_max(self, v: f64);
    fn set_popup_items(self, items: &[String]);
    fn popup_selection(self) -> u32;
    fn set_popup_selection(self, idx: u32);
    fn focus(self) -> bool;
    fn blur(self) -> bool;
    fn create_text(content: &str) -> Self;
    fn set_text(self, content: &str);
    fn create_placeholder() -> Self;
}

impl GtkNodeExt for GtkElem {
    /// Typed registration primitive: hand in a concrete gtk widget, get
    /// back a `Node`.
    fn new_from_widget<W>(widget: W, default_style: Style) -> Self
    where
        W: IsA<gtk4::Widget>,
    {
        let widget: gtk4::Widget = widget.upcast();
        GtkElem::from_id(GtkBackend::new_leaf(default_style, widget, (), ()))
    }

    /// The underlying `gtk::Widget` (owned clone — cheap gobject refcount
    /// bump). Main-thread only. Panics if the node is gone.
    fn widget(self) -> gtk4::Widget {
        self.view()
    }

    /// `Some(widget)` if the node is still in the store, else `None`.
    /// Setters resolve their widget through this (not the panicking
    /// `widget()`) so a reactive effect that fires after teardown is a
    /// graceful no-op.
    fn try_widget(self) -> Option<gtk4::Widget> {
        self.try_view()
    }

    /// Get a fresh `gtk4::Widget` clone — same as [`Self::widget`].
    fn into_widget(self) -> gtk4::Widget {
        self.widget()
    }

    /// Remove this node (and its structural subtree) from the store and
    /// unparent its widget.
    fn teardown(self) {
        if let Some(w) = self.try_widget() {
            if let Some(parent) = w.parent() {
                detach_child_widget(&parent, &w);
            }
        }
        GtkBackend::remove(self.id);
    }

    /// Generic flexbox container (gtk::Box-backed).
    fn create_container() -> Self {
        GtkElem::new_from_widget(container_widget(), Style::default()).with_tag("container")
    }

    /// Insert `child` before `marker`; if `marker` is `None`, append.
    fn insert_node(self, child: GtkElem, marker: Option<GtkElem>) {
        let _ = self.try_insert_node(child, marker);
    }

    /// Try to insert `child` before `marker`. Returns `false` if the
    /// parent isn't a supported container, or `marker` isn't its child.
    fn try_insert_node(self, child: GtkElem, marker: Option<GtkElem>) -> bool {
        let parent_w = self.widget();
        let parent: &gtk4::Widget = &parent_w;
        let child_w = child.widget();
        let child_widget: &gtk4::Widget = &child_w;

        // Self-parent? Reject.
        if child_widget.as_ptr() == parent.as_ptr() {
            return false;
        }

        // Window parents use `set_child` (single child).
        if let Some(window) = parent.downcast_ref::<gtk4::ApplicationWindow>() {
            if marker.is_some() {
                return false;
            }
            window.set_child(Some(child_widget));
            return true;
        }
        if let Some(window) = parent.downcast_ref::<gtk4::Window>() {
            if marker.is_some() {
                return false;
            }
            window.set_child(Some(child_widget));
            return true;
        }

        // Generic container path.
        match child_widget.parent() {
            Some(p) if p.as_ptr() == parent.as_ptr() => match marker {
                None => {
                    child_widget.insert_before(parent, None::<&gtk4::Widget>);
                }
                Some(m) => {
                    let m_widget = m.widget();
                    if m_widget.as_ptr() == child_widget.as_ptr() {
                        return true;
                    }
                    if m_widget.parent().map(|p| p.as_ptr()) != Some(parent.as_ptr()) {
                        return false;
                    }
                    child_widget.insert_before(parent, Some(&m_widget));
                }
            },
            Some(_) => {
                child_widget.unparent();
                attach_under(parent, child_widget, marker);
            }
            None => {
                attach_under(parent, child_widget, marker);
            }
        }

        // Mirror into Taffy by marker — no native-order readback.
        // (Canary: was `child_index_in_parent` + `insert_child_at` /
        // `attach_child`. We now drive both trees from the same marker.)
        layout::insert_child_before(self, child, marker);
        true
    }

    /// Remove `child` from this element's child list.
    fn remove_child(self, child: GtkElem) -> Option<GtkElem> {
        let parent_w = self.widget();
        let parent: &gtk4::Widget = &parent_w;
        let child_w = child.widget();
        let child_widget: &gtk4::Widget = &child_w;
        let child_parent = child_widget.parent()?;
        if child_parent.as_ptr() != parent.as_ptr() {
            return None;
        }
        detach_child_widget(parent, child_widget);
        layout::detach_child(self, child);
        Some(child)
    }

    /// Remove every child.
    fn clear_children(self) {
        let parent_w = self.widget();
        let parent: &gtk4::Widget = &parent_w;
        if let Some(window) = parent.downcast_ref::<gtk4::ApplicationWindow>() {
            window.set_child(None::<&gtk4::Widget>);
            return;
        }
        if let Some(window) = parent.downcast_ref::<gtk4::Window>() {
            window.set_child(None::<&gtk4::Widget>);
            return;
        }
        while let Some(child) = parent.first_child() {
            child.unparent();
        }
    }

    /// Set the title on a button-flavoured widget. No-op otherwise.
    fn set_title(self, value: &str) {
        let Some(widget) = self.try_widget() else { return; };
        let mut changed = false;
        if let Some(button) = widget.downcast_ref::<gtk4::Button>() {
            if button.label().as_deref() != Some(value) {
                button.set_label(value);
                changed = true;
            }
        } else if let Some(check) = widget.downcast_ref::<gtk4::CheckButton>() {
            if check.label().as_deref() != Some(value) {
                check.set_label(Some(value));
                changed = true;
            }
        } else if let Some(label) = widget.downcast_ref::<gtk4::Label>() {
            if label.label().as_str() != value {
                label.set_label(value);
                changed = true;
            }
        }
        if changed {
            layout::schedule_relayout(self);
        }
    }

    /// Set the value/text on an `Entry` / `PasswordEntry` / `Label`.
    fn set_value(self, value: &str) {
        let Some(widget) = self.try_widget() else { return; };
        let mut changed = false;
        if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
            if entry.text().as_str() != value {
                entry.set_text(value);
                changed = true;
            }
        } else if let Some(entry) = widget.downcast_ref::<gtk4::PasswordEntry>() {
            if entry.text().as_str() != value {
                entry.set_text(value);
                changed = true;
            }
        } else if let Some(label) = widget.downcast_ref::<gtk4::Label>() {
            if label.label().as_str() != value {
                label.set_label(value);
                changed = true;
            }
        }
        if changed {
            layout::schedule_relayout(self);
        }
    }

    /// Set placeholder text on an `Entry` / `PasswordEntry`.
    fn set_placeholder(self, value: &str) {
        let Some(widget) = self.try_widget() else { return; };
        let mut changed = false;
        if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
            if entry.placeholder_text().as_deref() != Some(value) {
                entry.set_placeholder_text(Some(value));
                changed = true;
            }
        } else if let Some(entry) = widget.downcast_ref::<gtk4::PasswordEntry>() {
            if entry.placeholder_text().as_deref() != Some(value) {
                entry.set_placeholder_text(Some(value));
                changed = true;
            }
        }
        if changed {
            layout::schedule_relayout(self);
        }
    }

    /// Toggle widget sensitivity (gtk's "enabled").
    fn set_enabled(self, value: bool) {
        let Some(widget) = self.try_widget() else { return; };
        if widget.is_sensitive() != value {
            widget.set_sensitive(value);
        }
    }

    /// Set the on/off state on a `gtk::CheckButton`. No-op otherwise.
    fn set_checked(self, value: bool) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(check) = widget.downcast_ref::<gtk4::CheckButton>() {
            if check.is_active() != value {
                check.set_active(value);
            }
        }
    }

    fn on_click(self, cb: impl FnMut() + 'static) {
        event::on_click(&self.widget(), cb);
    }

    fn on_action(self, cb: impl FnMut() + 'static) {
        event::on_action(&self.widget(), cb);
    }

    fn on_value_change(self, mut cb: impl FnMut() + Send + 'static) {
        let Some(widget) = self.try_widget() else { return; };
        if widget.downcast_ref::<gtk4::Entry>().is_some() {
            event::on_text_change(&widget, move |_| cb());
            return;
        }
        if widget.downcast_ref::<gtk4::PasswordEntry>().is_some() {
            event::on_text_change(&widget, move |_| cb());
            return;
        }
        event::on_action(&widget, cb);
    }

    fn on_text_change(self, cb: impl FnMut(String) + 'static) {
        event::on_text_change(&self.widget(), cb);
    }

    fn on_text_end_editing(self, cb: impl FnMut(String) + 'static) {
        event::on_text_end_editing(&self.widget(), cb);
    }

    fn on_text_focus(self, cb: impl FnMut() + 'static) {
        event::on_text_focus(&self.widget(), cb);
    }

    fn on_text_blur(self, cb: impl FnMut() + 'static) {
        event::on_text_blur(&self.widget(), cb);
    }

    fn checked(self) -> bool {
        self.widget()
            .downcast_ref::<gtk4::CheckButton>()
            .map(|c| c.is_active())
            .unwrap_or(false)
    }

    fn double_value(self) -> f64 {
        self.widget()
            .downcast_ref::<gtk4::Scale>()
            .map(|s| s.value())
            .unwrap_or(0.0)
    }

    fn set_double_value(self, v: f64) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(s) = widget.downcast_ref::<gtk4::Scale>() {
            if (s.value() - v).abs() > f64::EPSILON {
                s.set_value(v);
            }
        }
    }

    fn set_slider_min(self, v: f64) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(s) = widget.downcast_ref::<gtk4::Scale>() {
            s.adjustment().set_lower(v);
        }
    }

    fn set_slider_max(self, v: f64) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(s) = widget.downcast_ref::<gtk4::Scale>() {
            s.adjustment().set_upper(v);
        }
    }

    fn set_popup_items(self, items: &[String]) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(dd) = widget.downcast_ref::<gtk4::DropDown>() {
            let model = gtk4::StringList::new(&[]);
            for it in items {
                model.append(it);
            }
            dd.set_model(Some(&model));
        }
    }

    /// Currently-selected index on a `gtk::DropDown` (0 otherwise).
    fn popup_selection(self) -> u32 {
        self.widget()
            .downcast_ref::<gtk4::DropDown>()
            .map(|dd| dd.selected())
            .unwrap_or(0)
    }

    fn set_popup_selection(self, idx: u32) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(dd) = widget.downcast_ref::<gtk4::DropDown>() {
            if dd.selected() != idx {
                dd.set_selected(idx);
            }
        }
    }

    /// Grab focus.
    fn focus(self) -> bool {
        let Some(widget) = self.try_widget() else { return false; };
        widget.grab_focus()
    }

    /// Resign focus.
    fn blur(self) -> bool {
        let Some(widget) = self.try_widget() else { return false; };
        if let Some(root) = widget.root() {
            root.set_focus(None::<&gtk4::Widget>);
            true
        } else {
            false
        }
    }

    /// Build a text-label Node — a `gtk::Label` configured for
    /// left-aligned word-wrap.
    fn create_text(content: &str) -> Self {
        let label = gtk4::Label::new(Some(content));
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        let mut style = Style::default();
        style.flex_shrink = 0.0;
        GtkElem::new_from_widget(label, style).with_tag("#text")
    }

    /// Update the displayed string on a text-label Node.
    fn set_text(self, content: &str) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(label) = widget.downcast_ref::<gtk4::Label>() {
            if label.label().as_str() != content {
                label.set_label(content);
                crate::dom::layout::schedule_relayout(self);
            }
        }
    }

    /// Build a placeholder Node — a hidden, zero-sized `gtk::Label`.
    fn create_placeholder() -> Self {
        let widget = gtk4::Label::new(None::<&str>);
        widget.set_visible(false);

        let mut style = Style::default();
        style.position = crate::dom::layout::Position::Absolute;
        style.size.width = crate::dom::layout::Dimension::length(0.0);
        style.size.height = crate::dom::layout::Dimension::length(0.0);

        GtkElem::new_from_widget(widget, style).with_tag("placeholder")
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

use crate::dom::make_view::container_widget;

/// Detach `child` from `parent`, using the correct GTK4 API for the
/// parent's child model.
///
/// Most containers (GtkBox and friends) accept a bare
/// `child.unparent()`. But single-child containers that track their
/// child via `set_child` — GtkWindow / GtkApplicationWindow, and
/// GtkOverlay's *main* child — keep an internal child pointer that a
/// direct `unparent()` does NOT clear. If the child is then freed
/// (e.g. its `Node` is removed from the store), that pointer dangles
/// and the parent double-disposes the freed child when it finalizes —
/// surfacing as `gtk_widget_unparent: assertion 'GTK_IS_WIDGET'
/// failed` at window/overlay teardown. Detach through the owning API
/// for those parents instead.
fn detach_child_widget(parent: &gtk4::Widget, child: &gtk4::Widget) {
    if let Some(win) = parent.downcast_ref::<gtk4::ApplicationWindow>() {
        win.set_child(None::<&gtk4::Widget>);
    } else if let Some(win) = parent.downcast_ref::<gtk4::Window>() {
        win.set_child(None::<&gtk4::Widget>);
    } else if let Some(overlay) = parent.downcast_ref::<gtk4::Overlay>() {
        // The main child is owned via set_child; overlay layers added
        // with `add_overlay` are removed via `remove_overlay`.
        if overlay.child().map(|c| c.as_ptr()) == Some(child.as_ptr()) {
            overlay.set_child(None::<&gtk4::Widget>);
        } else {
            overlay.remove_overlay(child);
        }
    } else {
        child.unparent();
    }
}

/// Append or insert `child` under `parent`.
fn attach_under(parent: &gtk4::Widget, child: &gtk4::Widget, marker: Option<GtkElem>) {
    if let Some(box_) = parent.downcast_ref::<gtk4::Box>() {
        match marker {
            None => box_.append(child),
            Some(m) => {
                let m_widget = m.widget();
                let prev = m_widget.prev_sibling();
                box_.insert_child_after(child, prev.as_ref());
            }
        }
    } else {
        child.set_parent(parent);
        if let Some(m) = marker {
            child.insert_before(parent, Some(&m.widget()));
        }
    }
}

/// Install our [`TaffyLayout`] on `widget` so its layout is driven by
/// the ambient store. Idempotent.
pub fn install_taffy_layout_for_container(widget: &gtk4::Widget, node_id: NodeId, is_root: bool) {
    if widget
        .layout_manager()
        .map(|lm| lm.is::<TaffyLayout>())
        .unwrap_or(false)
    {
        return;
    }
    let lm = TaffyLayout::new(node_id, is_root);
    widget.set_layout_manager(Some(lm));
}

/// Returns whether the given widget is a container.
pub fn is_container_widget(widget: &gtk4::Widget) -> bool {
    widget.is::<gtk4::Box>()
}

#[allow(dead_code)]
fn _unused() {
    let _ = glib::value::Value::from(0i32);
}
