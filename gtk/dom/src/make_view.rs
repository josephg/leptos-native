//! Typed per-control [`Node`] constructors.
//!
//! Each function here allocates a concrete gtk4 widget class
//! (`gtk::Button`, `gtk::Label`, ...), builds its default Taffy
//! [`Style`], and registers it in `tree` via [`Node::from_view`].
//! Every typed builder in `leptos_gtk` calls exactly one of these
//! from its `Render::build`.
//!
//! Replaces the old tag-string dispatch
//! (`Element::create(tree, "button")` → big `match tag` in
//! `node.rs`). Same shape as cocoa's `make_view.rs`.

use crate::{
    layout::{FlexDirection, Style},
    node::Node,
};
use gtk4::prelude::*;

impl Node {
    pub fn create_button() -> (Node, gtk4::Button) {
        let b = gtk4::Button::new();
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = Node::from_view(b.clone(), s).with_tag("button");
        (n, b)
    }

    pub fn create_checkbox() -> (Node, gtk4::CheckButton) {
        let c = gtk4::CheckButton::new();
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = Node::from_view(c.clone(), s).with_tag("checkbox");
        (n, c)
    }

    /// Wrapping multi-line label. Default left-aligned text — matches
    /// AppKit's `wrappingLabel` default. Wraps on overflow.
    pub fn create_label() -> (Node, gtk4::Label) {
        let l = gtk4::Label::new(None);
        l.set_xalign(0.0);
        l.set_wrap(true);
        l.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = Node::from_view(l.clone(), s).with_tag("label");
        (n, l)
    }

    pub fn create_text_field() -> (Node, gtk4::Entry) {
        let e = gtk4::Entry::new();
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = Node::from_view(e.clone(), s).with_tag("text_field");
        (n, e)
    }

    pub fn create_secure_text_field() -> (Node, gtk4::PasswordEntry) {
        let e = gtk4::PasswordEntry::new();
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = Node::from_view(e.clone(), s).with_tag("secure_text_field");
        (n, e)
    }

    /// Horizontal `gtk::Scale` with the numeric value display off
    /// (the slider is just the thumb + track).
    pub fn create_slider() -> (Node, gtk4::Scale) {
        let scale = gtk4::Scale::new(
            gtk4::Orientation::Horizontal,
            None::<&gtk4::Adjustment>,
        );
        scale.set_draw_value(false);
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = Node::from_view(scale.clone(), s).with_tag("slider");
        (n, scale)
    }

    pub fn create_pop_up_button() -> (Node, gtk4::DropDown) {
        let dd = gtk4::DropDown::default();
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = Node::from_view(dd.clone(), s).with_tag("pop_up_button");
        (n, dd)
    }

    /// Horizontal flexbox container (`<hstack>`).
    pub fn create_hstack() -> Node {
        let w = container_widget();
        let mut s = Style::default();
        s.flex_direction = FlexDirection::Row;
        Node::from_view(w, s).with_tag("hstack")
    }

    /// Vertical flexbox container (`<vstack>` / `<stack_view>`).
    pub fn create_vstack() -> Node {
        let w = container_widget();
        let mut s = Style::default();
        s.flex_direction = FlexDirection::Column;
        Node::from_view(w, s).with_tag("vstack")
    }

    /// Bare flexbox container — direction defaults to Row unless the
    /// builder sets it. `<stack>` / `<view>` route here.
    pub fn create_stack() -> Node {
        let w = container_widget();
        Node::from_view(w, Style::default()).with_tag("stack")
    }

    /// 2-D grid container backed by Taffy's grid algorithm. Template
    /// tracks / gap / placement attrs are applied by the higher-level
    /// builder.
    pub fn create_grid() -> Node {
        let w = container_widget();
        let mut s = Style::default();
        s.display = crate::layout::Display::Grid;
        Node::from_view(w, s).with_tag("grid")
    }
}

/// Bare GtkWidget that hosts arbitrary children laid out by our
/// `TaffyLayout`. (Same helper that used to live in `node.rs`.)
pub(crate) fn container_widget() -> gtk4::Widget {
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    b.upcast()
}
