//! Element builder types for iOS: `view()`, `button()`, `label()`,
//! `text_field()`, `slider()`, `switch_()`, etc.
//!
//! Each builder returns a struct that implements [`Render`] from
//! tachys' view core. Building emits an [`ios_dom::Element`] (or
//! similar leaf), wires attributes (with reactive effects for
//! signal-driven values), recursively builds children, and mounts
//! them. Direct port of the cocoa pattern in
//! [`crate::cocoa::element`] — same shape, UIKit-flavoured.

use super::attr::{install, IntoMaybeReactive, MaybeReactive};
use crate::{
    event_ios::{EventDescriptor, PendingHandler, SupportsEvent},
    Dom,
};
use renderer::view::{Mountable, Render};
use ios_dom::{
    layout::{
        set_aspect_ratio, set_flex_direction, set_flex_grow, set_gap,
        set_inset, set_padding, set_position, FlexDirection, Position,
    },
    BoolAttr, Element as IosElement, StringAttr,
};
use reactive_graph::effect::RenderEffect;

// ---------------------------------------------------------------------
// Universal NSView/UIView attrs — applied by every builder.
// iOS doesn't have tooltips (a macOS hover affordance) so this is
// just `alpha` for now; extending later is one place.
// ---------------------------------------------------------------------

fn apply_universal(
    el: &IosElement,
    alpha: Option<MaybeReactive<f64>>,
) -> Vec<RenderEffect<()>> {
    let mut out = Vec::new();
    if let Some(a) = alpha {
        let el_for = el.clone();
        if let Some(eff) = install(a, move |v| el_for.set_alpha(v)) {
            out.push(eff);
        }
    }
    out
}

/// Install the text-styling attributes shared by text-bearing
/// builders (label, text_field, secure_text_field). Each attr is
/// `MaybeReactive<T>`; effects are returned for the caller to
/// stash in the State.
fn apply_text_attrs(
    el: &IosElement,
    text_color: Option<MaybeReactive<ios_dom::Color>>,
    alignment: Option<MaybeReactive<ios_dom::NSTextAlignment>>,
    font_size: Option<MaybeReactive<f64>>,
) -> Vec<RenderEffect<()>> {
    let mut out = Vec::new();
    if let Some(c) = text_color {
        let el_for = el.clone();
        if let Some(eff) = install(c, move |v| el_for.set_text_color(v)) {
            out.push(eff);
        }
    }
    if let Some(a) = alignment {
        let el_for = el.clone();
        if let Some(eff) =
            install(a, move |v| el_for.set_text_alignment(v))
        {
            out.push(eff);
        }
    }
    if let Some(s) = font_size {
        let el_for = el.clone();
        if let Some(eff) = install(s, move |v| el_for.set_font_size(v)) {
            out.push(eff);
        }
    }
    out
}

/// Apply chrome attributes — background color, corner radius,
/// border width + color. All four sit on the underlying UIView
/// (or its CALayer); each is reactive via `MaybeReactive`.
fn apply_chrome(
    el: &IosElement,
    background_color: Option<MaybeReactive<ios_dom::Color>>,
    corner_radius: Option<MaybeReactive<f64>>,
    border_width: Option<MaybeReactive<f64>>,
    border_color: Option<MaybeReactive<ios_dom::Color>>,
) -> Vec<RenderEffect<()>> {
    let mut out = Vec::new();
    if let Some(c) = background_color {
        let el_for = el.clone();
        if let Some(eff) =
            install(c, move |v| el_for.set_background_color(Some(v)))
        {
            out.push(eff);
        }
    }
    if let Some(r) = corner_radius {
        let el_for = el.clone();
        if let Some(eff) = install(r, move |v| el_for.set_corner_radius(v))
        {
            out.push(eff);
        }
    }
    if let Some(w) = border_width {
        let el_for = el.clone();
        if let Some(eff) = install(w, move |v| el_for.set_border_width(v))
        {
            out.push(eff);
        }
    }
    if let Some(c) = border_color {
        let el_for = el.clone();
        if let Some(eff) =
            install(c, move |v| el_for.set_border_color(Some(v)))
        {
            out.push(eff);
        }
    }
    out
}

/// Generates `alpha(<reactive f64>)` builder method on `$builder<At>`.
/// (No `tool_tip` analogue on iOS — tooltips are a macOS hover concept.)
macro_rules! impl_universal_attrs {
    ($builder:ident) => {
        impl $builder {
            /// View opacity, 0.0..=1.0. Maps to UIView's `alpha`.
            /// Reactive: pass an f64 or a closure.
            pub fn alpha<V>(mut self, a: V) -> Self
            where
                V: IntoMaybeReactive<f64>,
            {
                self.alpha = Some(a.into_maybe_reactive());
                self
            }
        }
    };
}

/// Inherent-method block for text-styling attrs. Same pattern as
/// `impl_universal_attrs!`, reactive over Color / NSTextAlignment / f64.
macro_rules! impl_text_attrs {
    ($builder:ident) => {
        impl $builder {
            pub fn text_color<V>(mut self, c: V) -> Self
            where
                V: IntoMaybeReactive<ios_dom::Color>,
            {
                self.text_color = Some(c.into_maybe_reactive());
                self
            }
            /// Text alignment within the control's frame.
            pub fn alignment<V>(mut self, a: V) -> Self
            where
                V: IntoMaybeReactive<ios_dom::NSTextAlignment>,
            {
                self.alignment = Some(a.into_maybe_reactive());
                self
            }
            /// Font size in points (system font at this size).
            pub fn font_size<V>(mut self, p: V) -> Self
            where
                V: IntoMaybeReactive<f64>,
            {
                self.font_size = Some(p.into_maybe_reactive());
                self
            }
        }
    };
}

/// Adds `background_color` / `corner_radius` / `border_width` /
/// `border_color` builder methods. The struct must have those four
/// fields as `Option<MaybeReactive<...>>`.
///
/// Currently `View` wires chrome attrs by hand because of its
/// `<Children, At>` two-parameter shape. This macro is here for
/// when chrome lands on Button / Label / etc. Until then, mark
/// it `unused_macros`-allowed so the warning doesn't drown the
/// build log.
#[allow(unused_macros)]
macro_rules! impl_chrome_attrs {
    ($builder:ident) => {
        impl $builder {
            /// Background fill colour. Pass a `Color` (e.g.
            /// `Color::SYSTEM_BACKGROUND`) or a closure.
            pub fn background_color<V>(mut self, c: V) -> Self
            where
                V: IntoMaybeReactive<ios_dom::Color>,
            {
                self.background_color = Some(c.into_maybe_reactive());
                self
            }
            /// Rounded corners (in points). 0 = square (default).
            /// Sets `layer.cornerRadius` + `masksToBounds=true` so
            /// children clip to the rounded shape.
            pub fn corner_radius<V>(mut self, r: V) -> Self
            where
                V: IntoMaybeReactive<f64>,
            {
                self.corner_radius = Some(r.into_maybe_reactive());
                self
            }
            /// Border width in points (default 0). Pair with
            /// `border_color` — a width with no colour shows
            /// transparent.
            pub fn border_width<V>(mut self, w: V) -> Self
            where
                V: IntoMaybeReactive<f64>,
            {
                self.border_width = Some(w.into_maybe_reactive());
                self
            }
            /// Border colour. See `border_width` for thickness.
            pub fn border_color<V>(mut self, c: V) -> Self
            where
                V: IntoMaybeReactive<ios_dom::Color>,
            {
                self.border_color = Some(c.into_maybe_reactive());
                self
            }
        }
    };
}


// ---------------------------------------------------------------------
// ElementState — generic state for every builder
// ---------------------------------------------------------------------

pub struct ElementState<AttrState, ChildState> {
    pub el: IosElement,
    pub(crate) _effects: Vec<RenderEffect<()>>,
    pub(crate) _attrs: std::marker::PhantomData<AttrState>,
    pub(crate) children: ChildState,
}

impl<AttrState, ChildState: Mountable<Dom>> Mountable<Dom>
    for ElementState<AttrState, ChildState>
{
    fn unmount(&mut self) {
        self.children.unmount();
        self.el.as_node().teardown();
    }

    fn mount(
        &mut self,
        parent: &IosElement,
        marker: Option<&ios_dom::Node>,
    ) {
        parent.insert_node(self.el.as_node(), marker);
        self.children.mount(&self.el, None);
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<IosElement> {
        vec![self.el.clone()]
    }
}

// ---------------------------------------------------------------------
// view() — generic UIView container
// ---------------------------------------------------------------------

pub struct View<Children> {
    flex_direction: Option<FlexDirection>,
    padding: Option<f32>,
    gap: Option<f32>,
    flex_grow: Option<f32>,
    aspect_ratio: Option<f32>,
    position_absolute: bool,
    /// Insets used when `position_absolute`. `None` = `auto`.
    inset_top: Option<f32>,
    inset_right: Option<f32>,
    inset_bottom: Option<f32>,
    inset_left: Option<f32>,
    alpha: Option<MaybeReactive<f64>>,
    background_color: Option<MaybeReactive<ios_dom::Color>>,
    corner_radius: Option<MaybeReactive<f64>>,
    border_width: Option<MaybeReactive<f64>>,
    border_color: Option<MaybeReactive<ios_dom::Color>>,
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(&IosElement) + Send + 'static>>,
    children: Children,
}

pub fn view() -> View<()> {
    View {
        flex_direction: None,
        padding: None,
        gap: None,
        flex_grow: None,
        aspect_ratio: None,
        position_absolute: false,
        inset_top: None,
        inset_right: None,
        inset_bottom: None,
        inset_left: None,
        alpha: None,
        background_color: None,
        corner_radius: None,
        border_width: None,
        border_color: None,
        handlers: Vec::new(),
        pending_spreads: Vec::new(),
        children: (),
    }
}

impl<Ch> View<Ch> {
    pub fn flex_direction(mut self, dir: FlexDirection) -> Self {
        self.flex_direction = Some(dir);
        self
    }
    pub fn padding(mut self, p: f32) -> Self {
        self.padding = Some(p);
        self
    }
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = Some(g);
        self
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    /// Aspect ratio (width / height). `1.0` makes the view square,
    /// useful for photo cells in a grid.
    pub fn aspect_ratio(mut self, r: f32) -> Self {
        self.aspect_ratio = Some(r);
        self
    }
    /// Take this view out of the parent's flex flow and position
    /// it absolutely against the parent's content area. Combine
    /// with `inset_top` / `inset_right` / `inset_bottom` /
    /// `inset_left` to anchor. Useful for badge overlays — a star
    /// in the top-right of a photo cell, a "RAW" chip in the
    /// top-left, etc.
    ///
    /// Takes a `bool` so the `view!{}` macro's `attr=value`
    /// shorthand works: `<vstack position_absolute=true>`.
    pub fn position_absolute(mut self, abs: bool) -> Self {
        self.position_absolute = abs;
        self
    }
    pub fn inset_top(mut self, v: f32) -> Self {
        self.inset_top = Some(v);
        self
    }
    pub fn inset_right(mut self, v: f32) -> Self {
        self.inset_right = Some(v);
        self
    }
    pub fn inset_bottom(mut self, v: f32) -> Self {
        self.inset_bottom = Some(v);
        self
    }
    pub fn inset_left(mut self, v: f32) -> Self {
        self.inset_left = Some(v);
        self
    }
    pub fn alpha<V: IntoMaybeReactive<f64>>(mut self, a: V) -> Self {
        self.alpha = Some(a.into_maybe_reactive());
        self
    }
    pub fn background_color<V: IntoMaybeReactive<ios_dom::Color>>(
        mut self,
        c: V,
    ) -> Self {
        self.background_color = Some(c.into_maybe_reactive());
        self
    }
    pub fn corner_radius<V: IntoMaybeReactive<f64>>(mut self, r: V) -> Self {
        self.corner_radius = Some(r.into_maybe_reactive());
        self
    }
    pub fn border_width<V: IntoMaybeReactive<f64>>(mut self, w: V) -> Self {
        self.border_width = Some(w.into_maybe_reactive());
        self
    }
    pub fn border_color<V: IntoMaybeReactive<ios_dom::Color>>(
        mut self,
        c: V,
    ) -> Self {
        self.border_color = Some(c.into_maybe_reactive());
        self
    }
    pub fn child<NewCh>(self, child: NewCh) -> View<(Ch, NewCh)> {
        View {
            flex_direction: self.flex_direction,
            padding: self.padding,
            gap: self.gap,
            flex_grow: self.flex_grow,
            aspect_ratio: self.aspect_ratio,
            position_absolute: self.position_absolute,
            inset_top: self.inset_top,
            inset_right: self.inset_right,
            inset_bottom: self.inset_bottom,
            inset_left: self.inset_left,
            alpha: self.alpha,
            background_color: self.background_color,
            corner_radius: self.corner_radius,
            border_width: self.border_width,
            border_color: self.border_color,
            handlers: self.handlers,
            pending_spreads: self.pending_spreads,
            children: (self.children, child),
        }
    }
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

// `<view on:click=...>` works via UITapGestureRecognizer (installed
// in `Element::on_click` when the underlying view isn't a UIControl).
// Plain UIView, UILabel, UIImageView etc. all route through that
// fallback.
impl<Ch> SupportsEvent<crate::event_ios::ClickEvent> for View<Ch> {}

impl<Ch: Render<Dom>> Render<Dom> for View<Ch> {
    type State = ElementState<(), Ch::State>;
    fn build(self) -> Self::State {
        let el = IosElement::create("view");
        let mut effects = Vec::new();
        if let Some(dir) = self.flex_direction {
            set_flex_direction(el.as_node(), dir);
        }
        if let Some(p) = self.padding {
            set_padding(el.as_node(), p);
        }
        if let Some(g) = self.gap {
            set_gap(el.as_node(), g);
        }
        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }
        if let Some(r) = self.aspect_ratio {
            set_aspect_ratio(el.as_node(), r);
        }
        if self.position_absolute {
            set_position(el.as_node(), Position::Absolute);
            set_inset(
                el.as_node(),
                self.inset_top,
                self.inset_right,
                self.inset_bottom,
                self.inset_left,
            );
        }
        effects.extend(apply_universal(&el, self.alpha));
        effects.extend(apply_chrome(
            &el,
            self.background_color,
            self.corner_radius,
            self.border_width,
            self.border_color,
        ));
        let child_state = self.children.build();
        for handler in self.handlers {
            handler.apply_to(&el);
        }
        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: child_state,
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}


pub fn vstack() -> View<()> {
    view().flex_direction(FlexDirection::Column)
}
pub fn hstack() -> View<()> {
    view().flex_direction(FlexDirection::Row)
}

// ---------------------------------------------------------------------
// button() — UIButton with title + on:click
// ---------------------------------------------------------------------

pub struct Button {
    title: MaybeReactive<String>,
    enabled: Option<MaybeReactive<bool>>,
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(&IosElement) + Send + 'static>>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
    font_size: Option<MaybeReactive<f64>>,
}

pub fn button() -> Button {
    Button {
        title: MaybeReactive::Static(String::new()),
        enabled: None,
        handlers: Vec::new(),
        pending_spreads: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
        font_size: None,
    }
}

impl Button {
    pub fn title<V: IntoMaybeReactive<String>>(mut self, t: V) -> Self {
        self.title = t.into_maybe_reactive();
        self
    }
    /// `<button>"X"</button>` — macro forwards the child here.
    pub fn child<V: IntoMaybeReactive<String>>(self, value: V) -> Self {
        self.title(value)
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn font_size<V: IntoMaybeReactive<f64>>(mut self, p: V) -> Self {
        self.font_size = Some(p.into_maybe_reactive());
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl SupportsEvent<crate::event_ios::ClickEvent> for Button {}

impl_universal_attrs!(Button);


impl Render<Dom> for Button {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let el = IosElement::create("button");
        let mut effects = Vec::new();

        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_string_attribute(StringAttr::Title, &t);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for_en = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_en.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }

        if let Some(s) = self.font_size {
            let el_for = el.clone();
            if let Some(eff) =
                install(s, move |v| el_for.set_font_size(v))
            {
                effects.push(eff);
            }
        }

        for h in self.handlers {
            h.apply_to(&el);
        }
        for f in self.pending_spreads { f(&el); }


        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }



        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// label() — UILabel
// ---------------------------------------------------------------------

pub struct Label {
    text: MaybeReactive<String>,
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(&IosElement) + Send + 'static>>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
    text_color: Option<MaybeReactive<ios_dom::Color>>,
    alignment: Option<MaybeReactive<ios_dom::NSTextAlignment>>,
    font_size: Option<MaybeReactive<f64>>,
    pending_bind_text:
        Option<Box<dyn Fn() -> String + Send + 'static>>,
}

pub fn label() -> Label {
    Label {
        text: MaybeReactive::Static(String::new()),
        handlers: Vec::new(),
        pending_spreads: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
        text_color: None,
        alignment: None,
        font_size: None,
        pending_bind_text: None,
    }
}

impl Label {
    pub fn text<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.text = v.into_maybe_reactive();
        self
    }
    /// `<label>"X"</label>` or `<label>{closure}</label>`.
    pub fn child<V: IntoMaybeReactive<String>>(self, value: V) -> Self {
        self.text(value)
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
    /// Internal: stash a `bind:value=...` for installation in `build`.
    pub(crate) fn set_pending_bind_text(
        &mut self,
        getter: Box<dyn Fn() -> String + Send + 'static>,
    ) {
        self.pending_bind_text = Some(getter);
    }
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl SupportsEvent<crate::event_ios::ClickEvent> for Label {}

impl_universal_attrs!(Label);
impl_text_attrs!(Label);


impl Render<Dom> for Label {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let el = IosElement::create("label");
        let mut effects = Vec::new();

        // bind:value getter wins over .text(...) — same as cocoa.
        let text = match self.pending_bind_text {
            Some(getter) => MaybeReactive::Reactive(getter),
            None => self.text,
        };
        let el_for_text = el.clone();
        if let Some(eff) = install(text, move |s| {
            el_for_text.set_string_attribute(StringAttr::Title, &s);
        }) {
            effects.push(eff);
        }

        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }
        for f in self.pending_spreads { f(&el); }


        effects.extend(apply_universal(&el, self.alpha));
        effects.extend(apply_text_attrs(
            &el,
            self.text_color,
            self.alignment,
            self.font_size,
        ));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }



        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// text_field() / secure_text_field() — UITextField
// ---------------------------------------------------------------------

pub struct TextField {
    value: MaybeReactive<String>,
    placeholder: Option<String>,
    enabled: Option<MaybeReactive<bool>>,
    /// If `true`, build with `secureTextEntry = YES`. Used by the
    /// `secure_text_field()` constructor; same builder otherwise.
    secure: bool,
    pending_bind: Option<crate::ios::bind::BoundValue>,
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(&IosElement) + Send + 'static>>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
    text_color: Option<MaybeReactive<ios_dom::Color>>,
    alignment: Option<MaybeReactive<ios_dom::NSTextAlignment>>,
    font_size: Option<MaybeReactive<f64>>,
}

pub fn text_field() -> TextField {
    TextField {
        value: MaybeReactive::Static(String::new()),
        placeholder: None,
        enabled: None,
        secure: false,
        pending_bind: None,
        handlers: Vec::new(),
        pending_spreads: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
        text_color: None,
        alignment: None,
        font_size: None,
    }
}

/// Password-masking variant of [`text_field`]. Builds a UITextField
/// with `secureTextEntry = YES`.
pub fn secure_text_field() -> TextField {
    TextField {
        secure: true,
        ..text_field()
    }
}

impl TextField {
    pub fn value<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.value = v.into_maybe_reactive();
        self
    }
    pub fn placeholder(mut self, s: impl Into<String>) -> Self {
        self.placeholder = Some(s.into());
        self
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::ios::bind::BoundValue,
    ) {
        self.pending_bind = Some(bound);
    }
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

// Text fields fire on every keystroke (`input`) and on commit
// (`change` — return key / focus loss). Click is deliberately not
// supported — clicking inside the field places the caret. Focus/
// blur are UIControl `editingDidBegin` / `editingDidEnd`.
impl SupportsEvent<crate::event_ios::InputEvent> for TextField {}
impl SupportsEvent<crate::event_ios::ChangeEvent> for TextField {}
impl SupportsEvent<crate::event_ios::FocusEvent> for TextField {}
impl SupportsEvent<crate::event_ios::BlurEvent> for TextField {}

impl_universal_attrs!(TextField);
impl_text_attrs!(TextField);


impl Render<Dom> for TextField {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let tag = if self.secure { "secure_text_field" } else { "text_field" };
        let el = IosElement::create(tag);
        let mut effects = Vec::new();

        if let Some(p) = self.placeholder {
            el.set_string_attribute(StringAttr::Placeholder, &p);
        }

        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        // One-way `.value(...)` — applied even when bind:value is
        // present; the bind effect installs second so it wins on
        // subsequent ticks.
        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_string_attribute(StringAttr::Value, &v);
        }) {
            effects.push(eff);
        }

        // Two-way `bind:value=signal`.
        if let Some(bound) = self.pending_bind {
            let eff = crate::ios::bind::install_text_field_value_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }
        for f in self.pending_spreads { f(&el); }


        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }


        effects.extend(apply_universal(&el, self.alpha));
        effects.extend(apply_text_attrs(
            &el,
            self.text_color,
            self.alignment,
            self.font_size,
        ));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }


        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// switch_() — UISwitch
//
// Trailing underscore because `switch` is a Rust keyword. The
// `view!{}` macro maps `<switch>` → `switch_()`.
// ---------------------------------------------------------------------

pub struct Switch {
    checked: MaybeReactive<bool>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_checked: Option<crate::ios::bind::BoundChecked>,
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(&IosElement) + Send + 'static>>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
}

pub fn switch_() -> Switch {
    Switch {
        checked: MaybeReactive::Static(false),
        enabled: None,
        pending_bind_checked: None,
        handlers: Vec::new(),
        pending_spreads: Vec::new(),
        node_ref: None,
        alpha: None,
    }
}

impl Switch {
    pub fn checked<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.checked = v.into_maybe_reactive();
        self
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
    pub(crate) fn set_pending_bind_checked(
        &mut self,
        bound: crate::ios::bind::BoundChecked,
    ) {
        self.pending_bind_checked = Some(bound);
    }
    /// `<switch on:click=...>` — fires whenever the user toggles
    /// the switch (UIControlEventValueChanged routed through the
    /// shared `on_click` path).
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl SupportsEvent<crate::event_ios::ClickEvent> for Switch {}

impl_universal_attrs!(Switch);


impl Render<Dom> for Switch {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let el = IosElement::create("switch");
        let mut effects = Vec::new();

        // One-way `.checked(...)`.
        let el_for_checked = el.clone();
        if let Some(eff) = install(self.checked, move |b| {
            el_for_checked.set_bool_attribute(BoolAttr::Checked, b);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        // Two-way `bind:checked=signal`.
        if let Some(bound) = self.pending_bind_checked {
            let eff =
                crate::ios::bind::install_switch_checked_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }
        for f in self.pending_spreads { f(&el); }


        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }



        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// slider() — UISlider
// ---------------------------------------------------------------------

pub struct Slider {
    value: MaybeReactive<f64>,
    min_value: f64,
    max_value: f64,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::ios::bind::BoundFloat>,
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(&IosElement) + Send + 'static>>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
}

pub fn slider() -> Slider {
    Slider {
        value: MaybeReactive::Static(0.0),
        min_value: 0.0,
        max_value: 1.0,
        enabled: None,
        pending_bind: None,
        handlers: Vec::new(),
        pending_spreads: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
    }
}

impl Slider {
    pub fn value<V: IntoMaybeReactive<f64>>(mut self, v: V) -> Self {
        self.value = v.into_maybe_reactive();
        self
    }
    pub fn min_value(mut self, v: f64) -> Self {
        self.min_value = v;
        self
    }
    pub fn max_value(mut self, v: f64) -> Self {
        self.max_value = v;
        self
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::ios::bind::BoundFloat,
    ) {
        self.pending_bind = Some(bound);
    }
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl SupportsEvent<crate::event_ios::ClickEvent> for Slider {}

impl_universal_attrs!(Slider);


impl Render<Dom> for Slider {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let el = IosElement::create("slider");
        let mut effects = Vec::new();

        // min/max set FIRST so initial set_double_value clamps correctly.
        el.set_slider_min(self.min_value);
        el.set_slider_max(self.max_value);

        let el_for_value = el.clone();
        if let Some(eff) =
            install(self.value, move |v| el_for_value.set_double_value(v))
        {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind {
            let eff =
                crate::ios::bind::install_slider_value_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }
        for f in self.pending_spreads { f(&el); }


        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }


        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }


        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// stepper() — UIStepper (+/-)
// ---------------------------------------------------------------------

pub struct Stepper {
    value: MaybeReactive<f64>,
    min_value: f64,
    max_value: f64,
    increment: f64,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::ios::bind::BoundFloat>,
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(&IosElement) + Send + 'static>>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
}

pub fn stepper() -> Stepper {
    Stepper {
        value: MaybeReactive::Static(0.0),
        min_value: 0.0,
        max_value: 100.0,
        increment: 1.0,
        enabled: None,
        pending_bind: None,
        handlers: Vec::new(),
        pending_spreads: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
    }
}

impl Stepper {
    pub fn value<V: IntoMaybeReactive<f64>>(mut self, v: V) -> Self {
        self.value = v.into_maybe_reactive();
        self
    }
    pub fn min_value(mut self, v: f64) -> Self {
        self.min_value = v;
        self
    }
    pub fn max_value(mut self, v: f64) -> Self {
        self.max_value = v;
        self
    }
    pub fn increment(mut self, v: f64) -> Self {
        self.increment = v;
        self
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::ios::bind::BoundFloat,
    ) {
        self.pending_bind = Some(bound);
    }
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl SupportsEvent<crate::event_ios::ClickEvent> for Stepper {}

impl_universal_attrs!(Stepper);


impl Render<Dom> for Stepper {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let el = IosElement::create("stepper");
        let mut effects = Vec::new();

        // Bounds + increment first so the initial value clamps.
        el.configure_stepper(self.min_value, self.max_value, self.increment);

        let el_for_val = el.clone();
        if let Some(eff) =
            install(self.value, move |v| el_for_val.set_stepper_value(v))
        {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind {
            let eff =
                crate::ios::bind::install_stepper_value_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }
        for f in self.pending_spreads { f(&el); }


        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }


        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }


        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// progress_indicator() — UIProgressIndicator (determinate bar, 0..1)
//
// iOS doesn't ship an indeterminate progress bar — that's
// `UIActivityIndicatorView`, a separate builder. So the
// `indeterminate` field from cocoa's `ProgressIndicator` doesn't
// carry over. Tagged `progress_indicator` for cross-port name parity.
// ---------------------------------------------------------------------

pub struct ProgressIndicator {
    value: MaybeReactive<f64>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
}

pub fn progress_indicator() -> ProgressIndicator {
    ProgressIndicator {
        value: MaybeReactive::Static(0.0),
        flex_grow: None,
        node_ref: None,
        alpha: None,
    }
}

impl ProgressIndicator {
    pub fn value<V: IntoMaybeReactive<f64>>(mut self, v: V) -> Self {
        self.value = v.into_maybe_reactive();
        self
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
}

impl_universal_attrs!(ProgressIndicator);


impl Render<Dom> for ProgressIndicator {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let el = IosElement::create("progress_indicator");
        let mut effects = Vec::new();

        let el_for = el.clone();
        if let Some(eff) =
            install(self.value, move |v| el_for.set_progress_value(v))
        {
            effects.push(eff);
        }

        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }

        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }


        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// image_view() — UIImageView, source from a file path
// ---------------------------------------------------------------------

pub struct ImageView {
    source: MaybeReactive<String>,
    flex_grow: Option<f32>,
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(&IosElement) + Send + 'static>>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
}

pub fn image_view() -> ImageView {
    ImageView {
        source: MaybeReactive::Static(String::new()),
        flex_grow: None,
        handlers: Vec::new(),
        pending_spreads: Vec::new(),
        node_ref: None,
        alpha: None,
    }
}

impl ImageView {
    /// File path to the image. Empty string clears the image.
    /// Network URLs aren't supported here — fetch them yourself
    /// and pass the local path. `UIImage::imageWithContentsOfFile:`
    /// handles PNG, JPEG, PDF, etc.
    pub fn source<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.source = v.into_maybe_reactive();
        self
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

// `<image_view on:click=...>` lands on a UITapGestureRecognizer via
// the on_click → on_tap_gesture fallback.
impl SupportsEvent<crate::event_ios::ClickEvent> for ImageView {}

impl_universal_attrs!(ImageView);


impl Render<Dom> for ImageView {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let el = IosElement::create("image_view");
        let mut effects = Vec::new();

        let el_for = el.clone();
        if let Some(eff) =
            install(self.source, move |s| el_for.set_image_view_path(&s))
        {
            effects.push(eff);
        }

        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }
        for f in self.pending_spreads { f(&el); }


        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }



        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// segmented_control() — UISegmentedControl
// ---------------------------------------------------------------------

pub struct SegmentedControl {
    items: Vec<String>,
    selection: MaybeReactive<usize>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_selection: Option<crate::ios::bind::BoundIndex>,
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(&IosElement) + Send + 'static>>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
}

pub fn segmented_control() -> SegmentedControl {
    SegmentedControl {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        enabled: None,
        pending_bind_selection: None,
        handlers: Vec::new(),
        pending_spreads: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
    }
}

impl SegmentedControl {
    pub fn items<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.items = items.into_iter().map(Into::into).collect();
        self
    }
    pub fn selection<V: IntoMaybeReactive<usize>>(mut self, v: V) -> Self {
        self.selection = v.into_maybe_reactive();
        self
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
    pub(crate) fn set_pending_bind_selection(
        &mut self,
        bound: crate::ios::bind::BoundIndex,
    ) {
        self.pending_bind_selection = Some(bound);
    }
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl SupportsEvent<crate::event_ios::ClickEvent> for SegmentedControl {}

impl_universal_attrs!(SegmentedControl);


impl Render<Dom> for SegmentedControl {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let el = IosElement::create("segmented_control");
        let mut effects = Vec::new();

        el.set_segmented_items(&self.items);

        let el_for = el.clone();
        if let Some(eff) = install(self.selection, move |i| {
            el_for.set_segmented_selection(i as isize);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind_selection {
            let eff = crate::ios::bind::install_segmented_selection_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }
        for f in self.pending_spreads { f(&el); }


        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }


        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }


        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// date_picker() — UIDatePicker
// ---------------------------------------------------------------------

pub struct DatePicker {
    value: MaybeReactive<ios_dom::Date>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::ios::bind::BoundDate>,
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(&IosElement) + Send + 'static>>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
    style: Option<MaybeReactive<ios_dom::UIDatePickerStyle>>,
    min_date: Option<MaybeReactive<ios_dom::Date>>,
    max_date: Option<MaybeReactive<ios_dom::Date>>,
}

pub fn date_picker() -> DatePicker {
    DatePicker {
        value: MaybeReactive::Static(ios_dom::Date::now()),
        enabled: None,
        pending_bind: None,
        handlers: Vec::new(),
        pending_spreads: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
        style: None,
        min_date: None,
        max_date: None,
    }
}

impl DatePicker {
    pub fn value<V: IntoMaybeReactive<ios_dom::Date>>(mut self, v: V) -> Self {
        self.value = v.into_maybe_reactive();
        self
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
    pub(crate) fn set_pending_bind_date(
        &mut self,
        bound: crate::ios::bind::BoundDate,
    ) {
        self.pending_bind = Some(bound);
    }
    /// Visual style: `Wheels`, `Compact` (default), `Inline`,
    /// `Automatic`. See `ios_dom::UIDatePickerStyle`.
    pub fn style<V>(mut self, s: V) -> Self
    where
        V: IntoMaybeReactive<ios_dom::UIDatePickerStyle>,
    {
        self.style = Some(s.into_maybe_reactive());
        self
    }
    pub fn min_date<V: IntoMaybeReactive<ios_dom::Date>>(mut self, d: V) -> Self {
        self.min_date = Some(d.into_maybe_reactive());
        self
    }
    pub fn max_date<V: IntoMaybeReactive<ios_dom::Date>>(mut self, d: V) -> Self {
        self.max_date = Some(d.into_maybe_reactive());
        self
    }
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl SupportsEvent<crate::event_ios::ClickEvent> for DatePicker {}

impl_universal_attrs!(DatePicker);


impl Render<Dom> for DatePicker {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let el = IosElement::create("date_picker");
        let mut effects = Vec::new();

        let el_for = el.clone();
        if let Some(eff) =
            install(self.value, move |d| el_for.set_date_picker_value(d))
        {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind {
            let eff = crate::ios::bind::install_date_picker_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }
        for f in self.pending_spreads { f(&el); }


        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }


        if let Some(s) = self.style {
            let el_for = el.clone();
            if let Some(eff) =
                install(s, move |v| el_for.set_date_picker_style(v))
            {
                effects.push(eff);
            }
        }
        if let Some(d) = self.min_date {
            let el_for = el.clone();
            if let Some(eff) =
                install(d, move |v| el_for.set_date_picker_min(Some(v)))
            {
                effects.push(eff);
            }
        }
        if let Some(d) = self.max_date {
            let el_for = el.clone();
            if let Some(eff) =
                install(d, move |v| el_for.set_date_picker_max(Some(v)))
            {
                effects.push(eff);
            }
        }

        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }


        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// scroll_view() — UIScrollView wrapping arbitrary child content
//
// The scroll view's content `UIView` is created at construction time
// in `ios_dom::Element::create_with` (it's the first subview of the
// `UIScrollView`); child mounts route there via
// `Element::subview_parent`. `apply_layout` special-cases scroll
// views: a second-pass relayout with `MaxContent` height lets
// children take their natural sizes, and `contentSize` is set to
// the union of children's rects so UIScrollView shows scroll bars.
// ---------------------------------------------------------------------

pub struct ScrollView<Children> {
    flex_grow: Option<f32>,
    children: Children,
    alpha: Option<MaybeReactive<f64>>,
    has_horizontal_scroller: Option<MaybeReactive<bool>>,
    has_vertical_scroller: Option<MaybeReactive<bool>>,
}

pub fn scroll_view() -> ScrollView<()> {
    ScrollView {
        flex_grow: None,
        children: (),
        alpha: None,
        has_horizontal_scroller: None,
        has_vertical_scroller: None,
    }
}

impl<Ch> ScrollView<Ch> {
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn alpha<V: IntoMaybeReactive<f64>>(mut self, a: V) -> Self {
        self.alpha = Some(a.into_maybe_reactive());
        self
    }
    pub fn has_horizontal_scroller<V: IntoMaybeReactive<bool>>(
        mut self,
        b: V,
    ) -> Self {
        self.has_horizontal_scroller = Some(b.into_maybe_reactive());
        self
    }
    pub fn has_vertical_scroller<V: IntoMaybeReactive<bool>>(
        mut self,
        b: V,
    ) -> Self {
        self.has_vertical_scroller = Some(b.into_maybe_reactive());
        self
    }
    pub fn child<NewCh>(self, child: NewCh) -> ScrollView<(Ch, NewCh)> {
        ScrollView {
            flex_grow: self.flex_grow,
            children: (self.children, child),
            alpha: self.alpha,
            has_horizontal_scroller: self.has_horizontal_scroller,
            has_vertical_scroller: self.has_vertical_scroller,
        }
    }
}

impl<Ch: Render<Dom>> Render<Dom> for ScrollView<Ch> {
    type State = ElementState<(), Ch::State>;
    fn build(self) -> Self::State {
        let el = IosElement::create("scroll_view");
        let mut effects = Vec::new();

        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }

        if let Some(b) = self.has_horizontal_scroller {
            let el_for = el.clone();
            if let Some(eff) =
                install(b, move |v| el_for.set_has_horizontal_scroller(v))
            {
                effects.push(eff);
            }
        }
        if let Some(b) = self.has_vertical_scroller {
            let el_for = el.clone();
            if let Some(eff) =
                install(b, move |v| el_for.set_has_vertical_scroller(v))
            {
                effects.push(eff);
            }
        }

        effects.extend(apply_universal(&el, self.alpha));

        let child_state = self.children.build();

        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: child_state,
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}


// ---------------------------------------------------------------------
// text_view() — UITextView (multi-line plain-text editor)
//
// UITextView IS already a UIScrollView subclass — it scrolls itself,
// no wrapper needed. `bind:value` goes through UITextViewDelegate's
// `textViewDidChange:` instead of UIControl target/action.
// ---------------------------------------------------------------------

pub struct TextView {
    value: MaybeReactive<String>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::ios::bind::BoundValue>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
    text_color: Option<MaybeReactive<ios_dom::Color>>,
    alignment: Option<MaybeReactive<ios_dom::NSTextAlignment>>,
    font_size: Option<MaybeReactive<f64>>,
}

pub fn text_view() -> TextView {
    TextView {
        value: MaybeReactive::Static(String::new()),
        enabled: None,
        pending_bind: None,
        flex_grow: None,
        node_ref: None,
        alpha: None,
        text_color: None,
        alignment: None,
        font_size: None,
    }
}

impl TextView {
    pub fn value<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.value = v.into_maybe_reactive();
        self
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    pub fn node_ref(mut self, r: crate::ios::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::ios::bind::BoundValue,
    ) {
        self.pending_bind = Some(bound);
    }
}

impl_universal_attrs!(TextView);
impl_text_attrs!(TextView);


impl Render<Dom> for TextView {
    type State = ElementState<(), ()>;
    fn build(self) -> Self::State {
        let el = IosElement::create("text_view");
        let mut effects = Vec::new();

        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_string_attribute(StringAttr::Value, &v);
        }) {
            effects.push(eff);
        }

        // `enabled=…` toggles editability — UITextView isn't a
        // UIControl so BoolAttr::Enabled doesn't apply; use the
        // dedicated setter.
        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) =
                install(enabled, move |b| el_for.set_text_view_editable(b))
            {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind {
            let eff =
                crate::ios::bind::install_text_view_value_bind(&el, bound);
            effects.push(eff);
        }

        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }

        effects.extend(apply_universal(&el, self.alpha));
        effects.extend(apply_text_attrs(
            &el,
            self.text_color,
            self.alignment,
            self.font_size,
        ));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }


        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// AddAnyAttr<Dom> for iOS leaf builders. Routes spread attrs (e.g.
// `<MyComponent on:click=...>`) onto the existing `pending_spreads`
// post-build hook, drained in `Render::build` after the underlying
// UIView is constructed. For OnAttribute, `apply_to` calls
// `el.on_click(cb)`, which on iOS routes via the UIControl
// target-action mechanism (silently no-ops on non-UIControl views).
// ---------------------------------------------------------------------

macro_rules! impl_add_any_attr_for_leaf {
    ($($builder:ident),+ $(,)?) => {
        $(
            impl renderer::view::AddAnyAttr<crate::Dom> for $builder {
                fn add_any_attr<__A>(mut self, attr: __A) -> Self
                where
                    __A: renderer::view::ApplyAttr<crate::Dom>,
                {
                    self.pending_spreads.push(Box::new(move |el: &IosElement| {
                        attr.apply_to(el);
                    }));
                    self
                }
            }
        )+
    };
}

impl_add_any_attr_for_leaf!(
    Button, Label, TextField, Switch, Slider, Stepper,
    ImageView, SegmentedControl, DatePicker,
);

// ProgressIndicator + TextView don't carry a handlers/pending_spreads
// Vec, so attaching a spread here has no install path. Panic rather
// than silently drop. (UITextView and UIProgressView CAN take tap
// gesture recognizers in principle, but adding the storage is
// scope-creep for this commit.)
impl renderer::view::AddAnyAttr<crate::Dom> for ProgressIndicator {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where __A: renderer::view::ApplyAttr<crate::Dom> {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on ProgressIndicator — \
             UIProgressView doesn't carry handler/spread storage in \
             this fork. Attach the handler to a sibling control instead."
        )
    }
}

impl renderer::view::AddAnyAttr<crate::Dom> for TextView {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where __A: renderer::view::ApplyAttr<crate::Dom> {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on TextView — UITextView \
             doesn't carry handler/spread storage in this fork. \
             Attach the handler to a sibling control instead."
        )
    }
}

// View<Children> — the iOS analogue of cocoa's Stack. UIKit's
// `on_click` falls back to UITapGestureRecognizer for non-UIControl
// views, so View *does* have a real install path. Push to
// pending_spreads, drained in Render::build like the inline
// `.on(click, …)` path.
impl<Children> renderer::view::AddAnyAttr<crate::Dom> for View<Children> {
    fn add_any_attr<__A>(mut self, attr: __A) -> Self
    where
        __A: renderer::view::ApplyAttr<crate::Dom>,
    {
        self.pending_spreads.push(Box::new(move |el: &IosElement| {
            attr.apply_to(el);
        }));
        self
    }
}

// ScrollView lacks pending_spreads in its struct — same treatment as
// ProgressIndicator/TextView. UIScrollView could host a tap gesture
// recognizer in principle but isn't wired here.
impl<Children> renderer::view::AddAnyAttr<crate::Dom> for ScrollView<Children> {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where
        __A: renderer::view::ApplyAttr<crate::Dom>,
    {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on ScrollView — no spread \
             storage on ScrollView<Children>. Attach to inner content \
             or wait for gesture-recognizer support."
        )
    }
}
