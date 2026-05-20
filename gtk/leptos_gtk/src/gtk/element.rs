//! Element builder types: `view()`, `button()`, `label()`, etc.
//!
//! Mirrors `leptos_cocoa::cocoa::element` for the controls that
//! exist on both ports. AppKit-only knobs (NSDatePicker style,
//! NSSegmentStyle, NSColorWell, etc.) are absent here; users that
//! need them on GTK can extend the builders.

use super::attr::{install, IntoMaybeReactive, MaybeReactive};
use super::node_ref::NodeRef;
use crate::Dom;
use gtk_dom::{
    layout::{
        set_align_content, set_align_items, set_column_gap, set_flex_basis,
        set_flex_direction, set_flex_shrink, set_flex_wrap, set_gap,
        set_grid_auto_columns, set_grid_auto_flow, set_grid_auto_rows,
        set_grid_template_columns, set_grid_template_rows, set_justify_content,
        set_justify_items, set_row_gap, AlignContent, AlignItems, FlexDirection,
        FlexWrap, GridAutoFlow, GridTemplateComponent, JustifyContent,
        JustifyItems, TrackSizingFunction,
    },
    Node as GtkElement,
};
use reactive_graph::effect::RenderEffect;
use renderer::attrs::{
    LayoutAttrs, UniversalAttrs, WithLayout, WithUniversal,
};
use renderer::view::{Mountable, Render};

// `apply_layout` / `apply_universal` live in `renderer`. The
// `LayoutElement` / `UniversalElement` impls for `GtkElement` are in
// `gtk_dom::layout` (orphan rule).
use gtk_dom::layout::{apply_layout, apply_universal};

/// Apply the two "always there" attribute structs every builder
/// owns: `universal` then `layout`. Layout LAST because
/// `hidden=Display::None` lives in `LayoutAttrs` and the Taffy
/// display flip needs to happen after the visual chrome.
///
/// The GTK port has no `apply_text` or `apply_decoration` helpers
/// today — text styling goes through CSS providers (not wired) and
/// background-color / borders are likewise CSS-driven. Mirror of
/// cocoa's `apply_common` (3-arg variant for ports without those
/// extras).
fn apply_common(
    el: &GtkElement,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
) -> Vec<RenderEffect<()>> {
    let mut effects = apply_universal(el, universal);
    effects.extend(apply_layout(el, layout));
    effects
}

// ---------------------------------------------------------------------
// Generic State machinery
// ---------------------------------------------------------------------

/// State retained for an element instance between build and rebuild.
pub struct ElementState<ChildState> {
    pub el: GtkElement,
    pub(crate) _effects: Vec<RenderEffect<()>>,
    pub(crate) children: ChildState,
}

impl<ChildState: Mountable<Dom>> Mountable<Dom>
    for ElementState<ChildState>
{
    fn unmount(&mut self) {
        self.children.unmount();
        self.el.teardown();
    }

    fn mount(
        &mut self,
        parent: &GtkElement,
        marker: Option<&gtk_dom::Node>,
    ) {
        // Insert self.el under parent. If parent has a Taffy tree
        // handle, this also registers self.el (and recursively, on
        // the next mount, our children) into the tree.
        parent.insert_node(&self.el, marker);

        // If this element is a container, install our TaffyLayout
        // now that it's registered.
        let widget = self.el.widget();
        if gtk_dom::node::is_container_widget(&widget) {
            gtk_dom::node::install_taffy_layout_for_container(
                &widget,
                self.el.id(),
                /* is_root */ false,
            );
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

impl<ChildState> Drop for ElementState<ChildState> {
    /// Safety net for an `ElementState` dropped without `unmount`
    /// (orphaned before mount, or a panic mid-`build`): free our
    /// store entry so it doesn't leak. `teardown` (→ `renderer::remove`)
    /// is idempotent, so this is a no-op after a normal `unmount`.
    fn drop(&mut self) {
        self.el.teardown();
    }
}

// ---------------------------------------------------------------------
// stack() — Taffy flexbox container
// ---------------------------------------------------------------------

/// Install Stack-specific flex-item attrs that aren't covered by
/// [`LayoutAttrs`] / [`WithLayout`] (`flex_shrink`, `flex_basis`).
fn apply_flex_item_extras(
    el: &GtkElement,
    shrink: Option<MaybeReactive<f32>>,
    basis: Option<MaybeReactive<f32>>,
) -> Vec<RenderEffect<()>> {
    let mut out = Vec::new();
    if let Some(v) = shrink {
        let e = el.clone();
        if let Some(eff) = install(v, move |s| set_flex_shrink(&e, s))
        {
            out.push(eff);
        }
    }
    if let Some(v) = basis {
        let e = el.clone();
        if let Some(eff) = install(v, move |b| set_flex_basis(&e, b))
        {
            out.push(eff);
        }
    }
    out
}

pub struct Stack<Children> {
    direction: Option<MaybeReactive<FlexDirection>>,
    gap: Option<MaybeReactive<f32>>,
    justify_content: Option<MaybeReactive<JustifyContent>>,
    align: Option<MaybeReactive<AlignItems>>,
    align_content: Option<MaybeReactive<AlignContent>>,
    justify_items: Option<MaybeReactive<JustifyItems>>,
    wrap: Option<MaybeReactive<FlexWrap>>,
    shrink: Option<MaybeReactive<f32>>,
    basis: Option<MaybeReactive<f32>>,
    layout: LayoutAttrs,
    universal: UniversalAttrs,
    children: Children,
}

fn empty_stack() -> Stack<()> {
    Stack {
        direction: None,
        gap: None,
        justify_content: None,
        align: None,
        align_content: None,
        justify_items: None,
        wrap: None,
        shrink: None,
        basis: None,
        layout: LayoutAttrs::default(),
        universal: UniversalAttrs::default(),
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

    pub fn align_content<V>(mut self, a: V) -> Self
    where
        V: IntoMaybeReactive<AlignContent>,
    {
        self.align_content = Some(a.into_maybe_reactive());
        self
    }

    pub fn justify_items<V>(mut self, j: V) -> Self
    where
        V: IntoMaybeReactive<JustifyItems>,
    {
        self.justify_items = Some(j.into_maybe_reactive());
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

    pub fn child<NewCh>(self, child: NewCh) -> Stack<(Ch, NewCh)> {
        Stack {
            direction: self.direction,
            gap: self.gap,
            justify_content: self.justify_content,
            align: self.align,
            align_content: self.align_content,
            justify_items: self.justify_items,
            wrap: self.wrap,
            shrink: self.shrink,
            basis: self.basis,
            layout: self.layout,
            universal: self.universal,
            children: (self.children, child),
        }
    }
}

impl<Ch> WithLayout for Stack<Ch> {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}

impl<Ch> WithUniversal for Stack<Ch> {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl<Ch> Render<Dom> for Stack<Ch>
where
    Ch: Render<Dom>,
{
    type State = ElementState<Ch::State>;

    fn build(self) -> Self::State {
        let el = GtkElement::create_stack();
        let mut effects = Vec::new();

        let direction = self
            .direction
            .unwrap_or(MaybeReactive::Static(FlexDirection::Column));
        {
            let e = el.clone();
            if let Some(eff) = install(direction, move |d| {
                set_flex_direction(&e, d)
            }) {
                effects.push(eff);
            }
        }
        if let Some(v) = self.gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_gap(&e, g)) {
                effects.push(eff);
            }
        }
        if let Some(v) = self.justify_content {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |j| set_justify_content(&e, j))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_items(&e, a))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.wrap {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |w| set_flex_wrap(&e, w))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align_content {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_content(&e, a))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.justify_items {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |j| set_justify_items(&e, j))
            {
                effects.push(eff);
            }
        }
        effects.extend(apply_flex_item_extras(&el, self.shrink, self.basis));
        effects.extend(apply_common(&el, self.universal, self.layout));

        // Build children but DON'T mount them yet — same cascade
        // pattern as the cocoa port. Mounting is deferred until
        // ElementState::mount runs (when self.el has joined a tree).
        let child_state = self.children.build();

        ElementState {
            el,
            _effects: effects,
            children: child_state,
        }
    }
}

// ---------------------------------------------------------------------
// grid() — Taffy CSS-Grid container (2-D layout)
// ---------------------------------------------------------------------

/// CSS-Grid container. Mirrors the cocoa `Grid` shape exactly. The
/// underlying GTK widget is still a `gtk::Box` — Taffy assigns final
/// frames to each child, so the widget class doesn't matter.
pub struct Grid<Children> {
    columns:         Option<Vec<GridTemplateComponent>>,
    rows:            Option<Vec<GridTemplateComponent>>,
    auto_columns:    Option<Vec<TrackSizingFunction>>,
    auto_rows:       Option<Vec<TrackSizingFunction>>,
    auto_flow:       Option<MaybeReactive<GridAutoFlow>>,
    column_gap:      Option<MaybeReactive<f32>>,
    row_gap:         Option<MaybeReactive<f32>>,
    gap:             Option<MaybeReactive<f32>>,
    justify_items:   Option<MaybeReactive<JustifyItems>>,
    align_items:     Option<MaybeReactive<AlignItems>>,
    justify_content: Option<MaybeReactive<JustifyContent>>,
    align_content:   Option<MaybeReactive<AlignContent>>,
    layout:          LayoutAttrs,
    universal:       UniversalAttrs,
    children:        Children,
}

pub fn grid() -> Grid<()> {
    Grid {
        columns: None,
        rows: None,
        auto_columns: None,
        auto_rows: None,
        auto_flow: None,
        column_gap: None,
        row_gap: None,
        gap: None,
        justify_items: None,
        align_items: None,
        justify_content: None,
        align_content: None,
        layout: LayoutAttrs::default(),
        universal: UniversalAttrs::default(),
        children: (),
    }
}

impl<Ch> Grid<Ch> {
    pub fn columns(mut self, t: impl Into<Vec<GridTemplateComponent>>) -> Self {
        self.columns = Some(t.into());
        self
    }
    pub fn rows(mut self, t: impl Into<Vec<GridTemplateComponent>>) -> Self {
        self.rows = Some(t.into());
        self
    }
    pub fn auto_columns(
        mut self,
        t: impl Into<Vec<TrackSizingFunction>>,
    ) -> Self {
        self.auto_columns = Some(t.into());
        self
    }
    pub fn auto_rows(mut self, t: impl Into<Vec<TrackSizingFunction>>) -> Self {
        self.auto_rows = Some(t.into());
        self
    }
    pub fn auto_flow<V: IntoMaybeReactive<GridAutoFlow>>(
        mut self,
        v: V,
    ) -> Self {
        self.auto_flow = Some(v.into_maybe_reactive());
        self
    }
    pub fn gap<V: IntoMaybeReactive<f32>>(mut self, g: V) -> Self {
        self.gap = Some(g.into_maybe_reactive());
        self
    }
    pub fn column_gap<V: IntoMaybeReactive<f32>>(mut self, g: V) -> Self {
        self.column_gap = Some(g.into_maybe_reactive());
        self
    }
    pub fn row_gap<V: IntoMaybeReactive<f32>>(mut self, g: V) -> Self {
        self.row_gap = Some(g.into_maybe_reactive());
        self
    }
    pub fn justify_items<V: IntoMaybeReactive<JustifyItems>>(
        mut self,
        v: V,
    ) -> Self {
        self.justify_items = Some(v.into_maybe_reactive());
        self
    }
    pub fn align_items<V: IntoMaybeReactive<AlignItems>>(mut self, v: V) -> Self {
        self.align_items = Some(v.into_maybe_reactive());
        self
    }
    pub fn justify_content<V: IntoMaybeReactive<JustifyContent>>(
        mut self,
        v: V,
    ) -> Self {
        self.justify_content = Some(v.into_maybe_reactive());
        self
    }
    pub fn align_content<V: IntoMaybeReactive<AlignContent>>(
        mut self,
        v: V,
    ) -> Self {
        self.align_content = Some(v.into_maybe_reactive());
        self
    }
    pub fn child<NewCh>(self, child: NewCh) -> Grid<(Ch, NewCh)> {
        Grid {
            columns: self.columns,
            rows: self.rows,
            auto_columns: self.auto_columns,
            auto_rows: self.auto_rows,
            auto_flow: self.auto_flow,
            column_gap: self.column_gap,
            row_gap: self.row_gap,
            gap: self.gap,
            justify_items: self.justify_items,
            align_items: self.align_items,
            justify_content: self.justify_content,
            align_content: self.align_content,
            layout: self.layout,
            universal: self.universal,
            children: (self.children, child),
        }
    }
}

impl<Ch> WithLayout for Grid<Ch> {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}

impl<Ch> WithUniversal for Grid<Ch> {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl<Ch> Render<Dom> for Grid<Ch>
where
    Ch: Render<Dom>,
{
    type State = ElementState<Ch::State>;

    fn build(self) -> Self::State {
        let el = GtkElement::create_grid();
        let mut effects = Vec::new();

        if let Some(c) = self.columns {
            set_grid_template_columns(&el, c);
        }
        if let Some(r) = self.rows {
            set_grid_template_rows(&el, r);
        }
        if let Some(c) = self.auto_columns {
            set_grid_auto_columns(&el, c);
        }
        if let Some(r) = self.auto_rows {
            set_grid_auto_rows(&el, r);
        }
        if let Some(v) = self.auto_flow {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |f| set_grid_auto_flow(&e, f))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_gap(&e, g)) {
                effects.push(eff);
            }
        }
        if let Some(v) = self.column_gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_column_gap(&e, g))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.row_gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_row_gap(&e, g)) {
                effects.push(eff);
            }
        }
        if let Some(v) = self.justify_items {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |j| set_justify_items(&e, j))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align_items {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_items(&e, a))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.justify_content {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |j| set_justify_content(&e, j))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align_content {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_content(&e, a))
            {
                effects.push(eff);
            }
        }
        effects.extend(apply_common(&el, self.universal, self.layout));

        let child_state = self.children.build();

        ElementState {
            el,
            _effects: effects,
            children: child_state,
        }
    }
}

// ---------------------------------------------------------------------
// button()
// ---------------------------------------------------------------------

pub struct Button {
    title: MaybeReactive<String>,
    enabled: Option<MaybeReactive<bool>>,
    handlers: Vec<crate::event_gtk::PendingHandler>,
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
}

pub fn button() -> Button {
    Button {
        title: MaybeReactive::Static(String::new()),
        enabled: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
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

impl WithLayout for Button {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for Button {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl Render<Dom> for Button {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create_button().0;
        let mut effects = Vec::new();

        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_title(&t);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for_enabled = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_enabled.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        effects.extend(apply_common(&el, self.universal, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
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
    universal: UniversalAttrs,
    layout: LayoutAttrs,
}

/// Portable name for the boolean toggle. On GTK this is the
/// same widget as `<checkbox>` (gtk::CheckButton); on iOS it
/// maps to UISwitch.
pub fn toggle() -> Checkbox {
    checkbox()
}

pub fn checkbox() -> Checkbox {
    Checkbox {
        title: MaybeReactive::Static(String::new()),
        checked: MaybeReactive::Static(false),
        pending_bind_checked: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
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

impl WithLayout for Checkbox {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for Checkbox {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl Render<Dom> for Checkbox {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create_checkbox().0;
        let mut effects = Vec::new();

        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_title(&t);
        }) {
            effects.push(eff);
        }

        let el_for_checked = el.clone();
        if let Some(eff) = install(self.checked, move |b| {
            el_for_checked.set_checked(b);
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

        effects.extend(apply_common(&el, self.universal, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
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
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
}

pub fn slider() -> Slider {
    Slider {
        value: MaybeReactive::Static(0.0),
        min_value: 0.0,
        max_value: 1.0,
        enabled: None,
        pending_bind: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
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

impl WithLayout for Slider {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for Slider {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl crate::event_gtk::SupportsEvent<crate::event_gtk::ChangeEvent>
    for Slider
{
}

impl Render<Dom> for Slider {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create_slider().0;
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
                el_for_enabled.set_enabled(b);
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

        effects.extend(apply_common(&el, self.universal, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
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
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
}

pub fn pop_up_button() -> PopUpButton {
    PopUpButton {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        enabled: None,
        pending_bind_selection: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
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

impl WithLayout for PopUpButton {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for PopUpButton {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl crate::event_gtk::SupportsEvent<crate::event_gtk::ChangeEvent>
    for PopUpButton
{
}

impl Render<Dom> for PopUpButton {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create_pop_up_button().0;
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
                el_for_enabled.set_enabled(b);
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

        effects.extend(apply_common(&el, self.universal, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
}

// ---------------------------------------------------------------------
// label()
// ---------------------------------------------------------------------

pub struct Label {
    text: MaybeReactive<String>,
    handlers: Vec<crate::event_gtk::PendingHandler>,
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
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
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
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

impl WithLayout for Label {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for Label {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl Render<Dom> for Label {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = GtkElement::create_label().0;
        let mut effects = Vec::new();

        let el_for_text = el.clone();
        if let Some(eff) = install(self.text, move |s| {
            el_for_text.set_value(&s);
        }) {
            effects.push(eff);
        }

        for h in self.handlers {
            h.apply_to(&el);
        }

        effects.extend(apply_common(&el, self.universal, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
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
    node_ref: Option<NodeRef>,
    directives: Vec<Box<dyn FnOnce(&GtkElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
}

pub fn text_field() -> TextField {
    TextField {
        value: MaybeReactive::Static(String::new()),
        placeholder: None,
        enabled: None,
        secure: false,
        pending_bind: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
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
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
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
impl crate::event_gtk::SupportsEvent<crate::event_gtk::CommitEvent>
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

impl WithLayout for TextField {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for TextField {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl Render<Dom> for TextField {
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = if self.secure {
            GtkElement::create_secure_text_field().0
        } else {
            GtkElement::create_text_field().0
        };
        let mut effects = Vec::new();

        if let Some(p) = self.placeholder {
            let el_for_p = el.clone();
            if let Some(eff) = install(p, move |s| {
                el_for_p.set_placeholder(&s);
            }) {
                effects.push(eff);
            }
        }

        if let Some(enabled) = self.enabled {
            let el_for_enabled = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_enabled.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_value(&v);
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

        effects.extend(apply_common(&el, self.universal, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        super::directives::run_all(self.directives, &el);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
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

impl<Children> renderer::view::AddAnyAttr<crate::Dom> for Grid<Children> {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where
        __A: renderer::view::ApplyAttr<crate::Dom>,
    {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on Grid. Containers have no \
             signal target — click and other events have no install \
             path. Attach to a child button/label/text_field instead."
        )
    }
}

// ---------------------------------------------------------------------
// WithDecoration — portable warn-and-ignore for inline styling attrs.
// See `crate::gtk::decoration` for the trait. Each impl is empty; the
// trait provides default warning methods for every builder.
// ---------------------------------------------------------------------

impl<C> crate::gtk::decoration::WithDecoration for Stack<C> {}
impl<C> crate::gtk::decoration::WithDecoration for Grid<C> {}
impl crate::gtk::decoration::WithDecoration for Button {}
impl crate::gtk::decoration::WithDecoration for Checkbox {}
impl crate::gtk::decoration::WithDecoration for Slider {}
impl crate::gtk::decoration::WithDecoration for PopUpButton {}
impl crate::gtk::decoration::WithDecoration for Label {}
impl crate::gtk::decoration::WithDecoration for TextField {}
