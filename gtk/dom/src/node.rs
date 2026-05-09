//! Node, Element, Text, Placeholder — the DOM-shaped wrappers over
//! `gtk::Widget`.
//!
//! Each Node holds a SendWrapped `gtk::Widget` plus a shared layout
//! slot ([`NodeLayout`], stored in `Rc<RefCell<...>>` shared across
//! Node clones). The slot has two pieces:
//!
//!  - the node's *current* style (Taffy [`Style`]), mutated by setters
//!    and used as the seed when the node is registered in a tree;
//!  - an `Option<LayoutHandle>` — `Some` once the node has been
//!    registered into a [`TaffyTree`] (i.e. mounted somewhere under a
//!    [`Window`](crate::window)). While `None`, style mutations stay
//!    local; once `Some`, they're also pushed into the tree.
//!
//! Trees themselves are owned by their [`Window`]; each LayoutHandle
//! keeps an Rc to its tree.
//!
//! # Threading
//!
//! `gtk::Widget` is `!Send` (GTK widgets are main-thread-only).
//! `SendWrapper` makes `Node` nominally `Send + 'static` so it can
//! flow through tachys/reactive_graph's generic plumbing, with a
//! runtime panic if accessed off-main.

use crate::layout::{LayoutHandle, NodeLayout, Style};
use crate::taffy_layout::TaffyLayout;
use gtk4::glib;
use gtk4::prelude::*;
use send_wrapper::SendWrapper;
use std::{cell::RefCell, fmt, rc::Rc};

/// Compile-time-checked attribute identifiers, split by value type.
/// Mirrors `cocoa_dom::node::StringAttr`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum StringAttr {
    Title,
    Value,
    Placeholder,
}

impl StringAttr {
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "title" => Self::Title,
            "value" => Self::Value,
            "placeholder" => Self::Placeholder,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Value => "value",
            Self::Placeholder => "placeholder",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BoolAttr {
    Enabled,
    Hidden,
    Checked,
}

impl BoolAttr {
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "enabled" => Self::Enabled,
            "hidden" => Self::Hidden,
            "checked" => Self::Checked,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Hidden => "hidden",
            Self::Checked => "checked",
        }
    }
}

/// Distinguishes the three node varieties tachys cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Element,
    Text,
    Placeholder,
}

/// The core node wrapper.
#[derive(Clone)]
pub struct Node {
    widget: SendWrapper<gtk4::Widget>,
    layout: SendWrapper<Rc<RefCell<NodeLayout>>>,
    kind: NodeKind,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("kind", &self.kind)
            .field("type", &self.widget.type_().name())
            .field("registered", &self.layout.borrow().handle.is_some())
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
    /// Build a Node from any concrete `gtk::Widget` subclass with the
    /// given default Taffy style.
    pub fn from_widget<W>(
        widget: W,
        kind: NodeKind,
        default_style: Style,
    ) -> Self
    where
        W: IsA<gtk4::Widget>,
    {
        Node {
            widget: SendWrapper::new(widget.upcast()),
            layout: SendWrapper::new(Rc::new(RefCell::new(
                NodeLayout::new(default_style),
            ))),
            kind,
        }
    }

    /// Build a Node wrapping `widget` with a pre-existing
    /// [`LayoutHandle`] — used by `mount_before` in
    /// `leptos_gtk::Dom` to synthesise a parent Element wrapper for
    /// a widget whose Node we don't have, by borrowing the parent's
    /// LayoutHandle from a sibling node we do have.
    pub fn from_widget_with_handle<W>(
        widget: W,
        kind: NodeKind,
        handle: LayoutHandle,
    ) -> Self
    where
        W: IsA<gtk4::Widget>,
    {
        let layout = NodeLayout {
            style: Style::default(),
            handle: Some(handle),
        };
        Node {
            widget: SendWrapper::new(widget.upcast()),
            layout: SendWrapper::new(Rc::new(RefCell::new(layout))),
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

    /// Borrow the (interior-mutable) layout slot.
    pub fn layout_slot(&self) -> &RefCell<NodeLayout> {
        &**self.layout
    }

    /// Pointer-equality check (same underlying gobject).
    pub fn ptr_eq(&self, other: &Node) -> bool {
        self.widget.as_ptr() == other.widget.as_ptr()
    }

    /// Drop the resources owned by this node. Detaches Taffy entry
    /// and unparents the widget. Safe to call repeatedly.
    pub fn teardown(&self) {
        crate::layout::drop_node(self);
        if self.widget.parent().is_some() {
            self.widget.unparent();
        }
    }
}

// ---------------------------------------------------------------------
// Element
// ---------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Element {
    node: Node,
}

impl Element {
    /// Wrap a `Node` whose kind has already been verified as
    /// `Element`. Panics if the kind is wrong.
    pub fn from_node_unchecked(node: Node) -> Self {
        assert_eq!(
            node.kind(),
            NodeKind::Element,
            "Element::from_node_unchecked called with a non-Element node"
        );
        Element { node }
    }

    /// Construct an element by tag name. Tag names map to GTK4 widget
    /// classes.
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
    ///   - `vstack` / `stack_view` → bare `gtk::Widget` w/ Taffy
    ///     (`flex_direction: Column`)
    ///   - `hstack`              → bare `gtk::Widget` w/ Taffy
    ///     (`flex_direction: Row`)
    ///   - `stack`               → bare `gtk::Widget` w/ Taffy (no
    ///     direction preset; default Row)
    ///   - `view` (and unknown)  → bare `gtk::Widget` w/ Taffy
    ///     (default style — same as cocoa's flipped view container)
    pub fn create(tag: &str) -> Self {
        use crate::layout::{FlexDirection, Style};

        let (widget, default_style): (gtk4::Widget, Style) = match tag {
            "button" => {
                let b = gtk4::Button::new();
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (b.upcast(), s)
            }
            "checkbox" => {
                let c = gtk4::CheckButton::new();
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (c.upcast(), s)
            }
            "label" => {
                let l = gtk4::Label::new(None);
                // Default to left-aligned text — matches AppKit's
                // wrappingLabel default. Wraps on overflow.
                l.set_xalign(0.0);
                l.set_wrap(true);
                l.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (l.upcast(), s)
            }
            "text_field" => {
                let e = gtk4::Entry::new();
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (e.upcast(), s)
            }
            "secure_text_field" => {
                let e = gtk4::PasswordEntry::new();
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (e.upcast(), s)
            }
            "slider" => {
                let s_ = gtk4::Scale::new(
                    gtk4::Orientation::Horizontal,
                    None::<&gtk4::Adjustment>,
                );
                s_.set_draw_value(false);
                let mut st = Style::default();
                st.flex_shrink = 0.0;
                (s_.upcast(), st)
            }
            "pop_up_button" => {
                let dd = gtk4::DropDown::default();
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (dd.upcast(), s)
            }
            "hstack" => {
                let w = container_widget();
                let mut s = Style::default();
                s.flex_direction = FlexDirection::Row;
                (w, s)
            }
            "vstack" | "stack_view" => {
                let w = container_widget();
                let mut s = Style::default();
                s.flex_direction = FlexDirection::Column;
                (w, s)
            }
            "stack" => {
                // Bare flexbox container; direction defaults to Row
                // unless the builder sets it.
                let w = container_widget();
                (w, Style::default())
            }
            // `view` or anything unknown → generic container.
            _ => {
                let w = container_widget();
                (w, Style::default())
            }
        };

        Element {
            node: Node::from_widget(widget, NodeKind::Element, default_style),
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

    /// Insert `child` before `marker` in this element's child list.
    /// If `marker` is `None`, append.
    ///
    /// The widget tree is updated via `Widget::insert_before`/
    /// `Widget::set_parent`; the Taffy tree is updated to mirror.
    pub fn insert_node(&self, child: &crate::Node, marker: Option<&crate::Node>) {
        let _ = self.try_insert_node(child, marker);
    }

    /// Try to insert `child` before `marker`. Returns `false` if the
    /// parent isn't a supported container, or if `marker` is not a
    /// child of this parent.
    pub fn try_insert_node(
        &self,
        child: &crate::Node,
        marker: Option<&crate::Node>,
    ) -> bool {
        let parent = self.widget();
        let child_widget = child.widget();

        // Self-parent? Reject.
        if child_widget.as_ptr() == parent.as_ptr() {
            return false;
        }

        // Window parents use `set_child` (single child). We hit this
        // path during `mount_to_window` setup, before we install our
        // content_root.
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

        // Generic container path: detach if it's already parented
        // somewhere else, then re-parent under self. Use
        // `insert_before` to position relative to a sibling marker.
        match child_widget.parent() {
            Some(p) if p.as_ptr() == parent.as_ptr() => {
                // Already our child — reposition only.
                match marker {
                    None => {
                        // Move to end: insert_before(None, None) is a
                        // way to detach + reparent at end.
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
                        child_widget.insert_before(parent, Some(m_widget));
                    }
                }
            }
            Some(_) => {
                // Different parent — unparent first.
                child_widget.unparent();
                attach_under(parent, child_widget, marker);
            }
            None => {
                attach_under(parent, child_widget, marker);
            }
        }

        // Mirror into Taffy. Find where `child_widget` ended up in the
        // widget's child chain so the Taffy index matches.
        let idx = child_index_in_parent(parent, child_widget);
        if let Some(idx) = idx {
            crate::layout::insert_child_at(self.as_node(), child, idx);
        } else {
            crate::layout::attach_child(self.as_node(), child);
        }
        true
    }

    /// Remove `child` from this element's child list. Returns the
    /// node back if it was actually our child, otherwise `None`.
    pub fn remove_child(&self, child: &crate::Node) -> Option<crate::Node> {
        let parent = self.widget();
        let child_widget = child.widget();
        let child_parent = child_widget.parent()?;
        if child_parent.as_ptr() != parent.as_ptr() {
            return None;
        }
        // Window's set_child(None) unparents; otherwise just unparent.
        if let Some(window) = parent.downcast_ref::<gtk4::ApplicationWindow>() {
            window.set_child(None::<&gtk4::Widget>);
        } else if let Some(window) = parent.downcast_ref::<gtk4::Window>() {
            window.set_child(None::<&gtk4::Widget>);
        } else {
            child_widget.unparent();
        }
        crate::layout::detach_child(self.as_node(), child);
        Some(child.clone())
    }

    /// Remove every child.
    pub fn clear_children(&self) {
        let parent = self.widget();
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

    /// `&str`-keyed entry point matching the renderer trait. Routes
    /// through the typed enums; silently no-ops on unknown names.
    pub fn set_attribute(&self, name: &str, value: &str) {
        if let Some(attr) = StringAttr::from_name(name) {
            self.set_string_attribute(attr, value);
        }
    }

    /// Typed string-valued attribute setter.
    pub fn set_string_attribute(&self, attr: StringAttr, value: &str) {
        let widget = self.widget();
        let mut content_changed = false;
        match attr {
            StringAttr::Title => {
                if let Some(button) = widget.downcast_ref::<gtk4::Button>() {
                    let current = button.label();
                    if current.as_deref() != Some(value) {
                        button.set_label(value);
                        content_changed = true;
                    }
                } else if let Some(check) =
                    widget.downcast_ref::<gtk4::CheckButton>()
                {
                    let current = check.label();
                    if current.as_deref() != Some(value) {
                        check.set_label(Some(value));
                        content_changed = true;
                    }
                } else if let Some(label) =
                    widget.downcast_ref::<gtk4::Label>()
                {
                    if label.label().as_str() != value {
                        label.set_label(value);
                        content_changed = true;
                    }
                }
            }
            StringAttr::Value => {
                if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
                    if entry.text().as_str() != value {
                        entry.set_text(value);
                        content_changed = true;
                    }
                } else if let Some(entry) =
                    widget.downcast_ref::<gtk4::PasswordEntry>()
                {
                    if entry.text().as_str() != value {
                        entry.set_text(value);
                        content_changed = true;
                    }
                } else if let Some(label) =
                    widget.downcast_ref::<gtk4::Label>()
                {
                    if label.label().as_str() != value {
                        label.set_label(value);
                        content_changed = true;
                    }
                }
            }
            StringAttr::Placeholder => {
                if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
                    if entry.placeholder_text().as_deref() != Some(value) {
                        entry.set_placeholder_text(Some(value));
                        content_changed = true;
                    }
                } else if let Some(entry) =
                    widget.downcast_ref::<gtk4::PasswordEntry>()
                {
                    if entry.placeholder_text().as_deref() != Some(value) {
                        entry.set_placeholder_text(Some(value));
                        content_changed = true;
                    }
                }
            }
        }
        if content_changed {
            crate::layout::schedule_relayout(&self.node);
        }
    }

    /// Typed boolean-valued attribute setter.
    pub fn set_bool_attribute(&self, attr: BoolAttr, value: bool) {
        let widget = self.widget();
        match attr {
            BoolAttr::Hidden => {
                if widget.is_visible() == value {
                    widget.set_visible(!value);
                }
            }
            BoolAttr::Enabled => {
                if widget.is_sensitive() != value {
                    widget.set_sensitive(value);
                }
            }
            BoolAttr::Checked => {
                if let Some(check) =
                    widget.downcast_ref::<gtk4::CheckButton>()
                {
                    if check.is_active() != value {
                        check.set_active(value);
                    }
                }
            }
        }
    }

    pub fn remove_attribute(&self, name: &str) {
        if let Some(attr) = StringAttr::from_name(name) {
            self.remove_string_attribute(attr);
            return;
        }
        if let Some(attr) = BoolAttr::from_name(name) {
            self.remove_bool_attribute(attr);
        }
    }

    pub fn remove_string_attribute(&self, attr: StringAttr) {
        match attr {
            StringAttr::Title => self.set_string_attribute(StringAttr::Title, ""),
            StringAttr::Value => self.set_string_attribute(StringAttr::Value, ""),
            StringAttr::Placeholder => {
                let widget = self.widget();
                if let Some(entry) = widget.downcast_ref::<gtk4::Entry>() {
                    entry.set_placeholder_text(None);
                } else if let Some(entry) =
                    widget.downcast_ref::<gtk4::PasswordEntry>()
                {
                    entry.set_placeholder_text(None);
                }
            }
        }
    }

    pub fn remove_bool_attribute(&self, attr: BoolAttr) {
        match attr {
            BoolAttr::Hidden => self.set_bool_attribute(BoolAttr::Hidden, false),
            BoolAttr::Enabled => self.set_bool_attribute(BoolAttr::Enabled, true),
            BoolAttr::Checked => self.set_bool_attribute(BoolAttr::Checked, false),
        }
    }

    // ---- event hooks (delegate to crate::event) ----

    pub fn on_click(&self, cb: impl FnMut() + 'static) {
        crate::event::on_click(self.widget(), cb);
    }

    pub fn on_action(&self, cb: impl FnMut() + 'static) {
        crate::event::on_action(self.widget(), cb);
    }

    pub fn on_text_change(&self, cb: impl FnMut(String) + 'static) {
        crate::event::on_text_change(self.widget(), cb);
    }

    pub fn on_text_end_editing(&self, cb: impl FnMut(String) + 'static) {
        crate::event::on_text_end_editing(self.widget(), cb);
    }

    pub fn on_text_focus(&self, cb: impl FnMut() + 'static) {
        crate::event::on_text_focus(self.widget(), cb);
    }

    pub fn on_text_blur(&self, cb: impl FnMut() + 'static) {
        crate::event::on_text_blur(self.widget(), cb);
    }

    // ---- value accessors ----

    pub fn checked(&self) -> bool {
        self.widget()
            .downcast_ref::<gtk4::CheckButton>()
            .map(|c| c.is_active())
            .unwrap_or(false)
    }

    pub fn double_value(&self) -> f64 {
        self.widget()
            .downcast_ref::<gtk4::Scale>()
            .map(|s| s.value())
            .unwrap_or(0.0)
    }

    pub fn set_double_value(&self, v: f64) {
        if let Some(s) = self.widget().downcast_ref::<gtk4::Scale>() {
            if (s.value() - v).abs() > f64::EPSILON {
                s.set_value(v);
            }
        }
    }

    pub fn set_slider_min(&self, v: f64) {
        if let Some(s) = self.widget().downcast_ref::<gtk4::Scale>() {
            s.adjustment().set_lower(v);
        }
    }

    pub fn set_slider_max(&self, v: f64) {
        if let Some(s) = self.widget().downcast_ref::<gtk4::Scale>() {
            s.adjustment().set_upper(v);
        }
    }

    pub fn set_popup_items(&self, items: &[String]) {
        if let Some(dd) = self.widget().downcast_ref::<gtk4::DropDown>() {
            let model = gtk4::StringList::new(&[]);
            for it in items {
                model.append(it);
            }
            dd.set_model(Some(&model));
        }
    }

    /// Currently-selected index on a `gtk::DropDown`. Returns `0` for
    /// non-dropdown widgets, matching AppKit's "no selection" being
    /// represented as 0 here for portability with cocoa.
    pub fn popup_selection(&self) -> u32 {
        self.widget()
            .downcast_ref::<gtk4::DropDown>()
            .map(|dd| dd.selected())
            .unwrap_or(0)
    }

    pub fn set_popup_selection(&self, idx: u32) {
        if let Some(dd) = self.widget().downcast_ref::<gtk4::DropDown>() {
            if dd.selected() != idx {
                dd.set_selected(idx);
            }
        }
    }

    /// Set this view's opacity (0.0..=1.0). Maps to
    /// `gtk::Widget::set_opacity`. Diff-guarded.
    pub fn set_alpha(&self, alpha: f64) {
        let w = self.widget();
        let clamped = alpha.clamp(0.0, 1.0);
        if (w.opacity() - clamped).abs() > f64::EPSILON {
            w.set_opacity(clamped);
        }
    }

    /// Set this view's tooltip text. Empty string clears it.
    pub fn set_tool_tip(&self, tip: &str) {
        let w = self.widget();
        if tip.is_empty() {
            w.set_tooltip_text(None);
        } else {
            w.set_tooltip_text(Some(tip));
        }
    }

    /// Set the focused state — equivalent to AppKit's
    /// `makeFirstResponder`. Returns `true` always (GTK's
    /// `grab_focus` returns a bool indicating success).
    pub fn focus(&self) -> bool {
        self.widget().grab_focus()
    }

    /// Resign focus. GTK doesn't have a direct "blur" — we route to
    /// the parent window's "give up focus" by grabbing focus on the
    /// root.
    pub fn blur(&self) -> bool {
        if let Some(root) = self.widget().root() {
            root.set_focus(None::<&gtk4::Widget>);
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Text {
    node: Node,
}

impl Text {
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
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        let mut style = Style::default();
        style.flex_shrink = 0.0;
        Text {
            node: Node::from_widget(label, NodeKind::Text, style),
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
        if let Some(label) =
            self.node.widget().downcast_ref::<gtk4::Label>()
        {
            if label.label().as_str() != content {
                label.set_label(content);
                crate::layout::schedule_relayout(&self.node);
            }
        }
    }
}

// ---------------------------------------------------------------------
// Placeholder
// ---------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Placeholder {
    node: Node,
}

impl Placeholder {
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
        // Use a hidden Label (not a Box) so attempts to mount under a
        // placeholder error at the GTK layer rather than silently
        // succeeding.
        let widget = gtk4::Label::new(None::<&str>);
        widget.set_visible(false);

        let mut style = Style::default();
        // Keep the placeholder out of flex layout — same trick
        // cocoa_dom uses.
        style.position = crate::layout::Position::Absolute;
        style.size.width = crate::layout::Dimension::length(0.0);
        style.size.height = crate::layout::Dimension::length(0.0);

        Placeholder {
            node: Node::from_widget(widget, NodeKind::Placeholder, style),
        }
    }

    pub fn as_node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        self.node
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// Build a bare GtkWidget that hosts arbitrary children laid out by
/// our [`TaffyLayout`]. The widget itself is a `gtk::Box` (the
/// simplest concrete `gtk::Widget` that supports children), but its
/// layout manager is replaced — `BoxLayout`'s spacing/orientation
/// are not used.
fn container_widget() -> gtk4::Widget {
    // GtkBox with no internal spacing/orientation is the easiest way
    // to get a real GTK container that supports add/remove children
    // (via append/prepend) without needing to subclass GtkWidget. We
    // attach our TaffyLayout in `taffy_layout::install_taffy_layout`
    // once the node has registered in a tree (and we know the tree +
    // node id), so the GtkBox's default BoxLayout is replaced.
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    b.upcast()
}

/// Append or insert `child` under `parent`, choosing the right GTK
/// API based on whether `parent` is a `gtk::Box` (`append` /
/// `insert_child_after`) or a generic widget (use `set_parent` +
/// `insert_before`).
fn attach_under(
    parent: &gtk4::Widget,
    child: &gtk4::Widget,
    marker: Option<&crate::Node>,
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
        // Generic path. set_parent + insert_before.
        child.set_parent(parent);
        if let Some(m) = marker {
            child.insert_before(parent, Some(m.widget()));
        }
    }
}

/// Find the index of `target` in `parent`'s child chain, by walking
/// `first_child` → `next_sibling`. Returns `None` if not found.
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
/// `tree`. `is_root` is true only for the window's content_root.
///
/// Called from [`crate::window::open_window`] (for the root) and from
/// [`Element::create`]'s caller when a registered element is a
/// container. Idempotent: if the widget already has a TaffyLayout,
/// the existing one is left alone.
pub fn install_taffy_layout_for_container(
    widget: &gtk4::Widget,
    tree: &crate::layout::TreeRef,
    node_id: crate::layout::NodeId,
    is_root: bool,
) {
    // Already installed? Skip.
    if widget
        .layout_manager()
        .map(|lm| lm.is::<TaffyLayout>())
        .unwrap_or(false)
    {
        return;
    }
    let lm = TaffyLayout::new(tree.clone(), node_id, is_root);
    widget.set_layout_manager(Some(lm));
}

/// Returns whether the given widget is a container (one we should
/// install a TaffyLayout on). Containers are GtkBox-derived (we
/// build them as GtkBox in [`container_widget`] above).
pub fn is_container_widget(widget: &gtk4::Widget) -> bool {
    widget.is::<gtk4::Box>()
}

// Suppress an unused-import warning when only some functions need it.
#[allow(dead_code)]
fn _unused() {
    let _ = glib::value::Value::from(0i32);
}
