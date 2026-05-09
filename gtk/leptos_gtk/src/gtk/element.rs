//! Element builder types: `view()`, `button()`, `label()`, etc.
//!
//! Mirrors `leptos_cocoa::cocoa::element` for the controls that
//! exist on both ports. AppKit-only knobs (NSDatePicker style,
//! NSSegmentStyle, NSColorWell, etc.) are absent here; users that
//! need them on GTK can extend the builders.

use super::attr::{install, Dim, IntoMaybeReactive, MaybeReactive};
use super::node_ref::NodeRef;
use crate::Dom;
use gtk_dom::{
    layout::{
        schedule_relayout, set_align_items, set_flex_basis, set_flex_direction,
        set_flex_grow, set_flex_shrink, set_flex_wrap, set_gap,
        set_justify_content, set_padding, update_style, AlignItems,
        FlexDirection, FlexWrap, JustifyContent,
    },
    BoolAttr, Element as GtkElement, StringAttr,
};
use reactive_graph::effect::RenderEffect;
use renderer::view::{Mountable, Render};

/// Universal attrs (alpha, tool_tip) — Stack/Block helper.
fn apply_universal(
    _el: &GtkElement,
    _alpha: Option<MaybeReactive<f64>>,
    _tool_tip: Option<MaybeReactive<String>>,
) -> Vec<RenderEffect<()>> {
    // GTK's equivalent of NSView's alphaValue is `set_opacity`. We
    // currently only support static values; reactive opacity needs
    // its own install hook on gtk_dom::Element. Stub for now —
    // matches cocoa's surface so callers compile.
    Vec::new()
}

/// Inherent-method block for universal attrs (`alpha`, `tool_tip`).
macro_rules! impl_universal_attrs {
    ($builder:ident) => {
        impl $builder {
            pub fn alpha<V>(mut self, a: V) -> Self
            where
                V: IntoMaybeReactive<f64>,
            {
                self.alpha = Some(a.into_maybe_reactive());
                self
            }
            pub fn tool_tip<V>(mut self, s: V) -> Self
            where
                V: IntoMaybeReactive<String>,
            {
                self.tool_tip = Some(s.into_maybe_reactive());
                self
            }
        }
    };
}

// ---------------------------------------------------------------------
// Generic State machinery
// ---------------------------------------------------------------------

/// State retained for an element instance between build and rebuild.
pub struct ElementState<AttrState, ChildState> {
    pub el: GtkElement,
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
        parent: &GtkElement,
        marker: Option<&gtk_dom::Node>,
    ) {
        // Insert self.el under parent. If parent has a Taffy tree
        // handle, this also registers self.el (and recursively, on
        // the next mount, our children) into the tree.
        parent.insert_node(self.el.as_node(), marker);

        // If this element is a container, install our TaffyLayout
        // now that it's registered.
        let node = self.el.as_node();
        let handle = node.layout_slot().borrow().handle.clone();
        if let Some(h) = handle {
            if gtk_dom::node::is_container_widget(self.el.widget()) {
                gtk_dom::node::install_taffy_layout_for_container(
                    self.el.widget(),
                    &h.tree,
                    h.node_id,
                    /* is_root */ false,
                );
            }
        }

        // Cascade — mount children under self.el.
        self.children.mount(&self.el, None);
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<GtkElement> {
        vec![self.el.clone()]
    }
}

// ---------------------------------------------------------------------
// stack() — Taffy flexbox container
// ---------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn apply_flex_item_attrs(
    el: &GtkElement,
    grow: Option<MaybeReactive<f32>>,
    shrink: Option<MaybeReactive<f32>>,
    basis: Option<MaybeReactive<f32>>,
    width: Option<MaybeReactive<Dim>>,
    min_width: Option<MaybeReactive<Dim>>,
    max_width: Option<MaybeReactive<Dim>>,
    height: Option<MaybeReactive<Dim>>,
    min_height: Option<MaybeReactive<Dim>>,
    max_height: Option<MaybeReactive<Dim>>,
) -> Vec<RenderEffect<()>> {
    let mut out = Vec::new();
    if let Some(v) = grow {
        let e = el.clone();
        if let Some(eff) = install(v, move |g| set_flex_grow(e.as_node(), g)) {
            out.push(eff);
        }
    }
    if let Some(v) = shrink {
        let e = el.clone();
        if let Some(eff) = install(v, move |s| set_flex_shrink(e.as_node(), s))
        {
            out.push(eff);
        }
    }
    if let Some(v) = basis {
        let e = el.clone();
        if let Some(eff) = install(v, move |b| set_flex_basis(e.as_node(), b))
        {
            out.push(eff);
        }
    }
    if let Some(v) = width {
        let e = el.clone();
        if let Some(eff) = install(v, move |d: Dim| {
            let n = e.as_node();
            update_style(n, |s| s.size.width = d.to_dimension());
            schedule_relayout(n);
        }) {
            out.push(eff);
        }
    }
    if let Some(v) = min_width {
        let e = el.clone();
        if let Some(eff) = install(v, move |d: Dim| {
            let n = e.as_node();
            update_style(n, |s| s.min_size.width = d.to_dimension());
            schedule_relayout(n);
        }) {
            out.push(eff);
        }
    }
    if let Some(v) = max_width {
        let e = el.clone();
        if let Some(eff) = install(v, move |d: Dim| {
            let n = e.as_node();
            update_style(n, |s| s.max_size.width = d.to_dimension());
            schedule_relayout(n);
        }) {
            out.push(eff);
        }
    }
    if let Some(v) = height {
        let e = el.clone();
        if let Some(eff) = install(v, move |d: Dim| {
            let n = e.as_node();
            update_style(n, |s| s.size.height = d.to_dimension());
            schedule_relayout(n);
        }) {
            out.push(eff);
        }
    }
    if let Some(v) = min_height {
        let e = el.clone();
        if let Some(eff) = install(v, move |d: Dim| {
            let n = e.as_node();
            update_style(n, |s| s.min_size.height = d.to_dimension());
            schedule_relayout(n);
        }) {
            out.push(eff);
        }
    }
    if let Some(v) = max_height {
        let e = el.clone();
        if let Some(eff) = install(v, move |d: Dim| {
            let n = e.as_node();
            update_style(n, |s| s.max_size.height = d.to_dimension());
            schedule_relayout(n);
        }) {
            out.push(eff);
        }
    }
    out
}

pub struct Stack<Children> {
    direction: Option<MaybeReactive<FlexDirection>>,
    gap: Option<MaybeReactive<f32>>,
    padding: Option<MaybeReactive<f32>>,
    justify_content: Option<MaybeReactive<JustifyContent>>,
    align: Option<MaybeReactive<AlignItems>>,
    wrap: Option<MaybeReactive<FlexWrap>>,
    grow: Option<MaybeReactive<f32>>,
    shrink: Option<MaybeReactive<f32>>,
    basis: Option<MaybeReactive<f32>>,
    width: Option<MaybeReactive<Dim>>,
    min_width: Option<MaybeReactive<Dim>>,
    max_width: Option<MaybeReactive<Dim>>,
    height: Option<MaybeReactive<Dim>>,
    min_height: Option<MaybeReactive<Dim>>,
    max_height: Option<MaybeReactive<Dim>>,
    alpha: Option<MaybeReactive<f64>>,
    tool_tip: Option<MaybeReactive<String>>,
    children: Children,
}

fn empty_stack() -> Stack<()> {
    Stack {
        direction: None,
        gap: None,
        padding: None,
        justify_content: None,
        align: None,
        wrap: None,
        grow: None,
        shrink: None,
        basis: None,
        width: None,
        min_width: None,
        max_width: None,
        height: None,
        min_height: None,
        max_height: None,
        alpha: None,
        tool_tip: None,
        children: (),
    }
}

pub fn stack() -> Stack<()> {
    empty_stack()
}

pub fn vstack() -> Stack<()> {
    Stack {
        direction: Some(MaybeReactive::Static(FlexDirection::Column)),
        ..empty_stack()
    }
}

pub fn hstack() -> Stack<()> {
    Stack {
        direction: Some(MaybeReactive::Static(FlexDirection::Row)),
        ..empty_stack()
    }
}

pub fn stack_view() -> Stack<()> {
    vstack()
}

pub fn view() -> Stack<()> {
    empty_stack()
}

impl<Ch> Stack<Ch> {
    pub fn direction<V>(mut self, d: V) -> Self
    where
        V: IntoMaybeReactive<FlexDirection>,
    {
        self.direction = Some(d.into_maybe_reactive());
        self
    }

    pub fn padding<V>(mut self, p: V) -> Self
    where
        V: IntoMaybeReactive<f32>,
    {
        self.padding = Some(p.into_maybe_reactive());
        self
    }

    pub fn gap<V>(mut self, g: V) -> Self
    where
        V: IntoMaybeReactive<f32>,
    {
        self.gap = Some(g.into_maybe_reactive());
        self
    }

    pub fn justify_content<V>(mut self, j: V) -> Self
    where
        V: IntoMaybeReactive<JustifyContent>,
    {
        self.justify_content = Some(j.into_maybe_reactive());
        self
    }

    pub fn align<V>(mut self, a: V) -> Self
    where
        V: IntoMaybeReactive<AlignItems>,
    {
        self.align = Some(a.into_maybe_reactive());
        self
    }

    pub fn wrap<V>(mut self, w: V) -> Self
    where
        V: IntoMaybeReactive<FlexWrap>,
    {
        self.wrap = Some(w.into_maybe_reactive());
        self
    }

    pub fn grow<V>(mut self, g: V) -> Self
    where
        V: IntoMaybeReactive<f32>,
    {
        self.grow = Some(g.into_maybe_reactive());
        self
    }

    pub fn shrink<V>(mut self, s: V) -> Self
    where
        V: IntoMaybeReactive<f32>,
    {
        self.shrink = Some(s.into_maybe_reactive());
        self
    }

    pub fn basis<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<f32>,
    {
        self.basis = Some(b.into_maybe_reactive());
        self
    }

    pub fn width<V>(mut self, w: V) -> Self
    where
        V: IntoMaybeReactive<Dim>,
    {
        self.width = Some(w.into_maybe_reactive());
        self
    }

    pub fn min_width<V>(mut self, w: V) -> Self
    where
        V: IntoMaybeReactive<Dim>,
    {
        self.min_width = Some(w.into_maybe_reactive());
        self
    }

    pub fn max_width<V>(mut self, w: V) -> Self
    where
        V: IntoMaybeReactive<Dim>,
    {
        self.max_width = Some(w.into_maybe_reactive());
        self
    }

    pub fn height<V>(mut self, h: V) -> Self
    where
        V: IntoMaybeReactive<Dim>,
    {
        self.height = Some(h.into_maybe_reactive());
        self
    }

    pub fn min_height<V>(mut self, h: V) -> Self
    where
        V: IntoMaybeReactive<Dim>,
    {
        self.min_height = Some(h.into_maybe_reactive());
        self
    }

    pub fn max_height<V>(mut self, h: V) -> Self
    where
        V: IntoMaybeReactive<Dim>,
    {
        self.max_height = Some(h.into_maybe_reactive());
        self
    }

    pub fn alpha<V>(mut self, a: V) -> Self
    where
        V: IntoMaybeReactive<f64>,
    {
        self.alpha = Some(a.into_maybe_reactive());
        self
    }

    pub fn tool_tip<V>(mut self, s: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.tool_tip = Some(s.into_maybe_reactive());
        self
    }

    pub fn child<NewCh>(self, child: NewCh) -> Stack<(Ch, NewCh)> {
        Stack {
            direction: self.direction,
            gap: self.gap,
            padding: self.padding,
            justify_content: self.justify_content,
            align: self.align,
            wrap: self.wrap,
            grow: self.grow,
            shrink: self.shrink,
            basis: self.basis,
            width: self.width,
            min_width: self.min_width,
            max_width: self.max_width,
            height: self.height,
            min_height: self.min_height,
            max_height: self.max_height,
            alpha: self.alpha,
            tool_tip: self.tool_tip,
            children: (self.children, child),
        }
    }
}

impl<Ch> Render<Dom> for Stack<Ch>
where
    Ch: Render<Dom>,
{
    type State = ElementState<(), Ch::State>;

    fn build(self) -> Self::State {
        let el = GtkElement::create("stack");
        let mut effects = Vec::new();

        let direction = self
            .direction
            .unwrap_or(MaybeReactive::Static(FlexDirection::Column));
        {
            let e = el.clone();
            if let Some(eff) = install(direction, move |d| {
                set_flex_direction(e.as_node(), d)
            }) {
                effects.push(eff);
            }
        }
        if let Some(v) = self.padding {
            let e = el.clone();
            if let Some(eff) = install(v, move |p| set_padding(e.as_node(), p))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_gap(e.as_node(), g)) {
                effects.push(eff);
            }
        }
        if let Some(v) = self.justify_content {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |j| set_justify_content(e.as_node(), j))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_items(e.as_node(), a))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.wrap {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |w| set_flex_wrap(e.as_node(), w))
            {
                effects.push(eff);
            }
        }
        effects.extend(apply_flex_item_attrs(
            &el,
            self.grow,
            self.shrink,
            self.basis,
            self.width,
            self.min_width,
            self.max_width,
            self.height,
            self.min_height,
            self.max_height,
        ));
        effects.extend(apply_universal(&el, self.alpha, self.tool_tip));

        // Build children but DON'T mount them yet — same cascade
        // pattern as the cocoa port. Mounting is deferred until
        // ElementState::mount runs (when self.el has joined a tree).
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
// button()
// ---------------------------------------------------------------------

pub struct Button {
    title: MaybeReactive<String>,
    enabled: Option<MaybeReactive<bool>>,
    handlers: Vec<crate::event_gtk::PendingHandler>,
    grow: Option<f32>,
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    alpha: Option<MaybeReactive<f64>>,
    tool_tip: Option<MaybeReactive<String>>,
}

pub fn button() -> Button {
    Button {
        title: MaybeReactive::Static(String::new()),
        enabled: None,
        handlers: Vec::new(),
        grow: None,
        node_ref: None,
        directives: Vec::new(),
        alpha: None,
        tool_tip: None,
    }
}

impl Button {
    pub fn title<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.title = value.into_maybe_reactive();
        self
    }

    pub fn enabled<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.enabled = Some(value.into_maybe_reactive());
        self
    }

    pub fn on_click(mut self, mut cb: impl FnMut() + Send + 'static) -> Self {
        self.handlers.push(
            crate::event_gtk::PendingHandler::Click(Box::new(move || cb())),
        );
        self
    }

    pub fn node_ref(mut self, r: NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<GtkElement, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(super::directives::pack(handler, param));
        self
    }

    pub fn grow(mut self, g: f32) -> Self {
        self.grow = Some(g);
        self
    }

    pub fn child<V>(self, value: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.title(value)
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_gtk::SupportsEvent<E>,
        E: crate::event_gtk::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl crate::event_gtk::SupportsEvent<crate::event_gtk::ClickEvent>
    for Button
{
}

impl_universal_attrs!(Button);

impl Render<Dom> for Button {
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create("button");
        let mut effects = Vec::new();

        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_string_attribute(StringAttr::Title, &t);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for_enabled = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_enabled.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        if let Some(g) = self.grow {
            set_flex_grow(el.as_node(), g);
        }

        effects.extend(apply_universal(&el, self.alpha, self.tool_tip));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

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
// checkbox()
// ---------------------------------------------------------------------

pub struct Checkbox {
    title: MaybeReactive<String>,
    checked: MaybeReactive<bool>,
    pending_bind_checked: Option<super::bind::BoundChecked>,
    handlers: Vec<crate::event_gtk::PendingHandler>,
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    alpha: Option<MaybeReactive<f64>>,
    tool_tip: Option<MaybeReactive<String>>,
}

pub fn checkbox() -> Checkbox {
    Checkbox {
        title: MaybeReactive::Static(String::new()),
        checked: MaybeReactive::Static(false),
        pending_bind_checked: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        alpha: None,
        tool_tip: None,
    }
}

impl Checkbox {
    pub fn title<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.title = value.into_maybe_reactive();
        self
    }

    pub fn child<V>(self, value: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.title(value)
    }

    pub fn checked<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.checked = value.into_maybe_reactive();
        self
    }

    pub(crate) fn set_pending_bind_checked(
        &mut self,
        bound: super::bind::BoundChecked,
    ) {
        self.pending_bind_checked = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_gtk::SupportsEvent<E>,
        E: crate::event_gtk::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn node_ref(mut self, r: NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<GtkElement, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(super::directives::pack(handler, param));
        self
    }
}

impl crate::event_gtk::SupportsEvent<crate::event_gtk::ClickEvent>
    for Checkbox
{
}

impl_universal_attrs!(Checkbox);

impl Render<Dom> for Checkbox {
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create("checkbox");
        let mut effects = Vec::new();

        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_string_attribute(StringAttr::Title, &t);
        }) {
            effects.push(eff);
        }

        let el_for_checked = el.clone();
        if let Some(eff) = install(self.checked, move |b| {
            el_for_checked.set_bool_attribute(BoolAttr::Checked, b);
        }) {
            effects.push(eff);
        }

        if let Some(bound) = self.pending_bind_checked {
            let eff =
                super::bind::install_checkbox_checked_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        effects.extend(apply_universal(&el, self.alpha, self.tool_tip));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

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
// slider()
// ---------------------------------------------------------------------

pub struct Slider {
    value: MaybeReactive<f64>,
    min_value: f64,
    max_value: f64,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<super::bind::BoundFloat>,
    handlers: Vec<crate::event_gtk::PendingHandler>,
    grow: Option<f32>,
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    alpha: Option<MaybeReactive<f64>>,
    tool_tip: Option<MaybeReactive<String>>,
}

pub fn slider() -> Slider {
    Slider {
        value: MaybeReactive::Static(0.0),
        min_value: 0.0,
        max_value: 1.0,
        enabled: None,
        pending_bind: None,
        handlers: Vec::new(),
        grow: None,
        node_ref: None,
        directives: Vec::new(),
        alpha: None,
        tool_tip: None,
    }
}

impl Slider {
    pub fn value<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<f64>,
    {
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

    pub fn enabled<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.enabled = Some(value.into_maybe_reactive());
        self
    }

    pub fn grow(mut self, g: f32) -> Self {
        self.grow = Some(g);
        self
    }

    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: super::bind::BoundFloat,
    ) {
        self.pending_bind = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_gtk::SupportsEvent<E>,
        E: crate::event_gtk::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn node_ref(mut self, r: NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<GtkElement, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(super::directives::pack(handler, param));
        self
    }
}

impl_universal_attrs!(Slider);

impl Render<Dom> for Slider {
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create("slider");
        let mut effects = Vec::new();

        el.set_slider_min(self.min_value);
        el.set_slider_max(self.max_value);

        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_double_value(v);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for_enabled = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_enabled.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind {
            let eff = super::bind::install_slider_value_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        if let Some(g) = self.grow {
            set_flex_grow(el.as_node(), g);
        }

        effects.extend(apply_universal(&el, self.alpha, self.tool_tip));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

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
// pop_up_button()
// ---------------------------------------------------------------------

pub struct PopUpButton {
    items: Vec<String>,
    selection: MaybeReactive<usize>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_selection: Option<super::bind::BoundIndex>,
    handlers: Vec<crate::event_gtk::PendingHandler>,
    grow: Option<f32>,
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    alpha: Option<MaybeReactive<f64>>,
    tool_tip: Option<MaybeReactive<String>>,
}

pub fn pop_up_button() -> PopUpButton {
    PopUpButton {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        enabled: None,
        pending_bind_selection: None,
        handlers: Vec::new(),
        grow: None,
        node_ref: None,
        directives: Vec::new(),
        alpha: None,
        tool_tip: None,
    }
}

impl PopUpButton {
    pub fn items<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.items = items.into_iter().map(Into::into).collect();
        self
    }

    pub fn selection<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<usize>,
    {
        self.selection = v.into_maybe_reactive();
        self
    }

    pub fn enabled<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.enabled = Some(value.into_maybe_reactive());
        self
    }

    pub fn grow(mut self, g: f32) -> Self {
        self.grow = Some(g);
        self
    }

    pub(crate) fn set_pending_bind_selection(
        &mut self,
        bound: super::bind::BoundIndex,
    ) {
        self.pending_bind_selection = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_gtk::SupportsEvent<E>,
        E: crate::event_gtk::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn node_ref(mut self, r: NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
}

impl_universal_attrs!(PopUpButton);

impl Render<Dom> for PopUpButton {
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create("pop_up_button");
        let mut effects = Vec::new();

        el.set_popup_items(&self.items);

        let el_for_sel = el.clone();
        if let Some(eff) = install(self.selection, move |i| {
            el_for_sel.set_popup_selection(i as u32);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for_enabled = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_enabled.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind_selection {
            let eff = super::bind::install_popup_selection_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        if let Some(g) = self.grow {
            set_flex_grow(el.as_node(), g);
        }

        effects.extend(apply_universal(&el, self.alpha, self.tool_tip));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

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
// label()
// ---------------------------------------------------------------------

pub struct Label {
    text: MaybeReactive<String>,
    handlers: Vec<crate::event_gtk::PendingHandler>,
    grow: Option<f32>,
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    alpha: Option<MaybeReactive<f64>>,
    tool_tip: Option<MaybeReactive<String>>,
}

impl Label {
    pub(crate) fn set_pending_bind_text(
        &mut self,
        getter: Box<dyn Fn() -> String + Send + 'static>,
    ) {
        self.text = MaybeReactive::Reactive(Box::new(move || getter()));
    }
}

pub fn label() -> Label {
    Label {
        text: MaybeReactive::Static(String::new()),
        handlers: Vec::new(),
        grow: None,
        node_ref: None,
        directives: Vec::new(),
        alpha: None,
        tool_tip: None,
    }
}

impl Label {
    pub fn text<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.text = value.into_maybe_reactive();
        self
    }

    pub fn child<V>(self, value: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.text(value)
    }

    pub fn grow(mut self, g: f32) -> Self {
        self.grow = Some(g);
        self
    }

    pub fn node_ref(mut self, r: NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_gtk::SupportsEvent<E>,
        E: crate::event_gtk::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl_universal_attrs!(Label);

impl Render<Dom> for Label {
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create("label");
        let mut effects = Vec::new();

        let el_for_text = el.clone();
        if let Some(eff) = install(self.text, move |s| {
            el_for_text.set_string_attribute(StringAttr::Value, &s);
        }) {
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        if let Some(g) = self.grow {
            set_flex_grow(el.as_node(), g);
        }

        effects.extend(apply_universal(&el, self.alpha, self.tool_tip));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

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
// text_field() / secure_text_field()
// ---------------------------------------------------------------------

pub struct TextField {
    value: MaybeReactive<String>,
    placeholder: Option<MaybeReactive<String>>,
    enabled: Option<MaybeReactive<bool>>,
    secure: bool,
    pending_bind: Option<super::bind::BoundValue>,
    handlers: Vec<crate::event_gtk::PendingHandler>,
    grow: Option<f32>,
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    alpha: Option<MaybeReactive<f64>>,
    tool_tip: Option<MaybeReactive<String>>,
}

pub fn text_field() -> TextField {
    TextField {
        value: MaybeReactive::Static(String::new()),
        placeholder: None,
        enabled: None,
        secure: false,
        pending_bind: None,
        handlers: Vec::new(),
        grow: None,
        node_ref: None,
        directives: Vec::new(),
        alpha: None,
        tool_tip: None,
    }
}

pub fn secure_text_field() -> TextField {
    TextField {
        value: MaybeReactive::Static(String::new()),
        placeholder: None,
        enabled: None,
        secure: true,
        pending_bind: None,
        handlers: Vec::new(),
        grow: None,
        node_ref: None,
        directives: Vec::new(),
        alpha: None,
        tool_tip: None,
    }
}

impl TextField {
    pub fn value<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.value = v.into_maybe_reactive();
        self
    }

    pub fn placeholder<V>(mut self, s: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.placeholder = Some(s.into_maybe_reactive());
        self
    }

    pub fn enabled<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.enabled = Some(value.into_maybe_reactive());
        self
    }

    pub fn grow(mut self, g: f32) -> Self {
        self.grow = Some(g);
        self
    }

    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: super::bind::BoundValue,
    ) {
        self.pending_bind = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_gtk::SupportsEvent<E>,
        E: crate::event_gtk::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn node_ref(mut self, r: NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }
}

// TextField fires the "input" / "change" / "focus" / "blur" events.
impl crate::event_gtk::SupportsEvent<crate::event_gtk::InputEvent>
    for TextField
{
}
impl crate::event_gtk::SupportsEvent<crate::event_gtk::ChangeEvent>
    for TextField
{
}
impl crate::event_gtk::SupportsEvent<crate::event_gtk::FocusEvent>
    for TextField
{
}
impl crate::event_gtk::SupportsEvent<crate::event_gtk::BlurEvent>
    for TextField
{
}

impl_universal_attrs!(TextField);

impl Render<Dom> for TextField {
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let tag = if self.secure {
            "secure_text_field"
        } else {
            "text_field"
        };
        let el = GtkElement::create(tag);
        let mut effects = Vec::new();

        if let Some(p) = self.placeholder {
            let el_for_p = el.clone();
            if let Some(eff) = install(p, move |s| {
                el_for_p.set_string_attribute(StringAttr::Placeholder, &s);
            }) {
                effects.push(eff);
            }
        }

        if let Some(enabled) = self.enabled {
            let el_for_enabled = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_enabled.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_string_attribute(StringAttr::Value, &v);
        }) {
            effects.push(eff);
        }

        if let Some(bound) = self.pending_bind {
            let eff = super::bind::install_text_field_value_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        if let Some(g) = self.grow {
            set_flex_grow(el.as_node(), g);
        }

        effects.extend(apply_universal(&el, self.alpha, self.tool_tip));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

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
// AddAnyAttr<Dom> for the leaf builders.
// ---------------------------------------------------------------------

macro_rules! impl_add_any_attr_for_leaf {
    ($($builder:ident),+ $(,)?) => {
        $(
            impl renderer::view::AddAnyAttr<crate::Dom> for $builder {
                fn add_any_attr<__A>(mut self, attr: __A) -> Self
                where
                    __A: renderer::view::ApplyAttr<crate::Dom>,
                {
                    self.directives.push(Box::new(move |el: &GtkElement| {
                        attr.apply_to(el);
                    }));
                    self
                }
            }
        )+
    };
}

impl_add_any_attr_for_leaf!(
    Button, Checkbox, Slider, PopUpButton, Label, TextField,
);

// Container builders panic on spread attrs — same as cocoa.
impl<Children> renderer::view::AddAnyAttr<crate::Dom> for Stack<Children> {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where
        __A: renderer::view::ApplyAttr<crate::Dom>,
    {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on Stack (vstack/hstack/\
             stack_view). Containers have no signal target — click \
             and other events have no install path. Attach to a \
             child button/label/text_field instead."
        )
    }
}
