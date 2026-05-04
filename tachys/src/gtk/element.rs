//! Element builder types: `view()`, `button()`, `label()`, etc.
//!
//! Each builder returns a struct that implements [`Render`] from
//! tachys' view core. Building emits a [`gtk_dom::Element`] (or
//! similar leaf), wires attributes (with reactive effects for
//! signal-driven values), recursively builds children, and mounts
//! them.

use super::attr::{install, IntoMaybeReactive, MaybeReactive};
use crate::view::{Mountable, Render};
use gtk_dom::gtk::prelude::*;
use gtk_dom::{Element as GtkElement, Text as GtkText};
use reactive_graph::effect::RenderEffect;

// ---------------------------------------------------------------------
// Generic State machinery
// ---------------------------------------------------------------------

pub struct ElementState<ChildState> {
    el: GtkElement,
    _effects: Vec<RenderEffect<()>>,
    children: ChildState,
}

impl<ChildState: Mountable> Mountable for ElementState<ChildState> {
    fn unmount(&mut self) {
        self.children.unmount();
        self.el.as_node().teardown();
    }

    fn mount(
        &mut self,
        parent: &GtkElement,
        marker: Option<&gtk_dom::Node>,
    ) {
        parent.insert_node(self.el.as_node(), marker);
        self.children.mount(&self.el, None);
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable) -> bool {
        false
    }

    fn elements(&self) -> Vec<GtkElement> {
        vec![self.el.clone()]
    }
}

// ---------------------------------------------------------------------
// view() — generic vertical box container
// ---------------------------------------------------------------------

pub struct View<Children> {
    flex_direction: Option<gtk_dom::gtk::Orientation>,
    padding: Option<f32>,
    gap: Option<f32>,
    flex_grow: Option<f32>,
    children: Children,
}

pub fn view() -> View<()> {
    View {
        flex_direction: None,
        padding: None,
        gap: None,
        flex_grow: None,
        children: (),
    }
}

impl<Ch> View<Ch> {
    pub fn flex_direction(self, _dir: FlexDirection) -> Self {
        // Orientation is set at create time via tag; this method
        // exists for cocoa API parity and does nothing on GTK —
        // use `<vstack>` or `<hstack>` instead.
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

    pub fn child<NewCh>(self, child: NewCh) -> View<(Ch, NewCh)> {
        View {
            flex_direction: self.flex_direction,
            padding: self.padding,
            gap: self.gap,
            flex_grow: self.flex_grow,
            children: (self.children, child),
        }
    }
}

impl<Ch> Render for View<Ch>
where
    Ch: Render,
{
    type State = ElementState<Ch::State>;

    fn build(self) -> Self::State {
        let el = GtkElement::create("view");
        let widget = el.widget();

        if let Some(box_) = widget.downcast_ref::<gtk_dom::gtk::Box>() {
            // If an explicit orientation tag was used during create (e.g.
            // hstack/vstack), the tag already set orientation. For plain
            // `<view>`, it defaulted to Vertical.
            if let Some(p) = self.padding {
                let px = p as i32;
                box_.set_margin_top(px);
                box_.set_margin_bottom(px);
                box_.set_margin_start(px);
                box_.set_margin_end(px);
            }
            if let Some(g) = self.gap {
                box_.set_spacing(g as i32);
            }
        }
        // flex_grow on GTK: map truthiness to hexpand/vexpand (binary,
        // not weighted). The builder API accepts f32 for cocoa parity.
        if let Some(g) = self.flex_grow {
            if g > 0.0 {
                widget.set_hexpand(true);
                widget.set_vexpand(true);
            }
        }

        let child_state = self.children.build();

        ElementState {
            el,
            _effects: Vec::new(),
            children: child_state,
        }
    }

    fn rebuild(self, _state: &mut Self::State) {}
}

/// Vertical stack — a container with default vertical orientation.
pub fn vstack() -> View<()> {
    View {
        flex_direction: None,
        padding: None,
        gap: None,
        flex_grow: None,
        children: (),
    }
}

/// Horizontal stack — a container with horizontal orientation.
pub fn hstack() -> View<()> {
    View {
        flex_direction: None,
        padding: None,
        gap: None,
        flex_grow: None,
        children: (),
    }
}

/// Alias for `vstack()`.
pub fn stack_view() -> View<()> {
    vstack()
}

// Re-export layout enums for cocoa API parity.
pub use gtk_dom::gtk::Orientation as FlexDirection;

// ---------------------------------------------------------------------
// button()
// ---------------------------------------------------------------------

pub struct Button {
    title: MaybeReactive<String>,
    enabled: Option<MaybeReactive<bool>>,
    handlers: Vec<crate::html::event::PendingHandler>,
    flex_grow: Option<f32>,
}

pub fn button() -> Button {
    Button {
        title: MaybeReactive::Static(String::new()),
        enabled: None,
        handlers: Vec::new(),
        flex_grow: None,
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
        self.handlers
            .push(crate::html::event::PendingHandler::Click(Box::new(
                move || cb(),
            )));
        self
    }

    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }

    pub fn child<V>(self, value: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.title(value)
    }

    pub fn add_any_attr(
        mut self,
        attr: crate::html::event::OnAttribute,
    ) -> Self {
        if let Some(h) = attr.take_pending() {
            self.handlers.push(h);
        }
        self
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        E: crate::html::event::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl crate::html::event::SupportsEvent<crate::html::event::ClickEvent>
    for Button
{
}

impl Render for Button {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create("button");
        let mut effects = Vec::new();

        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_attribute("title", &t);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for_enabled = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_enabled.set_bool_attribute("enabled", b);
            }) {
                effects.push(eff);
            }
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        if let Some(g) = self.flex_grow {
            if g > 0.0 {
                el.widget().set_hexpand(true);
                el.widget().set_vexpand(true);
            }
        }

        ElementState {
            el,
            _effects: effects,
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
    handlers: Vec<crate::html::event::PendingHandler>,
}

pub fn checkbox() -> Checkbox {
    Checkbox {
        title: MaybeReactive::Static(String::new()),
        checked: MaybeReactive::Static(false),
        pending_bind_checked: None,
        handlers: Vec::new(),
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
        E: crate::html::event::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn add_any_attr(
        mut self,
        attr: crate::html::event::OnAttribute,
    ) -> Self {
        if let Some(h) = attr.take_pending() {
            self.handlers.push(h);
        }
        self
    }
}

impl crate::html::event::SupportsEvent<crate::html::event::ClickEvent>
    for Checkbox
{
}

impl Render for Checkbox {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create("checkbox");
        let mut effects = Vec::new();

        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_attribute("title", &t);
        }) {
            effects.push(eff);
        }

        let el_for_checked = el.clone();
        if let Some(eff) = install(self.checked, move |b| {
            el_for_checked.set_bool_attribute("checked", b);
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

        ElementState {
            el,
            _effects: effects,
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
    handlers: Vec<crate::html::event::PendingHandler>,
    flex_grow: Option<f32>,
}

pub fn slider() -> Slider {
    Slider {
        value: MaybeReactive::Static(0.0),
        min_value: 0.0,
        max_value: 1.0,
        enabled: None,
        pending_bind: None,
        handlers: Vec::new(),
        flex_grow: None,
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

    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
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
        E: crate::html::event::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn add_any_attr(
        mut self,
        attr: crate::html::event::OnAttribute,
    ) -> Self {
        if let Some(h) = attr.take_pending() {
            self.handlers.push(h);
        }
        self
    }
}

impl Render for Slider {
    type State = ElementState<()>;

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
                el_for_enabled.set_bool_attribute("enabled", b);
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

        if let Some(g) = self.flex_grow {
            if g > 0.0 {
                el.widget().set_hexpand(true);
                el.widget().set_vexpand(true);
            }
        }

        ElementState {
            el,
            _effects: effects,
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
    handlers: Vec<crate::html::event::PendingHandler>,
    flex_grow: Option<f32>,
}

pub fn pop_up_button() -> PopUpButton {
    PopUpButton {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        enabled: None,
        pending_bind_selection: None,
        handlers: Vec::new(),
        flex_grow: None,
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

    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
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
        E: crate::html::event::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn add_any_attr(
        mut self,
        attr: crate::html::event::OnAttribute,
    ) -> Self {
        if let Some(h) = attr.take_pending() {
            self.handlers.push(h);
        }
        self
    }
}

impl Render for PopUpButton {
    type State = ElementState<()>;

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
                el_for_enabled.set_bool_attribute("enabled", b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind_selection {
            let eff =
                super::bind::install_popup_selection_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        if let Some(g) = self.flex_grow {
            if g > 0.0 {
                el.widget().set_hexpand(true);
                el.widget().set_vexpand(true);
            }
        }

        ElementState {
            el,
            _effects: effects,
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
}

pub struct LabelState {
    text: GtkText,
    _effects: Vec<RenderEffect<()>>,
}

impl Mountable for LabelState {
    fn unmount(&mut self) {
        self.text.as_node().teardown();
    }

    fn mount(
        &mut self,
        parent: &GtkElement,
        marker: Option<&gtk_dom::Node>,
    ) {
        parent.insert_node(self.text.as_node(), marker);
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable) -> bool {
        false
    }

    fn elements(&self) -> Vec<GtkElement> {
        Vec::new()
    }
}

impl Render for Label {
    type State = LabelState;

    fn build(self) -> Self::State {
        let text = GtkText::create("");
        let mut effects = Vec::new();

        let text_for_set = text.clone();
        if let Some(eff) = install(self.text, move |s| {
            text_for_set.set_text(&s);
        }) {
            effects.push(eff);
        }

        LabelState {
            text,
            _effects: effects,
        }
    }

    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// text_field() — editable entry
// ---------------------------------------------------------------------

pub struct TextField {
    value: MaybeReactive<String>,
    placeholder: Option<String>,
    enabled: Option<MaybeReactive<bool>>,
    secure: bool,
    pending_bind: Option<super::bind::BoundValue>,
    handlers: Vec<crate::html::event::PendingHandler>,
}

pub fn text_field() -> TextField {
    TextField {
        value: MaybeReactive::Static(String::new()),
        placeholder: None,
        enabled: None,
        secure: false,
        pending_bind: None,
        handlers: Vec::new(),
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

    pub fn placeholder(mut self, s: impl Into<String>) -> Self {
        self.placeholder = Some(s.into());
        self
    }

    pub fn enabled<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.enabled = Some(value.into_maybe_reactive());
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
        E: crate::html::event::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn add_any_attr(
        mut self,
        attr: crate::html::event::OnAttribute,
    ) -> Self {
        if let Some(h) = attr.take_pending() {
            self.handlers.push(h);
        }
        self
    }
}

impl Render for TextField {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let tag = if self.secure {
            "secure_text_field"
        } else {
            "text_field"
        };
        let el = GtkElement::create(tag);
        let mut effects = Vec::new();

        if let Some(p) = self.placeholder {
            el.set_attribute("placeholder", &p);
        }

        if let Some(enabled) = self.enabled {
            let el_for_enabled = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_enabled.set_bool_attribute("enabled", b);
            }) {
                effects.push(eff);
            }
        }

        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_attribute("value", &v);
        }) {
            effects.push(eff);
        }

        if let Some(bound) = self.pending_bind {
            let eff =
                super::bind::install_text_field_value_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }

    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// IntoView plumbing — RenderHtml + AddAnyAttr stubs.
// ---------------------------------------------------------------------

use super::render_html_stub::gtk_stub_view_impls;

gtk_stub_view_impls!(Button);
gtk_stub_view_impls!(Checkbox);
gtk_stub_view_impls!(Label);
gtk_stub_view_impls!(PopUpButton);
gtk_stub_view_impls!(Slider);
gtk_stub_view_impls!(TextField);

// View<Children> needs its own (generic) impls.
impl<Ch> crate::view::add_attr::AddAnyAttr for View<Ch>
where
    Ch: Render + Send + 'static + crate::view::RenderHtml,
{
    type Output<NewAttr: crate::html::attribute::Attribute> = View<Ch>;

    fn add_any_attr<NewAttr: crate::html::attribute::Attribute>(
        self,
        _attr: NewAttr,
    ) -> Self::Output<NewAttr> {
        self
    }
}

impl<Ch> crate::view::RenderHtml for View<Ch>
where
    Ch: Render + Send + 'static + crate::view::RenderHtml,
{
    type AsyncOutput = Self;
    type Owned = Self;

    const MIN_LENGTH: usize = 0;

    fn dry_resolve(&mut self) {}

    async fn resolve(self) -> Self::AsyncOutput {
        self
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
        self
    }
}
