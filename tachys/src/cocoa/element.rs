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
/// effects (so they survive as long as the element is mounted),
/// the dynamic-attribute state (from `add_any_attr` chains — the
/// macro's typed-attribute pipeline), and the children's State.
pub struct ElementState<AttrState, ChildState> {
    /// Pub for test inspection — consider using `Mountable::elements()`
    /// in production code paths instead.
    pub el: CocoaElement,
    /// Effects driving reactive attributes. Dropped on unmount;
    /// dropping unsubscribes from the reactive graph.
    pub(crate) _effects: Vec<RenderEffect<()>>,
    /// State for the dynamic attribute tuple installed via
    /// `add_any_attr`. `()` for the empty-tuple default.
    pub(crate) _attrs: AttrState,
    pub(crate) children: ChildState,
}

impl<AttrState, ChildState: Mountable> Mountable
    for ElementState<AttrState, ChildState>
{
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

pub struct View<Children, At = ()> {
    flex_direction: Option<FlexDirection>,
    padding: Option<f32>,
    gap: Option<f32>,
    flex_grow: Option<f32>,
    children: Children,
    attrs: At,
}

pub fn view() -> View<(), ()> {
    View {
        flex_direction: None,
        padding: None,
        gap: None,
        flex_grow: None,
        children: (),
        attrs: (),
    }
}

impl<Ch, At> View<Ch, At> {
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

    pub fn child<NewCh>(self, child: NewCh) -> View<(Ch, NewCh), At> {
        View {
            flex_direction: self.flex_direction,
            padding: self.padding,
            gap: self.gap,
            flex_grow: self.flex_grow,
            children: (self.children, child),
            attrs: self.attrs,
        }
    }
}

impl<Ch, At> Render for View<Ch, At>
where
    Ch: Render,
    At: crate::html::attribute::Attribute,
{
    type State = ElementState<At::State, Ch::State>;

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

        let attrs = self.attrs.build(&el);

        ElementState {
            el,
            _effects: Vec::new(),
            _attrs: attrs,
            children: child_state,
        }
    }

    fn rebuild(self, state: &mut Self::State) {
        self.attrs.rebuild(&mut state._attrs);
        // Stage 5 part 1: attribute changes on a View aren't expected
        // (they're set once at build time). When we add reactive
        // styles, this needs proper diffing.
    }
}

// `stack_view` is just a `view` whose default flex direction is column.
pub fn stack_view() -> View<(), ()> {
    vstack()
}

/// Vertical stack — a flipped container with `flex_direction: Column`.
/// SwiftUI-flavoured shorthand for the common case.
pub fn vstack() -> View<(), ()> {
    View {
        flex_direction: Some(FlexDirection::Column),
        padding: None,
        gap: None,
        flex_grow: None,
        children: (),
        attrs: (),
    }
}

/// Horizontal stack — a flipped container with `flex_direction: Row`.
pub fn hstack() -> View<(), ()> {
    View {
        flex_direction: Some(FlexDirection::Row),
        padding: None,
        gap: None,
        flex_grow: None,
        children: (),
        attrs: (),
    }
}

// ---------------------------------------------------------------------
// impl_typed_attrs_for! — macro-ify the AddAnyAttr + RenderHtml
// boilerplate shared by every <At = ()> builder. Invoke once per
// builder; cuts ~350 LOC of near-duplicate impls.
// ---------------------------------------------------------------------

/// Emit `impl AddAnyAttr + impl RenderHtml` for a cocoa element
/// builder that is generic over a single type parameter `<At = ()>`.
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
                // Destructure so we can move `attrs` through
                // `.resolve().await` without partially-moving
                // `self`. The other fields are preserved inside
                // the destructured bindings and then used to
                // reconstruct the struct.
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
// button()
// ---------------------------------------------------------------------

pub struct Button<At = ()> {
    title: MaybeReactive<String>,
    enabled: Option<MaybeReactive<bool>>,
    handlers: Vec<crate::html::event::PendingHandler>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    /// Type-level attribute tuple accumulated via `add_any_attr`
    /// (the macro's typed-attribute pipeline). Default `()` —
    /// extends to `(NewAttr,)`, `(NewAttr, AnotherAttr)`, … as
    /// `add_any_attr` is called.
    attrs: At,
}

pub fn button() -> Button<()> {
    Button {
        title: MaybeReactive::Static(String::new()),
        enabled: None,
        handlers: Vec::new(),
        flex_grow: None,
        node_ref: None,
        directives: Vec::new(),
        attrs: (),
    }
}

impl<At> Button<At> {
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

    /// `node_ref=…` from the macro. The ref gets filled with this
    /// builder's underlying `cocoa_dom::Element` after
    /// `Render::build` runs.
    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    /// `use:directive=param` from the macro. Stores the directive
    /// call; runs at `Render::build` time with the constructed
    /// `cocoa_dom::Element` and the supplied `param`.
    ///
    /// Directives are escape-hatches for imperative manipulation
    /// of the underlying NSView — exactly the upstream
    /// `IntoDirective` shape, with `cocoa_dom::Element` as the
    /// element type. See `examples/.../directives_macos` for
    /// usage.
    ///
    /// Inherent method (not a trait impl) — Rust resolves it
    /// before `DirectiveAttribute::directive`, sidestepping the
    /// fact that our `AddAnyAttr` stub drops attributes.
    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::html::directive::IntoDirective<T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
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

// Buttons fire on click (NSButton target/action). Generic over
// At because every type-level attribute extension still describes
// the same control kind.
impl<At> crate::html::event::SupportsEvent<crate::html::event::ClickEvent>
    for Button<At>
{
}

// AddAnyAttr — the typed-attribute pipeline. Each call extends
// `attrs` from `At` to `<At as NextAttribute>::Output<NewAttr>`.
// At Render::build time, `attrs.build(&el)` walks the resulting
// tuple and runs each attribute's `build(&el)` against the live
// NSView.
impl_typed_attrs_for!(Button, title, enabled, handlers,
    flex_grow, node_ref, directives);

impl<At> Render for Button<At>
where
    At: crate::html::attribute::Attribute,
{
    type State = ElementState<At::State, ()>;

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

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);

        // Run the typed-attribute pipeline. For the empty-tuple
        // default this is `().build(&el)` — a no-op.
        let attrs = self.attrs.build(&el);

        ElementState {
            el,
            _effects: effects,
            _attrs: attrs,
            children: (),
        }
    }

    fn rebuild(self, state: &mut Self::State) {
        // Reactive attrs already update themselves via their Effects.
        // The typed-attribute pipeline rebuilds against its
        // accumulated state.
        self.attrs.rebuild(&mut state._attrs);
    }
}

// ---------------------------------------------------------------------
// checkbox() — NSButton in switch style with bool state
// ---------------------------------------------------------------------

pub struct Checkbox<At = ()> {
    title: MaybeReactive<String>,
    /// Static-or-reactive `checked=...` value (one-way: signal →
    /// button state). For two-way binding use `bind:checked=signal`,
    /// which sets `pending_bind_checked`.
    checked: MaybeReactive<bool>,
    pending_bind_checked: Option<crate::cocoa::bind::BoundChecked>,
    handlers: Vec<crate::html::event::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    attrs: At,
}

pub fn checkbox() -> Checkbox<()> {
    Checkbox {
        title: MaybeReactive::Static(String::new()),
        checked: MaybeReactive::Static(false),
        pending_bind_checked: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        attrs: (),
    }
}

impl<At> Checkbox<At> {
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

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    /// `use:directive=param` — see Button::directive for full
    /// docs. Inherent method (Rust resolves before
    /// `DirectiveAttribute::directive`).
    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::html::directive::IntoDirective<T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

// A checkbox toggles on click.
impl<At> crate::html::event::SupportsEvent<crate::html::event::ClickEvent>
    for Checkbox<At>
{
}

impl_typed_attrs_for!(Checkbox, title, checked, pending_bind_checked,
    handlers, node_ref, directives);

impl<At> Render for Checkbox<At>
where
    At: crate::html::attribute::Attribute,
{
    type State = ElementState<At::State, ()>;

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

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);

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
// slider() — NSSlider with min/max + bind:value
// ---------------------------------------------------------------------

pub struct Slider<At = ()> {
    value: MaybeReactive<f64>,
    min_value: f64,
    max_value: f64,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::cocoa::bind::BoundFloat>,
    handlers: Vec<crate::html::event::PendingHandler>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
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
        directives: Vec::new(),
        attrs: (),
    }
}

impl<At> Slider<At> {
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

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    /// `use:directive=param` — see Button::directive for full
    /// docs. Inherent method (Rust resolves before
    /// `DirectiveAttribute::directive`).
    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::html::directive::IntoDirective<T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

impl_typed_attrs_for!(Slider, value, min_value, max_value, enabled,
    pending_bind, handlers, flex_grow, node_ref, directives);

impl<At> Render for Slider<At>
where
    At: crate::html::attribute::Attribute,
{
    type State = ElementState<At::State, ()>;

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

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);

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
// pop_up_button() — NSPopUpButton with items + bind:selection
// ---------------------------------------------------------------------

pub struct PopUpButton<At = ()> {
    items: Vec<String>,
    selection: MaybeReactive<usize>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_selection: Option<crate::cocoa::bind::BoundIndex>,
    handlers: Vec<crate::html::event::PendingHandler>,
    flex_grow: Option<f32>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    attrs: At,
}

pub fn pop_up_button() -> PopUpButton<()> {
    PopUpButton {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        enabled: None,
        pending_bind_selection: None,
        handlers: Vec::new(),
        flex_grow: None,
        node_ref: None,
        directives: Vec::new(),
        attrs: (),
    }
}

impl<At> PopUpButton<At> {
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

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    /// `use:directive=param` — see Button::directive for full
    /// docs. Inherent method (Rust resolves before
    /// `DirectiveAttribute::directive`).
    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::html::directive::IntoDirective<T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

impl_typed_attrs_for!(PopUpButton, items, selection, enabled,
    pending_bind_selection, handlers, flex_grow, node_ref, directives);

impl<At> Render for PopUpButton<At>
where
    At: crate::html::attribute::Attribute,
{
    type State = ElementState<At::State, ()>;

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

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);

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

pub struct TextField<At = ()> {
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
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
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
        node_ref: None,
        directives: Vec::new(),
        attrs: (),
    }
}

/// Password-masking variant of `text_field()`. Emits an
/// `NSSecureTextField`, which is a subclass of `NSTextField` — so all
/// the bind / event / placeholder plumbing works unchanged.
pub fn secure_text_field() -> TextField<()> {
    TextField {
        value: MaybeReactive::Static(String::new()),
        placeholder: None,
        enabled: None,
        secure: true,
        pending_bind: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        attrs: (),
    }
}

impl<At> TextField<At> {
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

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    /// `use:directive=param` — see Button::directive for full
    /// docs. Inherent method (Rust resolves before
    /// `DirectiveAttribute::directive`).
    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::html::directive::IntoDirective<T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

// Text fields fire on every keystroke (`input`) and on commit
// (`change` — return key / focus loss). Click is a deliberate
// non-event: clicking inside the field places the caret, no
// "click" semantic equivalent. Focus/blur are AppKit's begin/end
// editing notifications.
impl<At> crate::html::event::SupportsEvent<crate::html::event::InputEvent>
    for TextField<At>
{
}
impl<At> crate::html::event::SupportsEvent<crate::html::event::ChangeEvent>
    for TextField<At>
{
}
impl<At> crate::html::event::SupportsEvent<crate::html::event::FocusEvent>
    for TextField<At>
{
}
impl<At> crate::html::event::SupportsEvent<crate::html::event::BlurEvent>
    for TextField<At>
{
}

impl_typed_attrs_for!(TextField, value, placeholder, enabled, secure,
    pending_bind, handlers, node_ref, directives);

impl<At> Render for TextField<At>
where
    At: crate::html::attribute::Attribute,
{
    type State = ElementState<At::State, ()>;

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

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);

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

use super::render_html_stub::cocoa_stub_view_impls;

// Builders not yet refactored: combined AddAnyAttr+RenderHtml stub.
cocoa_stub_view_impls!(Label);

// View<Children, At> — refactored to the typed-attribute pipeline.
impl<Ch, At> crate::view::add_attr::AddAnyAttr for View<Ch, At>
where
    Ch: Render + Send + 'static + crate::view::RenderHtml,
    At: crate::html::attribute::Attribute,
{
    type Output<NewAttr: crate::html::attribute::Attribute> =
        View<Ch, <At as crate::html::attribute::NextAttribute>::Output<NewAttr>>;

    fn add_any_attr<NewAttr: crate::html::attribute::Attribute>(
        self,
        attr: NewAttr,
    ) -> Self::Output<NewAttr> {
        View {
            flex_direction: self.flex_direction,
            padding: self.padding,
            gap: self.gap,
            flex_grow: self.flex_grow,
            children: self.children,
            attrs: crate::html::attribute::NextAttribute::add_any_attr(
                self.attrs, attr,
            ),
        }
    }
}

impl<Ch, At> crate::view::RenderHtml for View<Ch, At>
where
    Ch: Render + Send + 'static + crate::view::RenderHtml,
    At: crate::html::attribute::Attribute,
{
    type AsyncOutput = View<Ch::AsyncOutput, At::AsyncOutput>;
    type Owned = View<Ch::Owned, At::CloneableOwned>;

    const MIN_LENGTH: usize = 0;

    fn dry_resolve(&mut self) {
        self.attrs.dry_resolve();
    }

    async fn resolve(self) -> Self::AsyncOutput {
        let (children_resolved, attrs_resolved) =
            futures::join!(self.children.resolve(), self.attrs.resolve());
        View {
            flex_direction: self.flex_direction,
            padding: self.padding,
            gap: self.gap,
            flex_grow: self.flex_grow,
            children: children_resolved,
            attrs: attrs_resolved,
        }
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
        View {
            flex_direction: self.flex_direction,
            padding: self.padding,
            gap: self.gap,
            flex_grow: self.flex_grow,
            children: self.children.into_owned(),
            attrs: self.attrs.into_cloneable_owned(),
        }
    }
}
