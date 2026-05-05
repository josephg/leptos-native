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
    html::event::{EventDescriptor, PendingHandler, SupportsEvent},
    view::{Mountable, Render, RenderHtml},
};
use ios_dom::{
    layout::{set_flex_direction, set_flex_grow, set_gap, set_padding, FlexDirection},
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

/// Generates `alpha(<reactive f64>)` builder method on `$builder<At>`.
/// (No `tool_tip` analogue on iOS — tooltips are a macOS hover concept.)
macro_rules! impl_universal_attrs {
    ($builder:ident) => {
        impl<At> $builder<At> {
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
        impl<At> $builder<At> {
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

/// Emit `impl AddAnyAttr + impl RenderHtml` for a builder generic
/// over a single type parameter `<At = ()>`. Cuts the boilerplate
/// shared by every builder. Direct port of cocoa's macro.
macro_rules! impl_typed_attrs_for {
    ($builder:ident, $( $field:ident ),+ $(,)?) => {
        #[allow(clippy::type_complexity)]
        impl<At> $crate::view::add_attr::AddAnyAttr for $builder<At>
        where
            At: $crate::html::attribute::Attribute,
        {
            type Output<NewAttr: $crate::html::attribute::Attribute> =
                $builder<<At as $crate::html::attribute::NextAttribute>::Output<NewAttr>>;

            fn add_any_attr<NewAttr: $crate::html::attribute::Attribute>(
                self,
                attr: NewAttr,
            ) -> Self::Output<NewAttr> {
                $builder {
                    $($field: self.$field,)+
                    attrs: $crate::html::attribute::NextAttribute::add_any_attr(
                        self.attrs, attr,
                    ),
                }
            }
        }

        impl<At> $crate::view::RenderHtml for $builder<At>
        where
            At: $crate::html::attribute::Attribute,
        {
            type AsyncOutput = $builder<At::AsyncOutput>;
            type Owned = $builder<At::CloneableOwned>;

            const MIN_LENGTH: usize = 0;

            fn dry_resolve(&mut self) {
                self.attrs.dry_resolve();
            }

            async fn resolve(self) -> Self::AsyncOutput {
                let $builder { $($field,)+ attrs } = self;
                let attrs = attrs.resolve().await;
                $builder { $($field,)+ attrs }
            }

            fn to_html_with_buf(
                self,
                _buf: &mut String,
                _position: &mut $crate::view::Position,
                _escape: bool,
                _mark_branches: bool,
                _extra_attrs: Vec<
                    $crate::html::attribute::any_attribute::AnyAttribute,
                >,
            ) {
            }

            fn hydrate<const FROM_SERVER: bool>(
                self,
                _cursor: &$crate::hydration::Cursor,
                _position: &$crate::view::PositionState,
            ) -> Self::State {
                <Self as $crate::view::Render>::build(self)
            }

            fn into_owned(self) -> Self::Owned {
                let $builder { $($field,)+ attrs } = self;
                let attrs = attrs.into_cloneable_owned();
                $builder { $($field,)+ attrs }
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
    pub(crate) _attrs: AttrState,
    pub(crate) children: ChildState,
}

impl<AttrState, ChildState: Mountable> Mountable
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

    fn insert_before_this(&self, _child: &mut dyn Mountable) -> bool {
        false
    }

    fn elements(&self) -> Vec<IosElement> {
        vec![self.el.clone()]
    }
}

// ---------------------------------------------------------------------
// view() — generic UIView container
// ---------------------------------------------------------------------

pub struct View<Children, At = ()> {
    flex_direction: Option<FlexDirection>,
    padding: Option<f32>,
    gap: Option<f32>,
    flex_grow: Option<f32>,
    alpha: Option<MaybeReactive<f64>>,
    handlers: Vec<PendingHandler>,
    children: Children,
    attrs: At,
}

pub fn view() -> View<(), ()> {
    View {
        flex_direction: None,
        padding: None,
        gap: None,
        flex_grow: None,
        alpha: None,
        handlers: Vec::new(),
        children: (),
        attrs: (),
    }
}

impl<Ch, A> View<Ch, A> {
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
    pub fn alpha<V: IntoMaybeReactive<f64>>(mut self, a: V) -> Self {
        self.alpha = Some(a.into_maybe_reactive());
        self
    }
    pub fn child<NewCh>(self, child: NewCh) -> View<(Ch, NewCh), A> {
        View {
            flex_direction: self.flex_direction,
            padding: self.padding,
            gap: self.gap,
            flex_grow: self.flex_grow,
            alpha: self.alpha,
            handlers: self.handlers,
            children: (self.children, child),
            attrs: self.attrs,
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

impl<Ch: Render, A: crate::html::attribute::Attribute> Render for View<Ch, A> {
    type State = ElementState<A::State, Ch::State>;
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
        effects.extend(apply_universal(&el, self.alpha));
        let child_state = self.children.build();
        let attrs = self.attrs.build(&el);
        for handler in self.handlers {
            handler.apply_to(&el);
        }
        ElementState {
            el,
            _effects: effects,
            _attrs: attrs,
            children: child_state,
        }
    }
    fn rebuild(self, state: &mut Self::State) {
        self.attrs.rebuild(&mut state._attrs);
    }
}

impl<Ch, A> crate::view::add_attr::AddAnyAttr for View<Ch, A>
where
    Ch: Render + Send + 'static + RenderHtml,
    A: crate::html::attribute::Attribute,
{
    type Output<NewAttr: crate::html::attribute::Attribute> =
        View<Ch, <A as crate::html::attribute::NextAttribute>::Output<NewAttr>>;
    fn add_any_attr<NewAttr: crate::html::attribute::Attribute>(
        self,
        attr: NewAttr,
    ) -> Self::Output<NewAttr> {
        View {
            flex_direction: self.flex_direction,
            padding: self.padding,
            gap: self.gap,
            flex_grow: self.flex_grow,
            alpha: self.alpha,
            handlers: self.handlers,
            children: self.children,
            attrs: crate::html::attribute::NextAttribute::add_any_attr(
                self.attrs, attr,
            ),
        }
    }
}

impl<Ch, A> RenderHtml for View<Ch, A>
where
    Ch: Render + Send + 'static + RenderHtml,
    A: crate::html::attribute::Attribute,
{
    type AsyncOutput = View<Ch::AsyncOutput, A::AsyncOutput>;
    type Owned = View<Ch::Owned, A::CloneableOwned>;
    const MIN_LENGTH: usize = 0;
    fn dry_resolve(&mut self) {
        self.attrs.dry_resolve();
    }
    async fn resolve(self) -> Self::AsyncOutput {
        let ch = self.children.resolve();
        let a = self.attrs.resolve();
        View {
            flex_direction: self.flex_direction,
            padding: self.padding,
            gap: self.gap,
            flex_grow: self.flex_grow,
            alpha: self.alpha,
            handlers: Vec::new(),
            children: ch.await,
            attrs: a.await,
        }
    }
    fn to_html_with_buf(
        self,
        _buf: &mut String,
        _position: &mut crate::view::Position,
        _escape: bool,
        _mark_branches: bool,
        _extra_attrs: Vec<
            crate::html::attribute::any_attribute::AnyAttribute,
        >,
    ) {
    }
    fn hydrate<const FROM_SERVER: bool>(
        self,
        _cursor: &crate::hydration::Cursor,
        _position: &crate::view::PositionState,
    ) -> Self::State {
        <Self as Render>::build(self)
    }
    fn into_owned(self) -> Self::Owned {
        View {
            flex_direction: self.flex_direction,
            padding: self.padding,
            gap: self.gap,
            flex_grow: self.flex_grow,
            alpha: self.alpha,
            handlers: self.handlers,
            children: self.children.into_owned(),
            attrs: self.attrs.into_cloneable_owned(),
        }
    }
}

pub fn vstack() -> View<(), ()> {
    view().flex_direction(FlexDirection::Column)
}
pub fn hstack() -> View<(), ()> {
    view().flex_direction(FlexDirection::Row)
}

// ---------------------------------------------------------------------
// button() — UIButton with title + on:click
// ---------------------------------------------------------------------

pub struct Button<At = ()> {
    title: MaybeReactive<String>,
    enabled: Option<MaybeReactive<bool>>,
    handlers: Vec<PendingHandler>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
    font_size: Option<MaybeReactive<f64>>,
    attrs: At,
}

pub fn button() -> Button<()> {
    Button {
        title: MaybeReactive::Static(String::new()),
        enabled: None,
        handlers: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
        font_size: None,
        attrs: (),
    }
}

impl<A> Button<A> {
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

impl<A> SupportsEvent<crate::html::event::ClickEvent> for Button<A> {}

impl_universal_attrs!(Button);

impl_typed_attrs_for!(
    Button, title, enabled, handlers, flex_grow, node_ref, alpha,
    font_size,
);

impl<At: crate::html::attribute::Attribute> Render for Button<At> {
    type State = ElementState<At::State, ()>;
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

        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        let attrs = self.attrs.build(&el);

        ElementState {
            el,
            _effects: effects,
            _attrs: attrs,
            children: (),
        }
    }
    fn rebuild(self, state: &mut Self::State) {
        self.attrs.rebuild(&mut state._attrs);
    }
}

// ---------------------------------------------------------------------
// label() — UILabel
// ---------------------------------------------------------------------

pub struct Label<At = ()> {
    text: MaybeReactive<String>,
    handlers: Vec<PendingHandler>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
    text_color: Option<MaybeReactive<ios_dom::Color>>,
    alignment: Option<MaybeReactive<ios_dom::NSTextAlignment>>,
    font_size: Option<MaybeReactive<f64>>,
    pending_bind_text:
        Option<Box<dyn Fn() -> String + Send + 'static>>,
    attrs: At,
}

pub fn label() -> Label<()> {
    Label {
        text: MaybeReactive::Static(String::new()),
        handlers: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
        text_color: None,
        alignment: None,
        font_size: None,
        pending_bind_text: None,
        attrs: (),
    }
}

impl<A> Label<A> {
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

impl<A> SupportsEvent<crate::html::event::ClickEvent> for Label<A> {}

impl_universal_attrs!(Label);
impl_text_attrs!(Label);

impl_typed_attrs_for!(
    Label, text, handlers, flex_grow, node_ref, alpha, text_color,
    alignment, font_size, pending_bind_text,
);

impl<At: crate::html::attribute::Attribute> Render for Label<At> {
    type State = ElementState<At::State, ()>;
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

        let attrs = self.attrs.build(&el);

        ElementState {
            el,
            _effects: effects,
            _attrs: attrs,
            children: (),
        }
    }
    fn rebuild(self, state: &mut Self::State) {
        self.attrs.rebuild(&mut state._attrs);
    }
}

// ---------------------------------------------------------------------
// text_field() / secure_text_field() — UITextField
// ---------------------------------------------------------------------

pub struct TextField<At = ()> {
    value: MaybeReactive<String>,
    placeholder: Option<String>,
    enabled: Option<MaybeReactive<bool>>,
    /// If `true`, build with `secureTextEntry = YES`. Used by the
    /// `secure_text_field()` constructor; same builder otherwise.
    secure: bool,
    pending_bind: Option<crate::ios::bind::BoundValue>,
    handlers: Vec<PendingHandler>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
    text_color: Option<MaybeReactive<ios_dom::Color>>,
    alignment: Option<MaybeReactive<ios_dom::NSTextAlignment>>,
    font_size: Option<MaybeReactive<f64>>,
    attrs: At,
}

pub fn text_field() -> TextField<()> {
    TextField {
        value: MaybeReactive::Static(String::new()),
        placeholder: None,
        enabled: None,
        secure: false,
        pending_bind: None,
        handlers: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
        text_color: None,
        alignment: None,
        font_size: None,
        attrs: (),
    }
}

/// Password-masking variant of [`text_field`]. Builds a UITextField
/// with `secureTextEntry = YES`.
pub fn secure_text_field() -> TextField<()> {
    TextField {
        secure: true,
        ..text_field()
    }
}

impl<A> TextField<A> {
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
impl<At> SupportsEvent<crate::html::event::InputEvent> for TextField<At> {}
impl<At> SupportsEvent<crate::html::event::ChangeEvent> for TextField<At> {}
impl<At> SupportsEvent<crate::html::event::FocusEvent> for TextField<At> {}
impl<At> SupportsEvent<crate::html::event::BlurEvent> for TextField<At> {}

impl_universal_attrs!(TextField);
impl_text_attrs!(TextField);

impl_typed_attrs_for!(
    TextField, value, placeholder, enabled, secure, pending_bind,
    handlers, flex_grow, node_ref, alpha, text_color, alignment,
    font_size,
);

impl<At: crate::html::attribute::Attribute> Render for TextField<At> {
    type State = ElementState<At::State, ()>;
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

        let attrs = self.attrs.build(&el);

        ElementState {
            el,
            _effects: effects,
            _attrs: attrs,
            children: (),
        }
    }
    fn rebuild(self, state: &mut Self::State) {
        self.attrs.rebuild(&mut state._attrs);
    }
}

// ---------------------------------------------------------------------
// switch_() — UISwitch
//
// Trailing underscore because `switch` is a Rust keyword. The
// `view!{}` macro maps `<switch>` → `switch_()`.
// ---------------------------------------------------------------------

pub struct Switch<At = ()> {
    checked: MaybeReactive<bool>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_checked: Option<crate::ios::bind::BoundChecked>,
    handlers: Vec<PendingHandler>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
    attrs: At,
}

pub fn switch_() -> Switch<()> {
    Switch {
        checked: MaybeReactive::Static(false),
        enabled: None,
        pending_bind_checked: None,
        handlers: Vec::new(),
        node_ref: None,
        alpha: None,
        attrs: (),
    }
}

impl<A> Switch<A> {
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

impl<At> SupportsEvent<crate::html::event::ClickEvent> for Switch<At> {}

impl_universal_attrs!(Switch);

impl_typed_attrs_for!(
    Switch, checked, enabled, pending_bind_checked, handlers,
    node_ref, alpha,
);

impl<At: crate::html::attribute::Attribute> Render for Switch<At> {
    type State = ElementState<At::State, ()>;
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

        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        let attrs = self.attrs.build(&el);

        ElementState {
            el,
            _effects: effects,
            _attrs: attrs,
            children: (),
        }
    }
    fn rebuild(self, state: &mut Self::State) {
        self.attrs.rebuild(&mut state._attrs);
    }
}

// ---------------------------------------------------------------------
// slider() — UISlider
// ---------------------------------------------------------------------

pub struct Slider<At = ()> {
    value: MaybeReactive<f64>,
    min_value: f64,
    max_value: f64,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::ios::bind::BoundFloat>,
    handlers: Vec<PendingHandler>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::ios::NodeRef>,
    alpha: Option<MaybeReactive<f64>>,
    attrs: At,
}

pub fn slider() -> Slider<()> {
    Slider {
        value: MaybeReactive::Static(0.0),
        min_value: 0.0,
        max_value: 1.0,
        enabled: None,
        pending_bind: None,
        handlers: Vec::new(),
        flex_grow: None,
        node_ref: None,
        alpha: None,
        attrs: (),
    }
}

impl<A> Slider<A> {
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

impl<At> SupportsEvent<crate::html::event::ClickEvent> for Slider<At> {}

impl_universal_attrs!(Slider);

impl_typed_attrs_for!(
    Slider, value, min_value, max_value, enabled, pending_bind,
    handlers, flex_grow, node_ref, alpha,
);

impl<At: crate::html::attribute::Attribute> Render for Slider<At> {
    type State = ElementState<At::State, ()>;
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

        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }

        effects.extend(apply_universal(&el, self.alpha));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        let attrs = self.attrs.build(&el);

        ElementState {
            el,
            _effects: effects,
            _attrs: attrs,
            children: (),
        }
    }
    fn rebuild(self, state: &mut Self::State) {
        self.attrs.rebuild(&mut state._attrs);
    }
}
