//! Element builder types: `view()`, `button()`, `label()`, etc.
//!
//! Each builder returns a struct that implements [`Render`] from
//! tachys' view core. Building emits a [`cocoa_dom::Element`] (or
//! similar leaf), wires attributes (with reactive effects for
//! signal-driven values), recursively builds children, and mounts
//! them.

use super::attr::{install, IntoMaybeReactive, MaybeReactive};
use renderer::attrs::{
    LayoutAttrs, TextAttrs, UniversalAttrs, WithLayout, WithUniversal,
};
use renderer::view::{Mountable, Render};
use crate::Dom;
use cocoa_dom::{
    layout::{
        set_align_content, set_align_items, set_background_color, set_clip,
        set_column_gap, set_flex_basis, set_flex_direction, set_flex_shrink,
        set_flex_wrap, set_gap, set_grid_auto_columns, set_grid_auto_flow,
        set_grid_auto_rows, set_grid_template_columns, set_grid_template_rows,
        set_justify_content, set_justify_items, set_row_gap, AlignContent,
        AlignItems, FlexDirection, FlexWrap, GridAutoFlow,
        GridTemplateComponent, JustifyContent, JustifyItems, TrackSizingFunction,
    },
    BoolAttr, Color, Element as CocoaElement, StringAttr,
};
use reactive_graph::effect::RenderEffect;

/// Cocoa's text-attr struct alias — `TextAttrs` with cocoa's `Color`
/// and `NSTextAlignment`.
pub type CocoaText = TextAttrs<Color, cocoa_dom::TextAlignment>;

/// Port-local accessor trait for [`CocoaText`]. Mirrors the shape of
/// renderer-common's `WithLayout` / `WithUniversal`: each builder
/// implements `text_attrs_mut` returning `&mut self.text`; the
/// default methods supply the chainable setters.
///
/// Stays port-local rather than implementing renderer-common's
/// generic `WithText<C, A>` because the chainable setters need the
/// port-local [`IntoMaybeReactive`] (for AppKit-foreign types like
/// `NSTextAlignment` and `Color`). Renderer-common's `WithText` uses
/// its own renderer-common `IntoMaybeReactive`, which only has impls
/// for renderer-common-owned types.
pub trait WithText: Sized {
    fn text_attrs_mut(&mut self) -> &mut CocoaText;

    fn text_color<V: IntoMaybeReactive<Color>>(mut self, c: V) -> Self {
        self.text_attrs_mut().text_color = Some(c.into_maybe_reactive());
        self
    }
    /// Text alignment within the control's frame.
    fn alignment<V: IntoMaybeReactive<cocoa_dom::TextAlignment>>(
        mut self,
        a: V,
    ) -> Self {
        self.text_attrs_mut().alignment = Some(a.into_maybe_reactive());
        self
    }
    /// Font size in points (system font at this size).
    fn font_size<V: IntoMaybeReactive<f64>>(mut self, p: V) -> Self {
        self.text_attrs_mut().font_size = Some(p.into_maybe_reactive());
        self
    }
}

// `apply_universal` and `apply_layout` live in `renderer`. The
// `UniversalElement` / `LayoutElement` impls for `CocoaElement` live
// in `cocoa_dom` (orphan rule).
use renderer::apply_universal;

/// Apply [`CocoaText`] (text_color, alignment, font_size) to the live
/// NSView. Each leaf decides whether to invoke this — NSButton
/// skips `text_color` (uses `attributedTitle` if styling is needed).
fn apply_text(el: &CocoaElement, attrs: CocoaText) -> Vec<RenderEffect<()>> {
    let mut out = Vec::new();
    if let Some(c) = attrs.text_color {
        let el_for = el.clone();
        if let Some(eff) = install(c, move |v| el_for.set_text_color(v)) {
            out.push(eff);
        }
    }
    if let Some(a) = attrs.alignment {
        let el_for = el.clone();
        if let Some(eff) =
            install(a, move |v| el_for.set_text_alignment(v))
        {
            out.push(eff);
        }
    }
    if let Some(s) = attrs.font_size {
        let el_for = el.clone();
        if let Some(eff) = install(s, move |v| el_for.set_font_size(v)) {
            out.push(eff);
        }
    }
    out
}

use renderer::apply_layout;

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
    /// Phantom slot where upstream stored the state for the dynamic
    /// attribute tuple installed via `add_any_attr`. The fork dropped
    /// that machinery (no SSR, no spread); kept as a phantom so
    /// existing builder code that passes a unit `()` through the
    /// type still type-checks.
    pub(crate) _attrs: std::marker::PhantomData<AttrState>,
    pub(crate) children: ChildState,
}

impl<AttrState, ChildState: Mountable<Dom>> Mountable<Dom>
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

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<CocoaElement> {
        vec![self.el.clone()]
    }
}

// ---------------------------------------------------------------------
// stack() — Taffy flexbox container (canonical linear layout primitive)
// ---------------------------------------------------------------------

/// Apply the flex-item style attrs (`grow`, `shrink`, `basis`,
/// `width` / `min_width` / `max_width`, `height` / `min_height` /
/// `max_height`) — meaningful on any element that participates in a
/// flex parent's layout. Used by both `Stack` and `Block`.
#[allow(clippy::too_many_arguments)]
/// Install Stack/Block-specific flex-item attrs that aren't covered
/// by [`LayoutAttrs`] / [`WithLayout`] (`flex_shrink`, `flex_basis`).
fn apply_flex_item_extras(
    el: &CocoaElement,
    shrink: Option<MaybeReactive<f32>>,
    basis: Option<MaybeReactive<f32>>,
) -> Vec<RenderEffect<()>> {
    let mut out = Vec::new();
    if let Some(v) = shrink {
        let e = el.clone();
        if let Some(eff) = install(v, move |s| set_flex_shrink(e.as_node(), s)) {
            out.push(eff);
        }
    }
    if let Some(v) = basis {
        let e = el.clone();
        if let Some(eff) = install(v, move |b| set_flex_basis(e.as_node(), b)) {
            out.push(eff);
        }
    }
    out
}

pub struct Stack<Children> {
    direction:        Option<MaybeReactive<FlexDirection>>,
    gap:              Option<MaybeReactive<f32>>,
    justify_content:  Option<MaybeReactive<JustifyContent>>,
    align:            Option<MaybeReactive<AlignItems>>,
    align_content:    Option<MaybeReactive<AlignContent>>,
    justify_items:    Option<MaybeReactive<JustifyItems>>,
    wrap:             Option<MaybeReactive<FlexWrap>>,
    shrink:           Option<MaybeReactive<f32>>,
    basis:            Option<MaybeReactive<f32>>,
    background_color: Option<MaybeReactive<Color>>,
    clip:             Option<MaybeReactive<bool>>,
    layout:           LayoutAttrs,
    universal:        UniversalAttrs,
    children:         Children,
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
        background_color: None,
        clip: None,
        layout: LayoutAttrs::default(),
        universal: UniversalAttrs::default(),
        children: (),
    }
}

/// Linear layout container backed by Taffy flexbox. `direction`
/// defaults to `Column` if unset.
pub fn stack() -> Stack<()> {
    empty_stack()
}

/// Vertical stack — `direction = Column`.
pub fn vstack() -> Stack<()> {
    Stack {
        direction: Some(MaybeReactive::Static(FlexDirection::Column)),
        ..empty_stack()
    }
}

/// Horizontal stack — `direction = Row`.
pub fn hstack() -> Stack<()> {
    Stack {
        direction: Some(MaybeReactive::Static(FlexDirection::Row)),
        ..empty_stack()
    }
}

/// Legacy alias of `vstack()` — kept for source-compatibility.
pub fn stack_view() -> Stack<()> {
    vstack()
}

/// Generic flexbox container — direction defaults to Column. Same
/// as `stack()`; kept under the `<view>` tag name for parity with
/// the iOS / GTK ports' element vocabularies.
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

    /// Cross-axis content distribution when the children's total cross
    /// extent is less than the container's — same as CSS `align-content`.
    /// Only meaningful when `wrap` is enabled and lines exist on the
    /// cross axis.
    pub fn align_content<V>(mut self, a: V) -> Self
    where
        V: IntoMaybeReactive<AlignContent>,
    {
        self.align_content = Some(a.into_maybe_reactive());
        self
    }

    /// Default cross-axis alignment for items within their flex line
    /// — same as CSS `justify-items`. Overridable per-child via
    /// `align_self`.
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

    /// Solid background fill via CALayer. Switches the stack's
    /// underlying NSView to layer-backed.
    pub fn background_color<V>(mut self, c: V) -> Self
    where
        V: IntoMaybeReactive<Color>,
    {
        self.background_color = Some(c.into_maybe_reactive());
        self
    }

    /// Equivalent of CSS `overflow: hidden`. Children that extend
    /// past this stack's bounds are clipped at draw time. Layout
    /// still positions them at their full computed sizes.
    pub fn clip<V>(mut self, c: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.clip = Some(c.into_maybe_reactive());
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
            background_color: self.background_color,
            clip: self.clip,
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
    type State = ElementState<(), Ch::State>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("stack");
        let mut effects = Vec::new();

        // Default direction = Column when caller didn't specify (the
        // bare `stack()` constructor). vstack/hstack pre-fill Some.
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
            if let Some(eff) = install(v, move |w| set_flex_wrap(e.as_node(), w))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align_content {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_content(e.as_node(), a))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.justify_items {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |j| set_justify_items(e.as_node(), j))
            {
                effects.push(eff);
            }
        }
        effects.extend(apply_flex_item_extras(&el, self.shrink, self.basis));
        if let Some(v) = self.background_color {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |c| set_background_color(e.as_node(), c))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.clip {
            let e = el.clone();
            if let Some(eff) = install(v, move |c| set_clip(e.as_node(), c)) {
                effects.push(eff);
            }
        }
        effects.extend(apply_layout(&el, self.layout));
        effects.extend(apply_universal(&el, self.universal));

        // Build children but DON'T mount them yet. Mounting is
        // deferred until ElementState::mount runs (when self.el has
        // joined a tree); the recursive mount cascade then registers
        // every descendant in the right Taffy tree.
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
// grid() — Taffy CSS-Grid container (2-D layout)
// ---------------------------------------------------------------------

/// CSS-Grid container. Mirrors the `Stack` builder shape, but the
/// underlying Taffy node uses `Display::Grid`. Template-track lists,
/// implicit-track sizing, auto-flow, axis-specific gaps, and item-
/// alignment are configurable; per-cell placement is set via
/// `grid_column*` / `grid_row*` methods on the child elements (see
/// `WithLayout` in `renderer::attrs`).
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

    background_color: Option<MaybeReactive<Color>>,
    clip:             Option<MaybeReactive<bool>>,
    layout:           LayoutAttrs,
    universal:        UniversalAttrs,
    children:         Children,
}

/// Empty CSS-Grid container. Configure tracks via `.columns(...)` /
/// `.rows(...)`; place children via `.grid_column(...)` /
/// `.grid_row(...)` on each child.
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
        background_color: None,
        clip: None,
        layout: LayoutAttrs::default(),
        universal: UniversalAttrs::default(),
        children: (),
    }
}

impl<Ch> Grid<Ch> {
    /// `grid-template-columns` — explicit track list. Takes anything
    /// that converts into `Vec<GridTemplateComponent>` (e.g.
    /// `[fr(1.0), fr(2.0), auto()]`).
    pub fn columns(mut self, t: impl Into<Vec<GridTemplateComponent>>) -> Self {
        self.columns = Some(t.into());
        self
    }

    /// `grid-template-rows`.
    pub fn rows(mut self, t: impl Into<Vec<GridTemplateComponent>>) -> Self {
        self.rows = Some(t.into());
        self
    }

    /// `grid-auto-columns` — sizing for implicit columns (used when
    /// children overflow the explicit `columns` list).
    pub fn auto_columns(
        mut self,
        t: impl Into<Vec<TrackSizingFunction>>,
    ) -> Self {
        self.auto_columns = Some(t.into());
        self
    }

    /// `grid-auto-rows`.
    pub fn auto_rows(mut self, t: impl Into<Vec<TrackSizingFunction>>) -> Self {
        self.auto_rows = Some(t.into());
        self
    }

    /// `grid-auto-flow` — Row / Column / RowDense / ColumnDense.
    pub fn auto_flow<V: IntoMaybeReactive<GridAutoFlow>>(mut self, v: V) -> Self {
        self.auto_flow = Some(v.into_maybe_reactive());
        self
    }

    /// CSS `gap` shorthand — sets both `column_gap` and `row_gap` to
    /// the same value. Per-axis overrides win if also set.
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

    /// Layer-backed background fill.
    pub fn background_color<V>(mut self, c: V) -> Self
    where
        V: IntoMaybeReactive<Color>,
    {
        self.background_color = Some(c.into_maybe_reactive());
        self
    }

    /// CSS `overflow: hidden` — children outside the grid's frame are
    /// clipped at draw time.
    pub fn clip<V>(mut self, c: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.clip = Some(c.into_maybe_reactive());
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
            background_color: self.background_color,
            clip: self.clip,
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
    type State = ElementState<(), Ch::State>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("grid");
        let mut effects = Vec::new();

        // Static template-track lists go straight onto the node (no
        // reactive wrapper — animating the track shape is a v2 thing).
        if let Some(c) = self.columns {
            set_grid_template_columns(el.as_node(), c);
        }
        if let Some(r) = self.rows {
            set_grid_template_rows(el.as_node(), r);
        }
        if let Some(c) = self.auto_columns {
            set_grid_auto_columns(el.as_node(), c);
        }
        if let Some(r) = self.auto_rows {
            set_grid_auto_rows(el.as_node(), r);
        }

        if let Some(v) = self.auto_flow {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |f| set_grid_auto_flow(e.as_node(), f))
            {
                effects.push(eff);
            }
        }

        // Apply shorthand `gap` first so per-axis overrides win.
        if let Some(v) = self.gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_gap(e.as_node(), g)) {
                effects.push(eff);
            }
        }
        if let Some(v) = self.column_gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_column_gap(e.as_node(), g))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.row_gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_row_gap(e.as_node(), g)) {
                effects.push(eff);
            }
        }

        if let Some(v) = self.justify_items {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |j| set_justify_items(e.as_node(), j))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align_items {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_items(e.as_node(), a))
            {
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
        if let Some(v) = self.align_content {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_content(e.as_node(), a))
            {
                effects.push(eff);
            }
        }

        if let Some(v) = self.background_color {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |c| set_background_color(e.as_node(), c))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.clip {
            let e = el.clone();
            if let Some(eff) = install(v, move |c| set_clip(e.as_node(), c)) {
                effects.push(eff);
            }
        }

        effects.extend(apply_layout(&el, self.layout));
        effects.extend(apply_universal(&el, self.universal));

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
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    // Text styling — Button uses font_size + alignment only (no
    // text_color; that would need attributedTitle). Stored in the
    // shared `text` struct with `text_color: None`.
    text: CocoaText,
    // Button-specific.
    bordered: Option<MaybeReactive<bool>>,
    key_equivalent: Option<MaybeReactive<String>>,
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
        text: CocoaText::default(),
        bordered: None,
        key_equivalent: None,
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
            .push(crate::event_macos::PendingHandler::Click(Box::new(
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
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }

    /// Toggle whether the button draws its bezel. `false` →
    /// borderless / link-style.
    pub fn bordered<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.bordered = Some(b.into_maybe_reactive());
        self
    }

    /// Set a keyboard shortcut. `"\r"` (Return) makes this the
    /// default action button (highlighted, fires on Enter);
    /// `"\u{1b}"` (Escape) for cancel; any single-character
    /// string for arbitrary keys.
    pub fn key_equivalent<V>(mut self, key: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.key_equivalent = Some(key.into_maybe_reactive());
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
    /// [`PendingHandler`](crate::event_macos::PendingHandler) is
    /// pushed onto a Vec and applied during `Render::build` once
    /// the underlying NSView exists.
    ///
    /// The `Self: SupportsEvent<E>` bound rejects events the
    /// element doesn't accept — `<button on:input=...>` won't
    /// compile.
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_macos::SupportsEvent<E>,
        E: crate::event_macos::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

// Buttons fire on click (NSButton target/action). Generic over
// At because every type-level attribute extension still describes
// the same control kind.
impl crate::event_macos::SupportsEvent<crate::event_macos::ClickEvent>
    for Button
{
}

impl WithLayout for Button {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for Button {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}
impl WithText for Button {
    fn text_attrs_mut(&mut self) -> &mut CocoaText { &mut self.text }
}

// AddAnyAttr — the typed-attribute pipeline. Each call extends
// `attrs` from `At` to `<At as NextAttribute>::Output<NewAttr>`.
// At Render::build time, `attrs.build(&el)` walks the resulting
// tuple and runs each attribute's `build(&el)` against the live
// NSView.

impl Render<Dom> for Button
where
{
    type State = ElementState<(), ()>;

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

        if let Some(b) = self.bordered {
            let el_for = el.clone();
            if let Some(eff) =
                install(b, move |v| el_for.set_button_bordered(v))
            {
                effects.push(eff);
            }
        }
        if let Some(k) = self.key_equivalent {
            let el_for = el.clone();
            if let Some(eff) =
                install(k, move |v| el_for.set_key_equivalent(&v))
            {
                effects.push(eff);
            }
        }
        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_text(&el, self.text));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);

        // Run the typed-attribute pipeline. For the empty-tuple
        // default this is `().build(&el)` — a no-op.

        ElementState {
            el,
            _effects: effects,
            _attrs: std::marker::PhantomData,
            children: (),
        }
    }

    fn rebuild(self, _state: &mut Self::State) {
        // Reactive attrs already update themselves via their Effects.
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
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    text: CocoaText,
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
        text: CocoaText::default(),
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
        Self: crate::event_macos::SupportsEvent<E>,
        E: crate::event_macos::EventDescriptor,
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
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

// A checkbox toggles on click.
impl crate::event_macos::SupportsEvent<crate::event_macos::ClickEvent>
    for Checkbox
{
}

impl WithLayout for Checkbox {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for Checkbox {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}
impl WithText for Checkbox {
    fn text_attrs_mut(&mut self) -> &mut CocoaText { &mut self.text }
}


impl Render<Dom> for Checkbox
where
{
    type State = ElementState<(), ()>;

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

        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_text(&el, self.text));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// slider() — NSSlider with min/max + bind:value
// ---------------------------------------------------------------------

pub struct Slider {
    value: MaybeReactive<f64>,
    min_value: f64,
    max_value: f64,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::cocoa::bind::BoundFloat>,
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    vertical: Option<MaybeReactive<bool>>,
    num_tick_marks: Option<MaybeReactive<usize>>,
    snaps_to_ticks: Option<MaybeReactive<bool>>,
}

pub fn slider() -> Slider {
    Slider {
        value: MaybeReactive::Static(0.0),
        min_value: 0.0,
        max_value: 1.0,
        enabled: None,
        pending_bind: None,
        handlers: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
        vertical: None,
        num_tick_marks: None,
        snaps_to_ticks: None,
        node_ref: None,
        directives: Vec::new(),
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
        bound: crate::cocoa::bind::BoundFloat,
    ) {
        self.pending_bind = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_macos::SupportsEvent<E>,
        E: crate::event_macos::EventDescriptor,
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
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

impl WithLayout for Slider {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for Slider {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl Slider {
    /// Force vertical orientation. Default (None) lets AppKit
    /// pick based on the slider's frame ratio.
    pub fn vertical<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.vertical = Some(b.into_maybe_reactive());
        self
    }
    /// Number of evenly-spaced tick marks. 0 hides them.
    pub fn num_tick_marks<V>(mut self, n: V) -> Self
    where
        V: IntoMaybeReactive<usize>,
    {
        self.num_tick_marks = Some(n.into_maybe_reactive());
        self
    }
    /// Snap drag values to the nearest tick mark.
    pub fn snaps_to_ticks<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.snaps_to_ticks = Some(b.into_maybe_reactive());
        self
    }
}


impl Render<Dom> for Slider
where
{
    type State = ElementState<(), ()>;

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

        if let Some(v) = self.vertical {
            let el_for = el.clone();
            if let Some(eff) =
                install(v, move |x| el_for.set_slider_vertical(x))
            {
                effects.push(eff);
            }
        }
        if let Some(n) = self.num_tick_marks {
            let el_for = el.clone();
            if let Some(eff) =
                install(n, move |x| el_for.set_slider_tick_marks(x))
            {
                effects.push(eff);
            }
        }
        if let Some(s) = self.snaps_to_ticks {
            let el_for = el.clone();
            if let Some(eff) = install(s, move |x| {
                el_for.set_slider_snaps_to_ticks(x)
            }) {
                effects.push(eff);
            }
        }
        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// pop_up_button() — NSPopUpButton with items + bind:selection
// ---------------------------------------------------------------------

pub struct PopUpButton {
    items: Vec<String>,
    selection: MaybeReactive<usize>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_selection: Option<crate::cocoa::bind::BoundIndex>,
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    pulls_down: Option<MaybeReactive<bool>>,
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
        pulls_down: None,
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

    pub(crate) fn set_pending_bind_selection(
        &mut self,
        bound: crate::cocoa::bind::BoundIndex,
    ) {
        self.pending_bind_selection = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_macos::SupportsEvent<E>,
        E: crate::event_macos::EventDescriptor,
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
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

impl WithLayout for PopUpButton {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for PopUpButton {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl PopUpButton {
    /// `false` (default) → popup mode; `true` → pull-down menu.
    pub fn pulls_down<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.pulls_down = Some(b.into_maybe_reactive());
        self
    }
}


impl Render<Dom> for PopUpButton
where
{
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("pop_up_button");
        let mut effects = Vec::new();

        // pulls_down BEFORE items: NSPopUpButton's mode controls
        // how items are presented. Set the mode first.
        if let Some(p) = self.pulls_down {
            let el_for = el.clone();
            if let Some(eff) =
                install(p, move |v| el_for.set_pulls_down(v))
            {
                effects.push(eff);
            }
        }

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

        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// label() — static or reactive text
// ---------------------------------------------------------------------
//
// Backed by an Element wrapping a non-editable NSTextField (the
// "label" tag). Treated as a regular Element rather than a Text so
// `<label on:click=…>` and `<MyComponent>` returning a bare label
// can flow attached events / attributes through the standard
// AddAnyAttr pipeline.

pub struct Label {
    value: MaybeReactive<String>,
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    text: CocoaText,
    selectable: Option<MaybeReactive<bool>>,
}

impl Label {
    /// Internal: stash a `bind:value=...` (read-direction only) for
    /// installation in `Render::build`. Used by the `BindAttribute`
    /// impl in `crate::cocoa::bind`. Equivalent to `.text(closure)`.
    pub(crate) fn set_pending_bind_text(
        &mut self,
        getter: Box<dyn Fn() -> String + Send + 'static>,
    ) {
        self.value = MaybeReactive::Reactive(Box::new(move || getter()));
    }
}

pub fn label() -> Label {
    Label {
        value: MaybeReactive::Static(String::new()),
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
        text: CocoaText::default(),
        selectable: None,
    }
}

impl Label {
    pub fn text<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.value = value.into_maybe_reactive();
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

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_macos::SupportsEvent<E>,
        E: crate::event_macos::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

// Label is non-editable — treat it as a passive surface for events.
// Click is the only natural one (a label as part of a tappable
// "row" pattern). NSTextField *is* an NSControl so the existing
// on_action / on_click NSButton-downcast path won't fire — labels
// route Click via on_action instead (same as ColorWell etc.).
impl crate::event_macos::SupportsEvent<crate::event_macos::ClickEvent>
    for Label
{
}

impl WithLayout for Label {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for Label {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}
impl WithText for Label {
    fn text_attrs_mut(&mut self) -> &mut CocoaText { &mut self.text }
}

impl Label {
    /// Allow the label's text to be selected (and copied with
    /// ⌘C). NSTextField labels are non-selectable by default.
    pub fn selectable<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.selectable = Some(b.into_maybe_reactive());
        self
    }
}


impl Render<Dom> for Label
where
{
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("label");
        let mut effects = Vec::new();

        let el_for_text = el.clone();
        if let Some(eff) = install(self.value, move |s| {
            el_for_text.set_string_attribute(StringAttr::Value, &s);
        }) {
            effects.push(eff);
        }

        for h in self.handlers {
            // NSTextField (label) is an NSControl but not an
            // NSButton. Route Click via on_action; other events
            // fall through to apply_to (no-ops on non-NSTextField
            // events, which is most of them on a label).
            match h {
                crate::event_macos::PendingHandler::Click(cb) => {
                    el.on_action(cb);
                }
                other => other.apply_to(&el),
            }
        }

        if let Some(s) = self.selectable {
            let el_for = el.clone();
            if let Some(eff) =
                install(s, move |v| el_for.set_selectable(v))
            {
                effects.push(eff);
            }
        }
        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_text(&el, self.text));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    text: CocoaText,
    bordered: Option<MaybeReactive<bool>>,
    bezeled: Option<MaybeReactive<bool>>,
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
        text: CocoaText::default(),
        bordered: None,
        bezeled: None,
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
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
        text: CocoaText::default(),
        bordered: None,
        bezeled: None,
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
        Self: crate::event_macos::SupportsEvent<E>,
        E: crate::event_macos::EventDescriptor,
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
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
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
impl crate::event_macos::SupportsEvent<crate::event_macos::InputEvent>
    for TextField
{
}
impl crate::event_macos::SupportsEvent<crate::event_macos::ChangeEvent>
    for TextField
{
}
impl crate::event_macos::SupportsEvent<crate::event_macos::FocusEvent>
    for TextField
{
}
impl crate::event_macos::SupportsEvent<crate::event_macos::BlurEvent>
    for TextField
{
}
impl crate::event_macos::SupportsEvent<crate::event_macos::KeyDownEvent>
    for TextField
{
}
impl crate::event_macos::SupportsEvent<crate::event_macos::KeyUpEvent>
    for TextField
{
}

impl WithLayout for TextField {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for TextField {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}
impl WithText for TextField {
    fn text_attrs_mut(&mut self) -> &mut CocoaText { &mut self.text }
}

impl TextField {
    /// Toggle the field's border. `false` → label-style flat
    /// appearance even on editable fields.
    pub fn bordered<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.bordered = Some(b.into_maybe_reactive());
        self
    }
    /// Toggle the field's bezel (the inset 3D look).
    pub fn bezeled<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.bezeled = Some(b.into_maybe_reactive());
        self
    }
}


impl Render<Dom> for TextField
where
{
    type State = ElementState<(), ()>;

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

        if let Some(b) = self.bordered {
            let el_for = el.clone();
            if let Some(eff) = install(b, move |v| {
                el_for.set_text_field_bordered(v)
            }) {
                effects.push(eff);
            }
        }
        if let Some(b) = self.bezeled {
            let el_for = el.clone();
            if let Some(eff) = install(b, move |v| {
                el_for.set_text_field_bezeled(v)
            }) {
                effects.push(eff);
            }
        }
        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_text(&el, self.text));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// date_picker() — NSDatePicker
// ---------------------------------------------------------------------

pub struct DatePicker {
    value: MaybeReactive<cocoa_dom::Date>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::cocoa::bind::BoundDate>,
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    style: Option<MaybeReactive<cocoa_dom::DatePickerStyle>>,
    min_date: Option<MaybeReactive<cocoa_dom::Date>>,
    max_date: Option<MaybeReactive<cocoa_dom::Date>>,
}

pub fn date_picker() -> DatePicker {
    DatePicker {
        value: MaybeReactive::Static(cocoa_dom::Date::now()),
        enabled: None,
        pending_bind: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
        style: None,
        min_date: None,
        max_date: None,
    }
}

impl DatePicker {
    pub fn value<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<cocoa_dom::Date>,
    {
        self.value = v.into_maybe_reactive();
        self
    }

    pub fn enabled<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.enabled = Some(value.into_maybe_reactive());
        self
    }

    pub(crate) fn set_pending_bind_date(
        &mut self,
        bound: crate::cocoa::bind::BoundDate,
    ) {
        self.pending_bind = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_macos::SupportsEvent<E>,
        E: crate::event_macos::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

// NSDatePicker fires target/action when the user changes the date.
// As with ColorWell, we use the existing Click marker — semantically
// "change" but that's what the macro emits and the wiring works.
impl crate::event_macos::SupportsEvent<crate::event_macos::ClickEvent>
    for DatePicker
{
}

impl WithLayout for DatePicker {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for DatePicker {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl DatePicker {
    /// Set the picker's visual style. `Textual`,
    /// `TextualAndStepper` (default), or `ClockAndCalendar`.
    pub fn style<V>(mut self, s: V) -> Self
    where
        V: IntoMaybeReactive<cocoa_dom::DatePickerStyle>,
    {
        self.style = Some(s.into_maybe_reactive());
        self
    }
    /// Earliest selectable date. Builder API doesn't expose a
    /// "clear" path; use a directive to call
    /// `Element::set_date_picker_min(None)` if you need that.
    pub fn min_date<V>(mut self, d: V) -> Self
    where
        V: IntoMaybeReactive<cocoa_dom::Date>,
    {
        self.min_date = Some(d.into_maybe_reactive());
        self
    }
    /// Latest selectable date.
    pub fn max_date<V>(mut self, d: V) -> Self
    where
        V: IntoMaybeReactive<cocoa_dom::Date>,
    {
        self.max_date = Some(d.into_maybe_reactive());
        self
    }
}


impl Render<Dom> for DatePicker
where
{
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("date_picker");
        let mut effects = Vec::new();

        let el_for_val = el.clone();
        if let Some(eff) = install(self.value, move |d| {
            el_for_val.set_date_picker_value(d);
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
            let eff =
                crate::cocoa::bind::install_date_picker_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            // Date picker is an NSControl, not an NSButton — route
            // Click via on_action.
            match h {
                crate::event_macos::PendingHandler::Click(cb) => {
                    el.on_action(cb);
                }
                other => other.apply_to(&el),
            }
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
            if let Some(eff) = install(d, move |v| {
                el_for.set_date_picker_min(Some(v))
            }) {
                effects.push(eff);
            }
        }
        if let Some(d) = self.max_date {
            let el_for = el.clone();
            if let Some(eff) = install(d, move |v| {
                el_for.set_date_picker_max(Some(v))
            }) {
                effects.push(eff);
            }
        }
        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// stepper() — NSStepper, +/- numeric increment
// ---------------------------------------------------------------------

pub struct Stepper {
    value: MaybeReactive<f64>,
    min_value: f64,
    max_value: f64,
    increment: f64,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::cocoa::bind::BoundFloat>,
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
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
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
    }
}

impl Stepper {
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

    pub fn increment(mut self, v: f64) -> Self {
        self.increment = v;
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
        bound: crate::cocoa::bind::BoundFloat,
    ) {
        self.pending_bind = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_macos::SupportsEvent<E>,
        E: crate::event_macos::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

impl crate::event_macos::SupportsEvent<crate::event_macos::ClickEvent>
    for Stepper
{
}

impl WithLayout for Stepper {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for Stepper {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}


impl Render<Dom> for Stepper
where
{
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("stepper");
        let mut effects = Vec::new();

        // Configure bounds + increment first so the initial
        // setDoubleValue clamps correctly.
        el.configure_stepper(
            self.min_value,
            self.max_value,
            self.increment,
        );

        let el_for_val = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_val.set_stepper_value(v);
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
            let eff =
                crate::cocoa::bind::install_stepper_value_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            match h {
                crate::event_macos::PendingHandler::Click(cb) => {
                    el.on_action(cb);
                }
                other => other.apply_to(&el),
            }
        }

        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// progress_indicator() — NSProgressIndicator. Bar (determinate) or
// spinner (indeterminate). Read-only — `value=` drives the bar but
// there's no bind here, since user input doesn't reach a progress
// indicator.
// ---------------------------------------------------------------------

pub struct ProgressIndicator {
    value: MaybeReactive<f64>,
    max_value: f64,
    indeterminate: bool,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    displayed_when_stopped: Option<MaybeReactive<bool>>,
}

pub fn progress_indicator() -> ProgressIndicator {
    ProgressIndicator {
        value: MaybeReactive::Static(0.0),
        max_value: 1.0,
        indeterminate: false,
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
        displayed_when_stopped: None,
    }
}

impl ProgressIndicator {
    pub fn value<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<f64>,
    {
        self.value = v.into_maybe_reactive();
        self
    }

    pub fn max_value(mut self, v: f64) -> Self {
        self.max_value = v;
        self
    }

    /// `true` switches to spinner mode and starts the animation;
    /// `false` is a determinate progress bar.
    pub fn indeterminate(mut self, b: bool) -> Self {
        self.indeterminate = b;
        self
    }

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

impl WithLayout for ProgressIndicator {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for ProgressIndicator {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl ProgressIndicator {
    /// Whether the indicator stays visible while stopped (vs
    /// hiding itself entirely). Only meaningful in indeterminate
    /// (spinner) mode.
    pub fn displayed_when_stopped<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.displayed_when_stopped = Some(b.into_maybe_reactive());
        self
    }
}


impl Render<Dom> for ProgressIndicator
where
{
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("progress_indicator");
        let mut effects = Vec::new();

        // Order matters: max before value so the value clamps
        // correctly; indeterminate after both because indeterminate
        // mode ignores value (and starts the animation).
        el.set_progress_max(self.max_value);

        let el_for_val = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_val.set_progress_value(v);
        }) {
            effects.push(eff);
        }

        el.set_progress_indeterminate(self.indeterminate);

        if let Some(d) = self.displayed_when_stopped {
            let el_for = el.clone();
            if let Some(eff) = install(d, move |v| {
                el_for.set_progress_displayed_when_stopped(v)
            }) {
                effects.push(eff);
            }
        }
        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// color_well() — NSColorWell, opens system color picker on click.
// `value=` for one-way; `bind:value=` for two-way.
// ---------------------------------------------------------------------

pub struct ColorWell {
    value: MaybeReactive<cocoa_dom::Color>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::cocoa::bind::BoundColor>,
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
}

pub fn color_well() -> ColorWell {
    ColorWell {
        value: MaybeReactive::Static(cocoa_dom::Color::WHITE),
        enabled: None,
        pending_bind: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
    }
}

impl ColorWell {
    pub fn value<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<cocoa_dom::Color>,
    {
        self.value = v.into_maybe_reactive();
        self
    }

    pub fn enabled<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.enabled = Some(value.into_maybe_reactive());
        self
    }

    pub(crate) fn set_pending_bind_color(
        &mut self,
        bound: crate::cocoa::bind::BoundColor,
    ) {
        self.pending_bind = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_macos::SupportsEvent<E>,
        E: crate::event_macos::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

// NSColorWell fires target/action when the user picks a color and
// dismisses the picker. We use the existing `Click` event marker
// here because that's what the macro emits for `on:click=…` — but
// semantically it's a "value committed" event, more like
// `on:change` would be on the web. Document this divergence
// rather than introduce a separate Color-payload event for now.
impl crate::event_macos::SupportsEvent<crate::event_macos::ClickEvent>
    for ColorWell
{
}

impl WithLayout for ColorWell {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for ColorWell {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}


impl Render<Dom> for ColorWell
where
{
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("color_well");
        let mut effects = Vec::new();

        let el_for_val = el.clone();
        if let Some(eff) = install(self.value, move |c| {
            el_for_val.set_color_well_value(c);
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
            let eff =
                crate::cocoa::bind::install_color_well_bind(&el, bound);
            effects.push(eff);
        }

        for h in self.handlers {
            // ColorWell is an NSControl, not an NSButton — route
            // Click via on_action so the target/action wiring fires
            // when the user picks a color.
            match h {
                crate::event_macos::PendingHandler::Click(cb) => {
                    el.on_action(cb);
                }
                other => other.apply_to(&el),
            }
        }

        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// segmented_control() — NSSegmentedControl with items + bind:selection
// ---------------------------------------------------------------------

pub struct SegmentedControl {
    items: Vec<String>,
    selection: MaybeReactive<usize>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_selection: Option<crate::cocoa::bind::BoundIndex>,
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    segment_style: Option<MaybeReactive<cocoa_dom::SegmentStyle>>,
}

pub fn segmented_control() -> SegmentedControl {
    SegmentedControl {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        enabled: None,
        pending_bind_selection: None,
        handlers: Vec::new(),
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
        segment_style: None,
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
        bound: crate::cocoa::bind::BoundIndex,
    ) {
        self.pending_bind_selection = Some(bound);
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: crate::event_macos::SupportsEvent<E>,
        E: crate::event_macos::EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

// Click semantics for segmented_control match popup: a "click"
// is a selection change.
impl crate::event_macos::SupportsEvent<crate::event_macos::ClickEvent>
    for SegmentedControl
{
}

impl WithLayout for SegmentedControl {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for SegmentedControl {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl SegmentedControl {
    /// Visual style: `Rounded`, `RoundRect`, `Capsule`,
    /// `SmallSquare`, `Separated`, etc. See
    /// `cocoa_dom::SegmentStyle`.
    pub fn segment_style<V>(mut self, s: V) -> Self
    where
        V: IntoMaybeReactive<cocoa_dom::SegmentStyle>,
    {
        self.segment_style = Some(s.into_maybe_reactive());
        self
    }
}


impl Render<Dom> for SegmentedControl
where
{
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("segmented_control");
        let mut effects = Vec::new();

        el.set_segmented_items(&self.items);

        let el_for_sel = el.clone();
        if let Some(eff) = install(self.selection, move |i| {
            el_for_sel.set_segmented_selection(i as isize);
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
            let eff =
                crate::cocoa::bind::install_segmented_selection_bind(
                    &el, bound,
                );
            effects.push(eff);
        }

        for h in self.handlers {
            // Click on a segmented control is conceptually
            // "selection changed" — install via on_action (NSControl
            // path) rather than on_click (NSButton subtree only).
            match h {
                crate::event_macos::PendingHandler::Click(cb) => {
                    el.on_action(cb);
                }
                other => other.apply_to(&el),
            }
        }

        if let Some(s) = self.segment_style {
            let el_for = el.clone();
            if let Some(eff) =
                install(s, move |v| el_for.set_segment_style(v))
            {
                effects.push(eff);
            }
        }
        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// scroll_view() — NSScrollView wrapping arbitrary child content
// ---------------------------------------------------------------------
//
// Same shape as `View<Children, At>` (children + attribute pipeline).
// The scroll view's documentView is a FlippedView built at construction
// in `cocoa_dom::node::Element::create_with`; child mounts route there
// via `Element::subview_parent`. apply_layout special-cases NSScrollView
// to walk documentView's subviews and size documentView to the union
// of children's rects (so NSScrollView shows scroll bars when content
// overflows the viewport).

pub struct ScrollView<Children> {
    children: Children,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    autohides_scrollers: Option<MaybeReactive<bool>>,
    has_horizontal_scroller: Option<MaybeReactive<bool>>,
    has_vertical_scroller: Option<MaybeReactive<bool>>,
}

pub fn scroll_view() -> ScrollView<()> {
    ScrollView {
        children: (),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
        autohides_scrollers: None,
        has_horizontal_scroller: None,
        has_vertical_scroller: None,
    }
}

impl<Ch> ScrollView<Ch> {
    /// Auto-hide the scrollers when not in use (the default
    /// macOS overlay-scroller behavior).
    pub fn autohides_scrollers<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.autohides_scrollers = Some(b.into_maybe_reactive());
        self
    }

    /// Show / hide the horizontal scroller. Default at construct
    /// time is `false`.
    pub fn has_horizontal_scroller<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.has_horizontal_scroller =
            Some(b.into_maybe_reactive());
        self
    }

    /// Show / hide the vertical scroller. Default at construct
    /// time is `true`.
    pub fn has_vertical_scroller<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.has_vertical_scroller =
            Some(b.into_maybe_reactive());
        self
    }

    pub fn child<NewCh>(self, child: NewCh) -> ScrollView<(Ch, NewCh)> {
        ScrollView {
            children: (self.children, child),
            universal: self.universal,
            layout: self.layout,
            autohides_scrollers: self.autohides_scrollers,
            has_horizontal_scroller: self.has_horizontal_scroller,
            has_vertical_scroller: self.has_vertical_scroller,
        }
    }
}

impl<Ch> WithLayout for ScrollView<Ch> {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl<Ch> WithUniversal for ScrollView<Ch> {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}

impl<Ch> Render<Dom> for ScrollView<Ch>
where
    Ch: Render<Dom>,
{
    type State = ElementState<(), Ch::State>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("scroll_view");
        let mut effects = Vec::new();

        if let Some(b) = self.autohides_scrollers {
            let el_for = el.clone();
            if let Some(eff) = install(b, move |v| {
                el_for.set_autohides_scrollers(v)
            }) {
                effects.push(eff);
            }
        }
        if let Some(b) = self.has_horizontal_scroller {
            let el_for = el.clone();
            if let Some(eff) = install(b, move |v| {
                el_for.set_has_horizontal_scroller(v)
            }) {
                effects.push(eff);
            }
        }
        if let Some(b) = self.has_vertical_scroller {
            let el_for = el.clone();
            if let Some(eff) = install(b, move |v| {
                el_for.set_has_vertical_scroller(v)
            }) {
                effects.push(eff);
            }
        }
        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_layout(&el, self.layout));

        // Same cascade pattern as View: defer child mount to
        // ElementState::mount, so the tree-aware insert_node
        // registers each descendant in the right Taffy tree.
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
// image_view() — NSImageView, source from a file path
// ---------------------------------------------------------------------

pub struct ImageView {
    source: MaybeReactive<String>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
}

pub fn image_view() -> ImageView {
    ImageView {
        source: MaybeReactive::Static(String::new()),
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
    }
}

impl ImageView {
    /// File path to the image. Empty string clears the image.
    /// Network URLs aren't supported here — fetch them yourself
    /// (e.g. via reqwest) and write to a temp file, then pass the
    /// path. NSImage's `initWithContentsOfFile:` handles PNG, JPEG,
    /// PDF, TIFF, etc.
    pub fn source<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.source = v.into_maybe_reactive();
        self
    }

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

impl WithLayout for ImageView {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for ImageView {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}


impl Render<Dom> for ImageView
where
{
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("image_view");
        let mut effects = Vec::new();

        let el_for_src = el.clone();
        if let Some(eff) = install(self.source, move |s| {
            el_for_src.set_image_view_path(&s);
        }) {
            effects.push(eff);
        }

        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// text_view() — multi-line plain-text editor (NSScrollView wrapping
// an NSTextView). No event hooks yet — NSTextViewDelegate is a
// separate protocol; add when an example needs it.
// ---------------------------------------------------------------------

pub struct TextView {
    value: MaybeReactive<String>,
    enabled: Option<MaybeReactive<bool>>,
    /// `bind:value=…` two-way binding. Distinct from `.value(...)`
    /// (one-way: signal → field). Installed at build time via
    /// `install_text_view_value_bind`.
    pending_bind: Option<crate::cocoa::bind::BoundValue>,
    node_ref: Option<crate::cocoa::NodeRef>,
    directives: Vec<Box<dyn FnOnce(&CocoaElement) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    text: CocoaText,
}

pub fn text_view() -> TextView {
    TextView {
        value: MaybeReactive::Static(String::new()),
        enabled: None,
        pending_bind: None,
        node_ref: None,
        directives: Vec::new(),
        universal: UniversalAttrs::default(),
        layout: LayoutAttrs::default(),
        text: CocoaText::default(),
    }
}

impl TextView {
    pub fn value<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.value = v.into_maybe_reactive();
        self
    }

    pub fn enabled<V>(mut self, value: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.enabled = Some(value.into_maybe_reactive());
        self
    }

    /// Internal: stash a `bind:value=…` for installation in
    /// `Render::build`. Used by the `BindAttribute` impl in
    /// `crate::cocoa::bind`.
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::cocoa::bind::BoundValue,
    ) {
        self.pending_bind = Some(bound);
    }

    pub fn node_ref(mut self, r: crate::cocoa::NodeRef) -> Self {
        self.node_ref = Some(r);
        self
    }

    pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
    where
        D: crate::directive::IntoDirective<cocoa_dom::Element, T, P> + Send + 'static,
        P: Send + 'static,
        T: 'static,
    {
        self.directives
            .push(crate::cocoa::directives::pack(handler, param));
        self
    }
}

impl WithLayout for TextView {
    fn layout_mut(&mut self) -> &mut LayoutAttrs { &mut self.layout }
}
impl WithUniversal for TextView {
    fn universal_mut(&mut self) -> &mut UniversalAttrs { &mut self.universal }
}
impl WithText for TextView {
    fn text_attrs_mut(&mut self) -> &mut CocoaText { &mut self.text }
}


impl Render<Dom> for TextView
where
{
    type State = ElementState<(), ()>;

    fn build(self) -> Self::State {
        let el = CocoaElement::create("text_view");
        let mut effects = Vec::new();

        // value=… one-way drive. Routes through StringAttr::Value
        // which knows how to find the inner NSTextView.
        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_string_attribute(StringAttr::Value, &v);
        }) {
            effects.push(eff);
        }

        // enabled=… toggles editability on the inner NSTextView.
        // NSScrollView/NSTextView aren't NSControls, so the
        // BoolAttr::Enabled path doesn't apply — we use the
        // dedicated `set_text_view_editable` method on Element
        // which routes through the scroll view's documentView.
        if let Some(enabled) = self.enabled {
            let el_for_enabled = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_enabled.set_text_view_editable(b);
            }) {
                effects.push(eff);
            }
        }

        // bind:value=… both directions. Wires NSTextView's
        // `textDidChange:` for write-back and an Effect that
        // pushes signal → setString.
        if let Some(bound) = self.pending_bind {
            let eff =
                crate::cocoa::bind::install_text_view_value_bind(&el, bound);
            effects.push(eff);
        }

        effects.extend(apply_universal(&el, self.universal));
        effects.extend(apply_text(&el, self.text));
        effects.extend(apply_layout(&el, self.layout));

        if let Some(r) = self.node_ref {
            r.load(&el);
        }

        crate::cocoa::directives::run_all(self.directives, &el);


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
// AddAnyAttr<Dom> for the 13 leaf builders. Routes spread attrs (e.g.
// `<MyComponent on:click=...>`) onto the existing `directives: Vec<...>`
// post-build hook, which gets drained in `Render::build` after the
// underlying NSView is constructed. Same install timing as a `use:`
// directive — the attr's `apply_to` is called with `&CocoaElement` and
// (for OnAttribute) routes to e.g. `el.on_click(cb)`, which silently
// no-ops on non-NSButton element kinds.
//
// Container builders (Stack, Block, ScrollView) intentionally don't
// implement AddAnyAttr — the `<App on:click=…>` use case where App
// wraps a vstack-of-buttons would no-op anyway (NSView has no click
// target), and threading through containers needs decisions about
// re-attach-on-rebuild for branching wrappers.

macro_rules! impl_add_any_attr_for_leaf {
    ($($builder:ident),+ $(,)?) => {
        $(
            impl renderer::view::AddAnyAttr<crate::Dom> for $builder {
                fn add_any_attr<__A>(mut self, attr: __A) -> Self
                where
                    __A: renderer::view::ApplyAttr<crate::Dom>,
                {
                    self.directives.push(Box::new(move |el: &CocoaElement| {
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
    DatePicker, Stepper, ProgressIndicator, ColorWell,
    SegmentedControl, ImageView, TextView,
);

// Container builders (Stack, Block, ScrollView) — no-op AddAnyAttr.
// Their underlying NSView (FlippedView / NSScrollView) doesn't have a
// click target, so OnAttribute on a container would no-op anyway.
// Container builders (Stack, Block, ScrollView). Their underlying
// NSView (FlippedView / NSScrollView) isn't an NSControl, so cocoa_dom
// silently no-ops on `on_click` at the dom layer. Rather than inherit
// the silent failure, panic here with a clear diagnostic.
//
// Future: NSClickGestureRecognizer integration so `<vstack on:click=…>`
// becomes meaningful, then route through that.

impl<Children> renderer::view::AddAnyAttr<crate::Dom> for Stack<Children> {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where
        __A: renderer::view::ApplyAttr<crate::Dom>,
    {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on Stack (vstack/hstack/             stack_view). Containers have no NSControl target/action              slot — click and other UIControl events have no install              path. Attach to a child button/label/text_field instead."
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
            "AddAnyAttr<Dom>::add_any_attr on Grid. Containers have no NSControl target/action slot — click and other UIControl events have no install path. Attach to a child button/label/text_field instead."
        )
    }
}

impl<Children> renderer::view::AddAnyAttr<crate::Dom> for ScrollView<Children> {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where
        __A: renderer::view::ApplyAttr<crate::Dom>,
    {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on ScrollView. NSScrollView              isn't an NSControl — click handlers have no install path.              Attach to inner content instead."
        )
    }
}
