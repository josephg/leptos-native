//! Node, Element, Text, Placeholder — the DOM-shaped wrappers over
//! `gtk::Widget`.
//!
//! Each Node holds a SendWrapped `gtk::Widget`. Cloning is a cheap
//! gobject ref bump; the wrapped Widget stays alive as long as any
//! clone exists or it remains parented to another widget.
//!
//! Unlike `cocoa_dom`, we don't carry a layout slot per node — GTK
//! does its own layout (via each parent's `LayoutManager`), so there
//! is no Taffy tree to register against.
//!
//! # Threading
//!
//! `gtk::Widget` is `!Send` (GTK widgets are main-thread-only).
//! `SendWrapper` makes `Node` nominally `Send + 'static` so it can
//! flow through `tachys`/`reactive_graph`'s generic plumbing, with a
//! runtime panic if accessed off-main.

use gtk4::prelude::*;
#[allow(unused_imports)]
use gtk4::glib;
use send_wrapper::SendWrapper;
use std::fmt;

/// Distinguishes the three node varieties tachys cares about.
///
/// In the web DOM these correspond to Element / Text / Comment nodes.
/// We keep the distinction so `CastFrom<Node>` round-trips can validate
/// that a Node was originally created as an Element vs Text vs
/// Placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Element,
    Text,
    Placeholder,
}

/// The core node wrapper.
///
/// `Node` is `Clone` (cheap gobject retain) and `Send + 'static` (via
/// [`SendWrapper`]). It must only be touched on the main thread; off-
/// main access panics from the SendWrapper runtime check.
#[derive(Clone)]
pub struct Node {
    widget: SendWrapper<gtk4::Widget>,
    kind: NodeKind,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("kind", &self.kind)
            .field("type", &self.widget.type_().name())
            .finish()
    }
}

impl AsRef<Node> for Element {
    fn as_ref(&self) -> &Node {
        &self.node
    }
}

impl AsRef<Node> for Text {
    fn as_ref(&self) -> &Node {
        &self.node
    }
}

impl AsRef<Node> for Placeholder {
    fn as_ref(&self) -> &Node {
        &self.node
    }
}

impl Node {
    /// Build a Node from any concrete `gtk::Widget` subclass.
    pub fn from_widget<W>(widget: W, kind: NodeKind) -> Self
    where
        W: IsA<gtk4::Widget>,
    {
        Node {
            widget: SendWrapper::new(widget.upcast()),
            kind,
        }
    }

    /// Borrow the underlying `gtk::Widget`. Main-thread only.
    pub fn widget(&self) -> &gtk4::Widget {
        &self.widget
    }

    /// Take the wrapped `gtk::Widget`. Main-thread only.
    pub fn into_widget(self) -> gtk4::Widget {
        self.widget.take()
    }

    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    /// Pointer-equality check (same underlying gobject).
    pub fn ptr_eq(&self, other: &Node) -> bool {
        self.widget.as_ptr() == other.widget.as_ptr()
    }

    /// Drop the resources owned by this node — delegates to the
    /// parent's [`Element::remove_child`], which calls
    /// `gtk::Widget::unparent` to detach.
    ///
    /// In `cocoa_dom` this also unregisters the Taffy node and drops
    /// retained event-handler targets; on GTK both are unnecessary —
    /// GTK does its own layout, and signal-handler closures are owned
    /// by the widget itself, so they drop with it.
    ///
    /// Safe to call repeatedly; this no-ops if the widget has no parent.
    pub fn teardown(&self) {
        let Some(parent) = self.widget.parent() else {
            return;
        };

        let parent = Element::from_node_unchecked(Node::from_widget(
            parent,
            NodeKind::Element,
        ));
        let _ = parent.remove_child(self);
    }
}

// ---------------------------------------------------------------------
// Element
// ---------------------------------------------------------------------

/// An element node — anything created by [`Element::create`] for a
/// given tag. Wraps a `Node` whose kind is [`NodeKind::Element`].
#[derive(Clone, Debug)]
pub struct Element {
    node: Node,
}

impl Element {
    /// Wrap a `Node` whose kind has already been verified as
    /// `Element`. Panics in both debug and release if the kind is
    /// wrong — matches `cocoa_dom::Element::from_node_unchecked`.
    pub fn from_node_unchecked(node: Node) -> Self {
        assert_eq!(
            node.kind(),
            NodeKind::Element,
            "Element::from_node_unchecked called with a non-Element node"
        );
        Element { node }
    }

    /// Construct an element by tag name. Tag names map to GTK4 widget
    /// classes; see the impl for the supported set.
    ///
    /// Tags currently understood:
    ///
    ///   - `button`              → `gtk::Button`
    ///   - `checkbox`            → `gtk::CheckButton`
    ///   - `label`               → `gtk::Label`
    ///   - `text_field`          → `gtk::Entry`
    ///   - `secure_text_field`   → `gtk::PasswordEntry`
    ///   - `slider`              → `gtk::Scale` (horizontal)
    ///   - `pop_up_button`       → `gtk::DropDown`
    ///   - `vstack` / `stack_view` → `gtk::Box` (vertical)
    ///   - `hstack`              → `gtk::Box` (horizontal)
    ///   - `view` (and unknown)  → `gtk::Box` (vertical, treated as a
    ///     generic container — same default as cocoa_dom's flipped
    ///     view container)
    ///
    /// # Panics
    /// If called from a non-GTK-main thread (GTK enforces this
    /// internally via its own checks).
    pub fn create(tag: &str) -> Self {
        let widget: gtk4::Widget = match tag {
            "button" => gtk4::Button::new().upcast(),
            "checkbox" => gtk4::CheckButton::new().upcast(),
            "label" => gtk4::Label::new(None).upcast(),
            "text_field" => gtk4::Entry::new().upcast(),
            "secure_text_field" => gtk4::PasswordEntry::new().upcast(),
            "slider" => {
                let s = gtk4::Scale::new(
                    gtk4::Orientation::Horizontal,
                    None::<&gtk4::Adjustment>,
                );
                s.upcast()
            }
            "pop_up_button" => gtk4::DropDown::default().upcast(),
            "hstack" => {
                gtk4::Box::new(gtk4::Orientation::Horizontal, 0).upcast()
            }
            "vstack" | "stack_view" => {
                gtk4::Box::new(gtk4::Orientation::Vertical, 0).upcast()
            }
            // "view" or anything unknown → generic vertical box.
            // Mirrors cocoa_dom's choice of FlippedView with no
            // explicit flex direction (Taffy default is Row; for GTK
            // we default to Vertical to match the more common stack
            // expectation; users override with `<hstack>` when they
            // want horizontal).
            _ => gtk4::Box::new(gtk4::Orientation::Vertical, 0).upcast(),
        };

        Element {
            node: Node::from_widget(widget, NodeKind::Element),
        }
    }

    pub fn as_node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        self.node
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.node.widget()
    }

    fn prepare_child_for_parent(&self, child: &Node) -> bool {
        let parent = self.widget();
        let child_widget = child.widget();

        if child_widget.as_ptr() == parent.as_ptr() {
            return false;
        }

        match child_widget.parent() {
            Some(current_parent)
                if current_parent.as_ptr() == parent.as_ptr() =>
            {
                true
            }
            Some(_) => {
                child.teardown();
                child_widget.parent().is_none()
            }
            None => true,
        }
    }

    /// Insert `child` before `marker` in this element's child list.
    /// If `marker` is `None`, append.
    ///
    /// Mirrors `Node.insertBefore` from the web DOM.
    ///
    /// Routing per parent widget class:
    ///
    ///   - `gtk::Box`: append / insert via `insert_child_after` (the
    ///     sibling immediately *before* the marker).
    ///   - `gtk::Window` / `gtk::ApplicationWindow`: `set_child` when
    ///     appending the single root child.
    ///   - other: returns `false` — Stage 1 doesn't ship support for
    ///     other container kinds.
    pub fn insert_node(&self, child: &Node, marker: Option<&Node>) {
        let _ = self.try_insert_node(child, marker);
    }

    /// Try to insert `child` before `marker`. Returns `false` if the
    /// parent is not a supported GTK container, or if `marker` is not a
    /// child of this parent.
    pub fn try_insert_node(&self, child: &Node, marker: Option<&Node>) -> bool {
        let parent = self.widget();
        let child_widget = child.widget();

        if let Some(box_) = parent.downcast_ref::<gtk4::Box>() {
            match marker {
                None => {
                    if !self.prepare_child_for_parent(child) {
                        return false;
                    }

                    if child_widget.parent().as_ref().map(|p| p.as_ptr())
                        == Some(parent.as_ptr())
                    {
                        if box_.last_child().as_ref().map(|last| last.as_ptr())
                            != Some(child_widget.as_ptr())
                        {
                            box_.reorder_child_after(
                                child_widget,
                                box_.last_child().as_ref(),
                            );
                        }
                    } else {
                        box_.append(child_widget);
                    }
                }
                Some(marker) => {
                    let marker_widget = marker.widget();
                    if marker_widget.as_ptr() == child_widget.as_ptr() {
                        return true;
                    }
                    let Some(marker_parent) = marker_widget.parent() else {
                        return false;
                    };
                    if marker_parent.as_ptr() != parent.as_ptr() {
                        return false;
                    }

                    if !self.prepare_child_for_parent(child) {
                        return false;
                    }

                    let prev = marker.widget().prev_sibling();
                    if prev.as_ref().map(|prev| prev.as_ptr())
                        == Some(child_widget.as_ptr())
                    {
                        return true;
                    }

                    if child_widget.parent().as_ref().map(|p| p.as_ptr())
                        == Some(parent.as_ptr())
                    {
                        box_.reorder_child_after(child_widget, prev.as_ref());
                    } else {
                        box_.insert_child_after(child_widget, prev.as_ref());
                    }
                }
            }
            return true;
        }

        if let Some(window) = parent.downcast_ref::<gtk4::ApplicationWindow>() {
            if marker.is_some() || !self.prepare_child_for_parent(child) {
                return false;
            }
            window.set_child(Some(child_widget));
            return true;
        }
        if let Some(window) = parent.downcast_ref::<gtk4::Window>() {
            if marker.is_some() || !self.prepare_child_for_parent(child) {
                return false;
            }
            window.set_child(Some(child_widget));
            return true;
        }
        // Other parent classes: not supported in Stage 1.
        false
    }

    /// Remove `child` from this element's child list. Returns the
    /// node back if it was actually our child, otherwise `None`.
    ///
    /// Removal uses `gtk::Widget::unparent()` — the universal GTK4
    /// child-detachment call that works uniformly across every
    /// container class (Box, Frame, Grid, Window, Notebook, Stack,
    /// …). We don't dispatch per container class here the way
    /// [`insert_node`] does: insertion semantics are container-
    /// specific (ordered, positioned, single-child), but detachment
    /// is uniform across all GTK widgets.
    pub fn remove_child(&self, child: &Node) -> Option<Node> {
        let child_widget = child.widget();
        let child_parent = child_widget.parent()?;
        if child_parent.as_ptr() != self.widget().as_ptr() {
            return None;
        }
        child_widget.unparent();
        Some(child.clone())
    }

    /// Remove every child.
    ///
    /// Walks the widget's first-child chain, unparenting each as it
    /// goes. Like [`remove_child`], this works uniformly across all
    /// container classes.
    pub fn clear_children(&self) {
        while let Some(child) = self.widget().first_child() {
            child.unparent();
        }
    }

    /// Set a string-valued attribute. The supported set is small;
    /// extend here as needed.
    ///
    /// Currently understood:
    ///
    ///   - `title`       — `gtk::Button::set_label`
    ///   - `value`       — `gtk::Entry::set_text` /
    ///                      `gtk::Label::set_label` (depending on
    ///                      widget class)
    ///   - `placeholder` — `gtk::Entry::set_placeholder_text`
    ///
    /// Each setter diffs against the current value before mutating —
    /// avoids redundant signal fires that can cause `bind:` cycles
    /// and unwanted focus/cursor flicker.
    pub fn set_attribute(&self, name: &str, value: &str) {
        let widget = self.widget();
        match name {
            "title" => {
                if let Some(button) = widget.downcast_ref::<gtk4::Button>() {
                    let current = button.label();
                    if current.as_deref() != Some(value) {
                        button.set_label(value);
                    }
                } else if let Some(check) =
                    widget.downcast_ref::<gtk4::CheckButton>()
                {
                    let current = check.label();
                    if current.as_deref() != Some(value) {
                        check.set_label(Some(value));
                    }
                } else if let Some(label) = widget.downcast_ref::<gtk4::Label>()
                {
                    if label.label().as_str() != value {
                        label.set_label(value);
                    }
                }
            }
            "value" => {
                if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
                    if entry.text().as_str() != value {
                        entry.set_text(value);
                    }
                } else if let Some(label) = widget.downcast_ref::<gtk4::Label>()
                {
                    if label.label().as_str() != value {
                        label.set_label(value);
                    }
                }
            }
            "placeholder" => {
                if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
                    if entry.placeholder_text().as_deref() != Some(value) {
                        entry.set_placeholder_text(Some(value));
                    }
                } else if let Some(entry) =
                    widget.downcast_ref::<gtk4::PasswordEntry>()
                {
                    if entry.placeholder_text().as_deref() != Some(value) {
                        entry.set_placeholder_text(Some(value));
                    }
                }
            }
            _ => { /* silently ignored */ }
        }
    }

    /// Set a typed boolean attribute. Routing per name:
    ///
    ///   - `enabled` → `gtk::Widget::set_sensitive`
    ///   - `hidden`  → `gtk::Widget::set_visible` (inverted)
    ///   - `checked` → `gtk::CheckButton::set_active`
    ///
    /// Each setter diffs first to avoid the redundant-write cycle
    /// that `bind:` would otherwise flash.
    pub fn set_bool_attribute(&self, name: &str, value: bool) {
        let widget = self.widget();
        match name {
            "hidden" => {
                if widget.is_visible() == value {
                    widget.set_visible(!value);
                }
            }
            "enabled" => {
                if widget.is_sensitive() != value {
                    widget.set_sensitive(value);
                }
            }
            "checked" => {
                if let Some(check) = widget.downcast_ref::<gtk4::CheckButton>()
                {
                    if check.is_active() != value {
                        check.set_active(value);
                    }
                }
            }
            _ => { /* silently ignored */ }
        }
    }

    /// Wire a click handler to this element. No-op if the underlying
    /// widget isn't a `gtk::Button`.
    ///
    /// Multiple `on_click` calls stack — each registers an additional
    /// `clicked` signal connection. This differs from the AppKit
    /// target/action model (which has a single target slot) but
    /// matches the more web-like `addEventListener` shape; nothing
    /// in the rest of the port relies on the single-handler
    /// limitation.
    pub fn on_click(&self, cb: impl FnMut() + 'static) {
        if let Some(button) = self.widget().downcast_ref::<gtk4::Button>() {
            let cb = std::cell::RefCell::new(cb);
            button.connect_clicked(move |_| {
                if let Ok(mut cb) = cb.try_borrow_mut() {
                    cb();
                } else {
                    eprintln!("[gtk_dom] reentrant click handler skipped");
                }
            });
        }
    }

    /// Wire a callback that fires whenever the text content of an
    /// entry changes (every keystroke / paste / etc.). No-op if this
    /// element isn't a `gtk::Entry`. Multiple handlers stack —
    /// each call appends an additional `connect_changed` signal
    /// connection.
    pub fn on_text_change(&self, cb: impl FnMut(String) + 'static) {
        if let Some(entry) = self.widget().downcast_ref::<gtk4::Entry>() {
            let cb = std::cell::RefCell::new(cb);
            entry.connect_changed(move |e| {
                if let Ok(mut cb) = cb.try_borrow_mut() {
                    cb(e.text().to_string());
                } else {
                    eprintln!(
                        "[gtk_dom] reentrant text-change handler skipped"
                    );
                }
            });
        }
    }

    /// Wire a callback that fires when the user commits an edit in a
    /// text field (return key, Enter). No-op if this element isn't a
    /// `gtk::Entry`. Coexists with `on_text_change`.
    pub fn on_text_activate(&self, cb: impl FnMut(String) + 'static) {
        if let Some(entry) = self.widget().downcast_ref::<gtk4::Entry>() {
            let cb = std::cell::RefCell::new(cb);
            entry.connect_activate(move |e| {
                if let Ok(mut cb) = cb.try_borrow_mut() {
                    cb(e.text().to_string());
                } else {
                    eprintln!(
                        "[gtk_dom] reentrant text-activate handler skipped"
                    );
                }
            });
        }
    }

    /// Wire a callback that fires when an actionable control changes
    /// value (slider drag, dropdown selection, checkbox toggle). No-op
    /// if the element doesn't have a matching signal.
    ///
    /// This is the generic "value changed" companion to [`on_click`];
    /// use it for slider/checkbox-type controls where "click" is
    /// misleading.
    pub fn on_action(&self, cb: impl FnMut() + 'static) {
        let widget = self.widget();
        let cb = std::sync::Arc::new(std::sync::Mutex::new(cb));

        // Slider: connect_value_changed fires on drag + click-to-move.
        if let Some(scale) = widget.downcast_ref::<gtk4::Scale>() {
            let cb = cb.clone();
            scale.connect_value_changed(move |_| {
                if let Ok(mut cb) = cb.try_lock() {
                    cb();
                }
            });
            return;
        }
        // Checkbox toggle.
        if let Some(check) = widget.downcast_ref::<gtk4::CheckButton>() {
            let cb = cb.clone();
            check.connect_toggled(move |_| {
                if let Ok(mut cb) = cb.try_lock() {
                    cb();
                }
            });
            return;
        }
        // DropDown selection change.
        if let Some(dd) = widget.downcast_ref::<gtk4::DropDown>() {
            dd.connect_selected_notify(move |_| {
                if let Ok(mut cb) = cb.try_lock() {
                    cb();
                }
            });
        }
    }

    /// Read the on/off state of a `gtk::CheckButton` (checkbox /
    /// switch / other toggle types). Returns `false` for non-checkbutton
    /// widgets.
    pub fn checked(&self) -> bool {
        self.widget()
            .downcast_ref::<gtk4::CheckButton>()
            .map(|c| c.is_active())
            .unwrap_or(false)
    }

    /// Read the current value of a `gtk::Scale` (slider). Returns 0.0
    /// for non-scale widgets.
    pub fn double_value(&self) -> f64 {
        self.widget()
            .downcast_ref::<gtk4::Scale>()
            .map(|s| s.value())
            .unwrap_or(0.0)
    }

    /// Set the value on a `gtk::Scale`. Diffs to avoid redundant
    /// redraws and signal cycles. No-ops on non-scale widgets.
    pub fn set_double_value(&self, v: f64) {
        if let Some(s) = self.widget().downcast_ref::<gtk4::Scale>() {
            if (s.value() - v).abs() > f64::EPSILON {
                s.set_value(v);
            }
        }
    }

    /// Set the slider's minimum value. No-op on non-scale widgets.
    pub fn set_slider_min(&self, v: f64) {
        if let Some(s) = self.widget().downcast_ref::<gtk4::Scale>() {
            s.adjustment().set_lower(v);
        }
    }

    /// Set the slider's maximum value. No-op on non-scale widgets.
    pub fn set_slider_max(&self, v: f64) {
        if let Some(s) = self.widget().downcast_ref::<gtk4::Scale>() {
            s.adjustment().set_upper(v);
        }
    }

    /// Replace the items list on a `gtk::DropDown`. No-op on non-
    /// dropdown widgets.
    pub fn set_popup_items(&self, items: &[String]) {
        if let Some(dd) = self.widget().downcast_ref::<gtk4::DropDown>() {
            let model = gtk4::StringList::new(&[]);
            for it in items {
                model.append(it);
            }
            dd.set_model(Some(&model));
        }
    }

    /// Currently-selected index on a `gtk::DropDown` (0-based).
    /// Returns 0 for non-dropdown widgets.
    pub fn popup_selection(&self) -> u32 {
        self.widget()
            .downcast_ref::<gtk4::DropDown>()
            .map(|dd| dd.selected())
            .unwrap_or(0)
    }

    /// Programmatically pick an item by index. Diffs first to avoid
    /// redundant signal fires. No-op on non-dropdown widgets.
    pub fn set_popup_selection(&self, idx: u32) {
        if let Some(dd) = self.widget().downcast_ref::<gtk4::DropDown>() {
            if dd.selected() != idx {
                dd.set_selected(idx);
            }
        }
    }

    pub fn remove_attribute(&self, name: &str) {
        let widget = self.widget();
        match name {
            "title" => {
                if let Some(button) = widget.downcast_ref::<gtk4::Button>() {
                    button.set_label("");
                }
            }
            "value" => {
                if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
                    entry.set_text("");
                } else if let Some(label) = widget.downcast_ref::<gtk4::Label>()
                {
                    label.set_label("");
                }
            }
            "placeholder" => {
                if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
                    entry.set_placeholder_text(None);
                }
            }
            "hidden" => self.set_bool_attribute("hidden", false),
            "enabled" => self.set_bool_attribute("enabled", true),
            "checked" => self.set_bool_attribute("checked", false),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------

/// A text node. Backed by a `gtk::Label`.
#[derive(Clone, Debug)]
pub struct Text {
    node: Node,
}

impl Text {
    /// Wrap a `Node` whose kind has already been verified as `Text`.
    /// Panics in both debug and release if the kind is wrong.
    pub fn from_node_unchecked(node: Node) -> Self {
        assert_eq!(
            node.kind(),
            NodeKind::Text,
            "Text::from_node_unchecked called with a non-Text node"
        );
        Text { node }
    }

    pub fn create(content: &str) -> Self {
        let label = gtk4::Label::new(Some(content));
        Text {
            node: Node::from_widget(label, NodeKind::Text),
        }
    }

    pub fn as_node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        self.node
    }

    /// Update the displayed string. No-ops if the value hasn't
    /// changed.
    pub fn set_text(&self, content: &str) {
        if let Some(label) = self.node.widget().downcast_ref::<gtk4::Label>() {
            if label.label().as_str() != content {
                label.set_label(content);
            }
        }
    }
}

// ---------------------------------------------------------------------
// Placeholder
// ---------------------------------------------------------------------

/// A placeholder node — has a position in the tree but no visible
/// representation. Used by tachys to anchor dynamic content (the
/// moral equivalent of an HTML comment node used as a marker).
///
/// Backed by a hidden `gtk::Label`. A hidden widget is removed from
/// layout entirely on GTK, so there's no need for the `position:
/// Absolute` Taffy hack used in `cocoa_dom`. We use a label rather
/// than a Box because a label cannot accept children — if tachys
/// incorrectly tries to mount content under a placeholder, GTK will
/// error at the widget level rather than silently rendering visible
/// children under an invisible container.
#[derive(Clone, Debug)]
pub struct Placeholder {
    node: Node,
}

impl Placeholder {
    /// Wrap a `Node` whose kind has already been verified as
    /// `Placeholder`. Panics in both debug and release if the kind is
    /// wrong.
    pub fn from_node_unchecked(node: Node) -> Self {
        assert_eq!(
            node.kind(),
            NodeKind::Placeholder,
            "Placeholder::from_node_unchecked called with a \
             non-Placeholder node"
        );
        Placeholder { node }
    }

    pub fn create() -> Self {
        // Use a label rather than a box for the placeholder: a label
        // cannot accept children, so if tachys ever incorrectly mounts
        // something under a placeholder, GTK will error rather than
        // silently creating a visible child of an invisible widget.
        let widget = gtk4::Label::new(None::<&str>);
        widget.set_visible(false);
        Placeholder {
            node: Node::from_widget(widget, NodeKind::Placeholder),
        }
    }

    pub fn as_node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        self.node
    }
}