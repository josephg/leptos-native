//! `GtkNode` — a `Copy` handle (just a `NodeId`) into the ambient
//! thread-local node store.
//!
//! There is no `Rc`, no `SendWrapper`, no refcount: a `Node` is a bare
//! generational `NodeId` (`Copy + Send`). The backing `gtk::Widget` and
//! Taffy style live in the per-thread `LayoutState<GtkBackend>` (see
//! [`crate::layout`] and `renderer::scene`). Accessors fetch through
//! the store by id; a stale id resolves to `None`/no-op via the
//! generational key.
//!
//! Lifecycle is explicit: [`GtkNode::teardown`] removes the node and its
//! structural subtree from the store. Mirrors the cocoa port — see
//! `cocoa/dom/src/node.rs` for the longer rationale.
//!
//! GtkNode is a newtype wrapper around NodeId to add GTK-specific methods.

use crate::dom::layout::{GtkBackend, NodeId, Style};
use crate::dom::taffy_layout::TaffyLayout;
use gtk4::glib;
use gtk4::prelude::*;
use renderer::LayoutBackend;
use crate::dom::{event, layout};

/// A handle into the ambient node store — structurally just a
/// generational [`NodeId`]. `Copy + Send`.
///
/// All per-node state (the `gtk::Widget`, Taffy style) lives in
/// `LayoutState<GtkBackend>`; accessors read through the store keyed
/// by `id`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GtkNode {
    pub(crate) id: NodeId,
}

impl GtkNode {
    /// Typed registration primitive: hand in a concrete gtk widget,
    /// get back a `Node`.
    pub fn new_from_widget<W>(widget: W, default_style: Style) -> Self
    where
        W: IsA<gtk4::Widget>,
    {
        let widget: gtk4::Widget = widget.upcast();
        let id = GtkBackend::new_leaf(default_style, widget, (), ());
        GtkNode { id }
    }

    /// Wrap an existing store id as a `Node`.
    pub fn from_id(id: NodeId) -> Self {
        GtkNode { id }
    }

    /// The node's `NodeId`.
    pub fn id(self) -> NodeId {
        self.id
    }

    /// Record the element kind for debug tooling (devtools inspector).
    /// Cheap and harmless in release; set once at construction.
    pub fn with_tag(self, tag: &'static str) -> Self {
        GtkBackend::set_debug_tag_name(self.id, tag);
        self
    }

    /// The underlying `gtk::Widget` (owned clone — cheap gobject
    /// refcount bump). Main-thread only. Panics if the node is gone.
    pub fn widget(self) -> gtk4::Widget {
        GtkBackend::view(self.id).expect("Node id must exist in the store")
    }

    /// `Some(widget)` if the node is still in the store, else `None`.
    ///
    /// Setters resolve their widget through this (not the panicking
    /// `widget()`) so a reactive effect that fires *after* the node was
    /// torn down is a graceful no-op rather than a panic. Under the
    /// `Copy`-`NodeId` model a `RenderEffect` closure captures only the
    /// id (it pins nothing), so an async-scheduled effect re-run can
    /// outlive its node.
    ///
    /// This is **defense-in-depth, not the primary fix**. The real fix
    /// is that `ElementState::unmount` drops `_effects` before tearing
    /// the node down (see `leptos_gtk::gtk::element`), which ends the
    /// effects' driver futures so they can't re-run on a freed node. We
    /// keep this guard anyway as a uniform safety net (and because it
    /// matches the web backend, where setting an attribute on a
    /// detached-but-alive node is harmless). Trade-off: a future
    /// regression of the unmount cleanup is swallowed silently here.
    pub fn try_widget(self) -> Option<gtk4::Widget> {
        GtkBackend::view(self.id)
    }

    /// Get a fresh `gtk4::Widget` clone — same as [`Self::widget`].
    pub fn into_widget(self) -> gtk4::Widget {
        self.widget()
    }

    // ---- Accessor surface ------------------------------------------

    pub fn with_style<R>(self, f: impl FnOnce(&Style) -> R) -> R {
        let style = GtkBackend::style(self.id).unwrap_or_default();
        f(&style)
    }

    pub fn with_style_mut<R>(self, f: impl FnOnce(&mut Style) -> R) -> R {
        let mut style = GtkBackend::style(self.id).unwrap_or_default();
        let r = f(&mut style);
        GtkBackend::set_style(self.id, style);
        r
    }

    /// Pointer-equality check. Each node owns exactly one widget, so id
    /// equality is equivalent to underlying-gobject equality.
    pub fn ptr_eq(self, other: &GtkNode) -> bool {
        self.id == other.id
    }

    /// Remove this node (and its structural subtree) from the store and
    /// unparent its widget.
    pub fn teardown(self) {
        if let Some(w) = self.try_widget() {
            if let Some(parent) = w.parent() {
                detach_child_widget(&parent, &w);
            }
        }
        GtkBackend::remove(self.id);
    }

    /// Identity. Kept so `el.as_node()` call sites compile.
    pub fn as_node(&self) -> &GtkNode {
        self
    }

    /// Identity. See [`Self::as_node`].
    pub fn into_node(self) -> GtkNode {
        self
    }

    /// Generic flexbox container (gtk::Box-backed).
    pub fn create_container() -> Self {
        GtkNode::new_from_widget(container_widget(), Style::default()).with_tag("container")
    }

    /// Insert `child` before `marker`; if `marker` is `None`, append.
    pub fn insert_node(self, child: &GtkNode, marker: Option<&GtkNode>) {
        let _ = self.try_insert_node(child, marker);
    }

    /// Try to insert `child` before `marker`. Returns `false` if the
    /// parent isn't a supported container, or `marker` isn't its child.
    pub fn try_insert_node(
        self,
        child: &GtkNode,
        marker: Option<&GtkNode>,
    ) -> bool {
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
            Some(p) if p.as_ptr() == parent.as_ptr() => {
                match marker {
                    None => {
                        child_widget.insert_before(parent, None::<&gtk4::Widget>);
                    }
                    Some(m) => {
                        let m_widget = m.widget();
                        if m_widget.as_ptr() == child_widget.as_ptr() {
                            return true;
                        }
                        if m_widget.parent().map(|p| p.as_ptr())
                            != Some(parent.as_ptr())
                        {
                            return false;
                        }
                        child_widget.insert_before(parent, Some(&m_widget));
                    }
                }
            }
            Some(_) => {
                child_widget.unparent();
                attach_under(parent, child_widget, marker);
            }
            None => {
                attach_under(parent, child_widget, marker);
            }
        }

        // Mirror into Taffy at the right index.
        let idx = child_index_in_parent(parent, child_widget);
        if let Some(idx) = idx {
            layout::insert_child_at(self, *child, idx);
        } else {
            layout::attach_child(self, *child);
        }
        true
    }

    /// Remove `child` from this element's child list.
    pub fn remove_child(self, child: &GtkNode) -> Option<GtkNode> {
        let parent_w = self.widget();
        let parent: &gtk4::Widget = &parent_w;
        let child_w = child.widget();
        let child_widget: &gtk4::Widget = &child_w;
        let child_parent = child_widget.parent()?;
        if child_parent.as_ptr() != parent.as_ptr() {
            return None;
        }
        detach_child_widget(parent, child_widget);
        layout::detach_child(self, *child);
        Some(*child)
    }

    /// Remove every child.
    pub fn clear_children(self) {
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
    pub fn set_title(self, value: &str) {
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
    pub fn set_value(self, value: &str) {
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
    pub fn set_placeholder(self, value: &str) {
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

    /// Toggle widget visibility.
    pub fn set_hidden(self, value: bool) {
        let Some(widget) = self.try_widget() else { return; };
        if widget.is_visible() == value {
            widget.set_visible(!value);
        }
    }

    /// Toggle widget sensitivity (gtk's "enabled").
    pub fn set_enabled(self, value: bool) {
        let Some(widget) = self.try_widget() else { return; };
        if widget.is_sensitive() != value {
            widget.set_sensitive(value);
        }
    }

    /// Set the on/off state on a `gtk::CheckButton`. No-op otherwise.
    pub fn set_checked(self, value: bool) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(check) = widget.downcast_ref::<gtk4::CheckButton>() {
            if check.is_active() != value {
                check.set_active(value);
            }
        }
    }

    // ---- event hooks (delegate to crate::event) ----

    pub fn on_click(self, cb: impl FnMut() + 'static) {
        event::on_click(&self.widget(), cb);
    }

    pub fn on_action(self, cb: impl FnMut() + 'static) {
        event::on_action(&self.widget(), cb);
    }

    pub fn on_value_change(self, mut cb: impl FnMut() + Send + 'static) {
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

    pub fn on_text_change(self, cb: impl FnMut(String) + 'static) {
        event::on_text_change(&self.widget(), cb);
    }

    pub fn on_text_end_editing(self, cb: impl FnMut(String) + 'static) {
        event::on_text_end_editing(&self.widget(), cb);
    }

    pub fn on_text_focus(self, cb: impl FnMut() + 'static) {
        event::on_text_focus(&self.widget(), cb);
    }

    pub fn on_text_blur(self, cb: impl FnMut() + 'static) {
        event::on_text_blur(&self.widget(), cb);
    }

    // ---- value accessors ----

    pub fn checked(self) -> bool {
        self.widget()
            .downcast_ref::<gtk4::CheckButton>()
            .map(|c| c.is_active())
            .unwrap_or(false)
    }

    pub fn double_value(self) -> f64 {
        self.widget()
            .downcast_ref::<gtk4::Scale>()
            .map(|s| s.value())
            .unwrap_or(0.0)
    }

    pub fn set_double_value(self, v: f64) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(s) = widget.downcast_ref::<gtk4::Scale>() {
            if (s.value() - v).abs() > f64::EPSILON {
                s.set_value(v);
            }
        }
    }

    pub fn set_slider_min(self, v: f64) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(s) = widget.downcast_ref::<gtk4::Scale>() {
            s.adjustment().set_lower(v);
        }
    }

    pub fn set_slider_max(self, v: f64) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(s) = widget.downcast_ref::<gtk4::Scale>() {
            s.adjustment().set_upper(v);
        }
    }

    pub fn set_popup_items(self, items: &[String]) {
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
    pub fn popup_selection(self) -> u32 {
        self.widget()
            .downcast_ref::<gtk4::DropDown>()
            .map(|dd| dd.selected())
            .unwrap_or(0)
    }

    pub fn set_popup_selection(self, idx: u32) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(dd) = widget.downcast_ref::<gtk4::DropDown>() {
            if dd.selected() != idx {
                dd.set_selected(idx);
            }
        }
    }

    /// Set this view's opacity (0.0..=1.0).
    pub fn set_alpha(self, alpha: f64) {
        let Some(w) = self.try_widget() else { return; };
        let clamped = alpha.clamp(0.0, 1.0);
        if (w.opacity() - clamped).abs() > f64::EPSILON {
            w.set_opacity(clamped);
        }
    }

    /// Set this view's tooltip text. Empty string clears it.
    pub fn set_tool_tip(self, tip: &str) {
        let Some(w) = self.try_widget() else { return; };
        if tip.is_empty() {
            w.set_tooltip_text(None);
        } else {
            w.set_tooltip_text(Some(tip));
        }
    }

    /// Grab focus.
    pub fn focus(self) -> bool {
        let Some(widget) = self.try_widget() else { return false; };
        widget.grab_focus()
    }

    /// Resign focus.
    pub fn blur(self) -> bool {
        let Some(widget) = self.try_widget() else { return false; };
        if let Some(root) = widget.root() {
            root.set_focus(None::<&gtk4::Widget>);
            true
        } else {
            false
        }
    }

    // ---------------------------------------------------------------------
    // text-label & placeholder constructors
    // ---------------------------------------------------------------------

    /// Build a text-label Node — a `gtk::Label` configured for
    /// left-aligned word-wrap.
    pub fn create_text(content: &str) -> Self {
        let label = gtk4::Label::new(Some(content));
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        let mut style = Style::default();
        style.flex_shrink = 0.0;
        GtkNode::new_from_widget(label, style).with_tag("#text")
    }

    /// Update the displayed string on a text-label Node.
    pub fn set_text(self, content: &str) {
        let Some(widget) = self.try_widget() else { return; };
        if let Some(label) = widget.downcast_ref::<gtk4::Label>() {
            if label.label().as_str() != content {
                label.set_label(content);
                crate::dom::layout::schedule_relayout(self);
            }
        }
    }

    /// Build a placeholder Node — a hidden, zero-sized `gtk::Label`.
    pub fn create_placeholder() -> Self {
        let widget = gtk4::Label::new(None::<&str>);
        widget.set_visible(false);

        let mut style = Style::default();
        style.position = crate::dom::layout::Position::Absolute;
        style.size.width = crate::dom::layout::Dimension::length(0.0);
        style.size.height = crate::dom::layout::Dimension::length(0.0);

        GtkNode::new_from_widget(widget, style).with_tag("placeholder")
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
fn attach_under(
    parent: &gtk4::Widget,
    child: &gtk4::Widget,
    marker: Option<&GtkNode>,
) {
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

/// Find the index of `target` in `parent`'s child chain.
fn child_index_in_parent(
    parent: &gtk4::Widget,
    target: &gtk4::Widget,
) -> Option<usize> {
    let mut cur = parent.first_child();
    let mut idx = 0usize;
    while let Some(w) = cur {
        if w.as_ptr() == target.as_ptr() {
            return Some(idx);
        }
        idx += 1;
        cur = w.next_sibling();
    }
    None
}

/// Install our [`TaffyLayout`] on `widget` so its layout is driven by
/// the ambient store. Idempotent.
pub fn install_taffy_layout_for_container(
    widget: &gtk4::Widget,
    node_id: NodeId,
    is_root: bool,
) {
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

