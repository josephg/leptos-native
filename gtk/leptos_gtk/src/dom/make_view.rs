//! Typed per-control [`GtkElem`] constructors.
//!
//! Each function here allocates a concrete gtk4 widget class
//! (`gtk::Button`, `gtk::Label`, ...), builds its default Taffy
//! [`Style`], and registers it in `tree` via [`GtkNodeExt::new_from_widget`].
//! Every typed builder in `leptos_gtk` calls exactly one of these
//! from its `Render::build` (`GtkElem::create_button()`, …).
//!
//! These are exposed via the [`GtkMakeView`] extension trait (inherent
//! impls on the foreign `Node<GtkBackend>` alias aren't possible — see
//! [`crate::dom::node`]). Bring `GtkMakeView` into scope to call
//! `GtkElem::create_button()` etc.

use crate::dom::{
    layout::{FlexDirection, Style},
    node::{GtkElem, GtkNodeExt},
};
use gtk4::prelude::*;

/// Typed widget constructors for [`GtkElem`]. See the module docs.
pub trait GtkMakeView: Sized {
    fn create_button() -> (GtkElem, gtk4::Button);
    fn create_checkbox() -> (GtkElem, gtk4::CheckButton);
    fn create_label() -> (GtkElem, gtk4::Label);
    fn create_text_field() -> (GtkElem, gtk4::Entry);
    fn create_secure_text_field() -> (GtkElem, gtk4::PasswordEntry);
    fn create_slider() -> (GtkElem, gtk4::Scale);
    fn create_pop_up_button() -> (GtkElem, gtk4::DropDown);
    fn create_hstack() -> GtkElem;
    fn create_vstack() -> GtkElem;
    fn create_stack() -> GtkElem;
    fn create_grid() -> GtkElem;
}

impl GtkMakeView for GtkElem {
    fn create_button() -> (GtkElem, gtk4::Button) {
        let b = gtk4::Button::new();
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = GtkElem::new_from_widget(b.clone(), s).with_tag("button");
        (n, b)
    }

    fn create_checkbox() -> (GtkElem, gtk4::CheckButton) {
        let c = gtk4::CheckButton::new();
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = GtkElem::new_from_widget(c.clone(), s).with_tag("checkbox");
        (n, c)
    }

    /// Wrapping multi-line label. Default left-aligned text — matches
    /// AppKit's `wrappingLabel` default. Wraps on overflow.
    fn create_label() -> (GtkElem, gtk4::Label) {
        let l = gtk4::Label::new(None);
        l.set_xalign(0.0);
        l.set_wrap(true);
        l.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = GtkElem::new_from_widget(l.clone(), s).with_tag("label");
        (n, l)
    }

    fn create_text_field() -> (GtkElem, gtk4::Entry) {
        let e = gtk4::Entry::new();
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = GtkElem::new_from_widget(e.clone(), s).with_tag("text_field");
        (n, e)
    }

    fn create_secure_text_field() -> (GtkElem, gtk4::PasswordEntry) {
        let e = gtk4::PasswordEntry::new();
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = GtkElem::new_from_widget(e.clone(), s).with_tag("secure_text_field");
        (n, e)
    }

    /// Horizontal `gtk::Scale` with the numeric value display off
    /// (the slider is just the thumb + track).
    fn create_slider() -> (GtkElem, gtk4::Scale) {
        let scale = gtk4::Scale::new(gtk4::Orientation::Horizontal, None::<&gtk4::Adjustment>);
        scale.set_draw_value(false);
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = GtkElem::new_from_widget(scale.clone(), s).with_tag("slider");
        (n, scale)
    }

    fn create_pop_up_button() -> (GtkElem, gtk4::DropDown) {
        let dd = gtk4::DropDown::default();
        let mut s = Style::default();
        s.flex_shrink = 0.0;
        let n = GtkElem::new_from_widget(dd.clone(), s).with_tag("pop_up_button");
        (n, dd)
    }

    /// Horizontal flexbox container (`<hstack>`).
    fn create_hstack() -> GtkElem {
        let w = container_widget();
        let mut s = Style::default();
        s.flex_direction = FlexDirection::Row;
        GtkElem::new_from_widget(w, s).with_tag("hstack")
    }

    /// Vertical flexbox container (`<vstack>` / `<stack_view>`).
    fn create_vstack() -> GtkElem {
        let w = container_widget();
        let mut s = Style::default();
        s.flex_direction = FlexDirection::Column;
        GtkElem::new_from_widget(w, s).with_tag("vstack")
    }

    /// Bare flexbox container — direction defaults to Row unless the
    /// builder sets it. `<stack>` / `<view>` route here.
    fn create_stack() -> GtkElem {
        let w = container_widget();
        GtkElem::new_from_widget(w, Style::default()).with_tag("stack")
    }

    /// 2-D grid container backed by Taffy's grid algorithm. Template
    /// tracks / gap / placement attrs are applied by the higher-level
    /// builder.
    fn create_grid() -> GtkElem {
        let w = container_widget();
        let mut s = Style::default();
        s.display = crate::dom::layout::Display::Grid;
        GtkElem::new_from_widget(w, s).with_tag("grid")
    }
}

/// Bare GtkWidget that hosts arbitrary children laid out by our
/// `TaffyLayout`. (Same helper that used to live in `node.rs`.)
pub(crate) fn container_widget() -> gtk4::Widget {
    let b = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    b.upcast()
}
