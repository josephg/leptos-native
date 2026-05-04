//! Element builder types: `view()`, `button()`, `label()`, etc.
//!
//! Each builder returns a struct that implements [`Render`] from
//! tachys' view core. Building emits a [`cocoa_dom::Element`] (or
//! similar leaf), wires attributes (with reactive effects for
//! signal-driven values), recursively builds children, and mounts
//! them.

use super::attr::{install, IntoMaybeReactive, MaybeReactive};
use crate::view::{Mountable, Render};
use cocoa_dom::{
    layout::{
        set_flex_direction, set_flex_grow, set_gap, set_padding,
        FlexDirection,
    },
    BoolAttr, Element as CocoaElement, StringAttr, Text as CocoaText,
};
use reactive_graph::effect::RenderEffect;

// ---------------------------------------------------------------------
// Generic State machinery
// ---------------------------------------------------------------------

/// State retained for an element instance between build and rebuild.
///
/// Holds the underlying `cocoa_dom::Element`, any active reactive
/// effects (so they survive as long as the element is mounted), and
/// the children's State.
pub struct ElementState<ChildState> {
    /// Pub for test inspection — consider using `Mountable::elements()`
    /// in production code paths instead.
    pub el: CocoaElement,
    /// Effects driving reactive attributes. Dropped on unmount;
    /// dropping unsubscribes from the reactive graph.
    pub(crate) _effects: Vec<RenderEffect<()>>,
    pub(crate) children: ChildState,
}

impl<ChildState: Mountable> Mountable for ElementState<ChildState> {
    fn unmount(&mut self) {
        // Recurse first so children drop their Taffy/handler entries
        // before we drop ours. Then teardown(self.el) removes our own
        // entry and detaches us from our superview.
        self.children.unmount();
        self.el.as_node().teardown();
    }

    fn mount(
        &mut self,
        parent: &CocoaElement,
        marker: Option<&cocoa_dom::Node>,
    ) {
        // Step 1: insert self.el under parent. If parent has a Taffy
        // tree handle (i.e. is descended from a Window's content_root),
        // this also registers self.el in that tree.
        parent.insert_node(self.el.as_node(), marker);
        // Step 2: cascade — mount children under self.el. This is what
        // propagates the tree to descendants. We deliberately don't
        // mount children during build (which would try to attach them
        // before self.el is in any tree). The tree-aware
        // `insert_node` here registers each child as it goes.
        self.children.mount(&self.el, None);
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable) -> bool {
        false
    }

    fn elements(&self) -> Vec<CocoaElement> {
        vec![self.el.clone()]
    }
}

// ---------------------------------------------------------------------
// view() — generic flipped container
// ---------------------------------------------------------------------

pub struct View<Children> {
    flex_direction: Option<FlexDirection>,
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

    /// Flexbox grow factor — 0 (default) means don't grow into extra
    /// space; 1+ means take a share of extra space along the parent's
    /// main axis.
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
        let el = CocoaElement::create("view");

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

        // Build children but DON'T mount them yet. Mounting is
        // deferred until ElementState::mount runs (when self.el has
        // joined a tree); the recursive mount cascade then registers
        // every descendant in the right Taffy tree.
        let child_state = self.children.build();

        ElementState {
            el,
            _effects: Vec::new(),
            children: child_state,
        }
    }

    fn rebuild(self, _state: &mut Self::State) {
        // Stage 5 part 1: attribute changes on a View aren't expected
        // (they're set once at build time). When we add reactive
        // styles, this needs proper diffing.
    }
}

// `stack_view` is just a `view` whose default flex direction is column.
pub fn stack_view() -> View<()> {
    vstack()
}

/// Vertical stack — a flipped container with `flex_direction: Column`.
/// SwiftUI-flavoured shorthand for the common case.
pub fn vstack() -> View<()> {
    View {
        flex_direction: Some(FlexDirection::Column),
        padding: None,
        gap: None,
        flex_grow: None,
        children: (),
    }
}

/// Horizontal stack — a flipped container with `flex_direction: Row`.
pub fn hstack() -> View<()> {
    View {
        flex_direction: Some(FlexDirection::Row),
        padding: None,
        gap: None,
        flex_grow: None,
        children: (),
    }
}

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

    /// `enabled=true|false|closure` — controls NSControl::isEnabled.
    /// Disabled buttons render greyed-out and ignore clicks.
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

    /// See [`View::flex_grow`].
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }

    /// Sets the button's text. Used by the `view!{}` macro for
    /// `<button>"X"</button>` syntax — the macro emits `.child(value)`
    /// for each child node, and on a button the meaningful "child" is
    /// its title. Calling repeatedly replaces (last-wins).
    pub fn child<V>(self, value: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.title(value)
    }

    /// Apply an attribute. Used by the `view!{}` macro for spread
    /// syntax (`<button {..attr}>`). The single supported attribute
    /// kind today is an event listener; other attribute types
    /// (class, style, prop) won't satisfy the type signature.
    pub fn add_any_attr(
        mut self,
        attr: crate::html::event::OnAttribute,
    ) -> Self {
        if let Some(h) = attr.take_pending() {
            self.handlers.push(h);
        }
        self
    }

    /// Method called by the `view!{}` macro for the standard
    /// `on:event=handler` syntax. Defers installation: the
    /// [`PendingHandler`](crate::html::event::PendingHandler) is
    /// pushed onto a Vec and applied during `Render::build` once
    /// the underlying NSView exists.
    ///
    /// The `Self: SupportsEvent<E>` bound rejects events the
    /// element doesn't accept — `<button on:input=...>` won't
    /// compile.
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::html::event::SupportsEvent<E>,
        E: crate::html::event::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

// Buttons fire on click (NSButton target/action).
impl crate::html::event::SupportsEvent<crate::html::event::ClickEvent>
    for Button
{
}

impl Render for Button {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("button");
        let mut effects = Vec::new();

        // Wire the title — install handles both static and reactive.
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

        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
        }

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }

    fn rebuild(self, _state: &mut Self::State) {
        // Reactive attrs already update themselves via their Effects.
        // For static-attr changes across rebuilds, we'd need to diff;
        // skipped in part 1.
    }
}

// ---------------------------------------------------------------------
// checkbox() — NSButton in switch style with bool state
// ---------------------------------------------------------------------

pub struct Checkbox {
    title: MaybeReactive<String>,
    /// Static-or-reactive `checked=...` value (one-way: signal →
    /// button state). For two-way binding use `bind:checked=signal`,
    /// which sets `pending_bind_checked`.
    checked: MaybeReactive<bool>,
    pending_bind_checked: Option<crate::cocoa::bind::BoundChecked>,
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

    /// Macro forwards `<checkbox>"label"</checkbox>` here.
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

    /// Internal: stash a `bind:checked=...` for installation in
    /// `Render::build`.
    pub(crate) fn set_pending_bind_checked(
        &mut self,
        bound: crate::cocoa::bind::BoundChecked,
    ) {
        self.pending_bind_checked = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::html::event::SupportsEvent<E>,
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

// A checkbox toggles on click.
impl crate::html::event::SupportsEvent<crate::html::event::ClickEvent>
    for Checkbox
{
}

impl Render for Checkbox {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("checkbox");
        let mut effects = Vec::new();

        // Title: drive via the standard install pipeline.
        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_string_attribute(StringAttr::Title, &t);
        }) {
            effects.push(eff);
        }

        // One-way `checked=...` — install fires the closure on every
        // Effect tick with a typed bool, routed through
        // `set_bool_attribute(BoolAttr::Checked, ...)`.
        let el_for_checked = el.clone();
        if let Some(eff) = install(self.checked, move |b| {
            el_for_checked.set_bool_attribute(BoolAttr::Checked, b);
        }) {
            effects.push(eff);
        }

        // bind:checked — wires both directions (signal → button.state
        // via Effect, click → signal via on_click reading button.state).
        if let Some(bound) = self.pending_bind_checked {
            let eff = crate::cocoa::bind::install_checkbox_checked_bind(
                &el, bound,
            );
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
// slider() — NSSlider with min/max + bind:value
// ---------------------------------------------------------------------

pub struct Slider {
    value: MaybeReactive<f64>,
    min_value: f64,
    max_value: f64,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::cocoa::bind::BoundFloat>,
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
        bound: crate::cocoa::bind::BoundFloat,
    ) {
        self.pending_bind = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::html::event::SupportsEvent<E>,
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
        let el = CocoaElement::create("slider");
        let mut effects = Vec::new();

        // min/max set FIRST so initial setDoubleValue clamps correctly.
        el.set_slider_min(self.min_value);
        el.set_slider_max(self.max_value);

        // One-way `.value(...)`.
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

        // bind:value=signal.
        if let Some(bound) = self.pending_bind {
            let eff = crate::cocoa::bind::install_slider_value_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
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
// pop_up_button() — NSPopUpButton with items + bind:selection
// ---------------------------------------------------------------------

pub struct PopUpButton {
    items: Vec<String>,
    selection: MaybeReactive<usize>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_selection: Option<crate::cocoa::bind::BoundIndex>,
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
    /// Sets the popup's item list. Accepts any iterable of
    /// string-ish things — `Vec<&str>`, `Vec<String>`, etc.
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
        bound: crate::cocoa::bind::BoundIndex,
    ) {
        self.pending_bind_selection = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::html::event::SupportsEvent<E>,
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
        let el = CocoaElement::create("pop_up_button");
        let mut effects = Vec::new();

        // Items first (selection is meaningless without items).
        el.set_popup_items(&self.items);

        // One-way `.selection(...)`.
        let el_for_sel = el.clone();
        if let Some(eff) = install(self.selection, move |i| {
            el_for_sel.set_popup_selection(i as isize);
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

        // bind:selection=signal — wires both directions.
        if let Some(bound) = self.pending_bind_selection {
            let eff = crate::cocoa::bind::install_popup_selection_bind(
                &el, bound,
            );
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        if let Some(g) = self.flex_grow {
            set_flex_grow(el.as_node(), g);
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
// label() — static or reactive text
// ---------------------------------------------------------------------

pub struct Label {
    text: MaybeReactive<String>,
}

impl Label {
    /// Internal: stash a `bind:value=...` (read-direction only) for
    /// installation in `Render::build`. Used by the `BindAttribute`
    /// impl in `crate::cocoa::bind`. Equivalent to `.text(closure)`.
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

    /// Sets the label's text. Used by the `view!{}` macro for
    /// `<label>"X"</label>` syntax. Calling repeatedly replaces
    /// (last-wins).
    pub fn child<V>(self, value: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.text(value)
    }
}

/// Label has its own State because it wraps a `Text` rather than an
/// `Element`, but the Mountable contract is the same shape.
pub struct LabelState {
    /// Pub for test inspection.
    pub text: CocoaText,
    _effects: Vec<RenderEffect<()>>,
}

impl Mountable for LabelState {
    fn unmount(&mut self) {
        self.text.as_node().teardown();
    }

    fn mount(
        &mut self,
        parent: &CocoaElement,
        marker: Option<&cocoa_dom::Node>,
    ) {
        parent.insert_node(self.text.as_node(), marker);
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable) -> bool {
        false
    }

    fn elements(&self) -> Vec<CocoaElement> {
        Vec::new()
    }
}

impl Render for Label {
    type State = LabelState;

    fn build(self) -> Self::State {
        let text = CocoaText::create("");
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
// text_field() — editable text. on:input is Stage 5+ work; for now
// this is just a placeholder builder that renders an editable field
// with optional initial value.
// ---------------------------------------------------------------------

pub struct TextField {
    value: MaybeReactive<String>,
    placeholder: Option<String>,
    enabled: Option<MaybeReactive<bool>>,
    /// If `true`, build an NSSecureTextField instead of NSTextField.
    /// Used by the `secure_text_field()` constructor; same builder
    /// otherwise (NSSecureTextField is a subclass).
    secure: bool,
    /// `bind:value=...` state, applied at build time by
    /// `install_text_field_value_bind`. Distinct from `.value(...)`
    /// (which is one-way: signal → field).
    pending_bind: Option<crate::cocoa::bind::BoundValue>,
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

/// Password-masking variant of `text_field()`. Emits an
/// `NSSecureTextField`, which is a subclass of `NSTextField` — so all
/// the bind / event / placeholder plumbing works unchanged.
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

    /// Static placeholder text shown when the field is empty. (No
    /// reactive variant yet — login forms etc. just use literals.)
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

    /// Internal: stash a `bind:value=...` for installation in
    /// `Render::build`. Used by the `BindAttribute` impl in
    /// `crate::cocoa::bind`.
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::cocoa::bind::BoundValue,
    ) {
        self.pending_bind = Some(bound);
    }

    /// `on:event=handler` from the macro. Stashed; installed during
    /// `Render::build`. Click handlers on a text field are silently
    /// dropped (the underlying cocoa_dom call no-ops).
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::html::event::SupportsEvent<E>,
        E: crate::html::event::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    /// Spread-attribute path (`<text_field {..attr}/>`).
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

// Text fields fire on every keystroke (`input`) and on commit
// (`change` — return key / focus loss). Click is a deliberate
// non-event: clicking inside the field places the caret, no
// "click" semantic equivalent.
impl crate::html::event::SupportsEvent<crate::html::event::InputEvent>
    for TextField
{
}
impl crate::html::event::SupportsEvent<crate::html::event::ChangeEvent>
    for TextField
{
}

impl Render for TextField {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let tag = if self.secure { "secure_text_field" } else { "text_field" };
        let el = CocoaElement::create(tag);
        let mut effects = Vec::new();

        if let Some(p) = self.placeholder {
            el.set_string_attribute(StringAttr::Placeholder, &p);
        }

        if let Some(enabled) = self.enabled {
            let el_for_enabled = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_enabled.set_bool_attribute(BoolAttr::Enabled, b);
            }) {
                effects.push(eff);
            }
        }

        // Install one-way `.value(...)` if used.
        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_string_attribute(StringAttr::Value, &v);
        }) {
            effects.push(eff);
        }

        // Install `bind:value=signal` if used. This wires both
        // directions: signal → field via Effect, field → signal via
        // a fan-out delegate observing controlTextDidChange.
        if let Some(bound) = self.pending_bind {
            let eff =
                crate::cocoa::bind::install_text_field_value_bind(&el, bound);
            effects.push(eff);
        }

        // Install user-supplied event handlers (on:input, on:change).
        // These coexist with bind:value because the underlying
        // delegate fans out across all installed callbacks.
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

// (Children are handled via tachys' tuple Render/Mountable impls; no
// extra trait needed.)

// ---------------------------------------------------------------------
// IntoView plumbing — RenderHtml + AddAnyAttr stubs.
//
// These exist for the type checker. SSR / hydration aren't real on
// native, so the impls are no-ops; the work happens in `Render::build`.
// ---------------------------------------------------------------------

use super::render_html_stub::cocoa_stub_view_impls;

cocoa_stub_view_impls!(Button);
cocoa_stub_view_impls!(Checkbox);
cocoa_stub_view_impls!(Label);
cocoa_stub_view_impls!(PopUpButton);
cocoa_stub_view_impls!(Slider);
cocoa_stub_view_impls!(TextField);

// View<Children> needs its own (generic) impls — the macro only takes
// concrete types.
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
        _extra_attrs: Vec<crate::html::attribute::any_attribute::AnyAttribute>,
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
