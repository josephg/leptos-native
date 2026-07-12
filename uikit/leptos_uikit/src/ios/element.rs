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
    IosBackend,
};
use leptos_native::renderer::attrs::{
    DecorationAttrs, LayoutAttrs, TextAttrs, UniversalAttrs, WithLayout,
    WithUniversal,
};
use leptos_native::renderer::view::{ApplyAttr, Mountable, Render};
use crate::dom::{layout::{
    set_align_content, set_align_items, set_aspect_ratio, set_column_gap,
    set_flex_direction, set_gap, set_grid_auto_columns, set_grid_auto_flow,
    set_grid_auto_rows, set_grid_template_columns, set_grid_template_rows,
    set_inset, set_justify_content, set_justify_items, set_row_gap,
    AlignContent, AlignItems, FlexDirection, GridAutoFlow,
    GridTemplateComponent, JustifyContent, JustifyItems, Position,
    TrackSizingFunction,
}, Color, Date, DatePickerStyle, UikitElem, UikitMakeView, UikitNodeExt};
use reactive_graph::effect::RenderEffect;

use crate::dom::layout::{apply_layout, apply_universal};
use leptos_native::renderer::apply_decoration;
use leptos_native::node_ref::NodeRef;
use leptos_native::prelude::AddAnyAttr;

/// iOS's text-attr struct alias — `TextAttrs` with iOS's `Color`
/// and `NSTextAlignment`.
pub type IosText = TextAttrs<crate::dom::Color, crate::dom::TextAlignment>;

/// Port-local accessor trait for [`IosText`]. Mirrors the shape of
/// renderer-common's `WithLayout` / `WithUniversal`: each builder
/// implements `text_attrs_mut` returning `&mut self.text`; the
/// default methods supply the chainable setters.
///
/// Stays port-local rather than implementing renderer-common's
/// generic `WithText<C, A>` because the chainable setters need the
/// port-local [`IntoMaybeReactive`] (for UIKit-foreign types like
/// `NSTextAlignment` and `Color`). Renderer-common's `WithText` uses
/// its own renderer-common `IntoMaybeReactive`, which only has impls
/// for renderer-common-owned types.
pub trait WithText: Sized {
    fn text_attrs_mut(&mut self) -> &mut IosText;

    fn text_color<V: IntoMaybeReactive<crate::dom::Color>>(mut self, c: V) -> Self {
        self.text_attrs_mut().text_color = Some(c.into_maybe_reactive());
        self
    }
    /// Text alignment within the control's frame.
    fn alignment<V: IntoMaybeReactive<crate::dom::TextAlignment>>(
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
    /// Font weight, CSS-style 100..=900 (400 regular, 700 bold).
    fn font_weight<V: IntoMaybeReactive<i32>>(mut self, w: V) -> Self {
        self.text_attrs_mut().font_weight = Some(w.into_maybe_reactive());
        self
    }
}


/// Apply [`IosText`] (text_color, alignment, font_size) to the live
/// UIView.
fn apply_text(el: UikitElem, attrs: IosText) -> Vec<RenderEffect<()>> {
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
    if let Some(w) = attrs.font_weight {
        let el_for = el.clone();
        if let Some(eff) = install(w, move |v| el_for.set_font_weight(v)) {
            out.push(eff);
        }
    }
    out
}

/// Apply the always-present `universal` (+ optional `text`) and
/// `layout` cascade tail every typed builder runs. Layout LAST
/// because `hidden=Display::None` lives in `LayoutAttrs`. iOS-port
/// counterpart to cocoa's `apply_common`.
fn apply_common(
    el: UikitElem,
    universal: UniversalAttrs,
    text: Option<IosText>,
    layout: LayoutAttrs,
) -> Vec<RenderEffect<()>> {
    let mut effects = apply_universal(el, universal);
    if let Some(text) = text {
        effects.extend(apply_text(el, text));
    }
    effects.extend(apply_layout(el, layout));
    effects
}

/// iOS's decoration-attr struct alias — `DecorationAttrs<Color>`.
pub type IosDecoration = DecorationAttrs<Color>;

/// Port-local `WithDecoration` shadow. Same shape as the generic
/// `renderer::attrs::WithDecoration<C>`, but pins `C = Color` and uses
/// the port-local [`IntoMaybeReactive`] so chainable setters accept
/// either bare `Color` or `Fn() -> Color` closures.
pub trait WithDecoration: Sized {
    fn decoration_mut(&mut self) -> &mut IosDecoration;

    /// Background fill.
    fn background_color<V: IntoMaybeReactive<Color>>(mut self, c: V) -> Self {
        self.decoration_mut().background_color = Some(c.into_maybe_reactive());
        self
    }

    /// Round the corners (also enables `masksToBounds` when > 0).
    fn corner_radius<V: IntoMaybeReactive<f32>>(mut self, r: V) -> Self {
        self.decoration_mut().corner_radius = Some(r.into_maybe_reactive());
        self
    }

    /// Border width in points. `0.0` disables.
    fn border_width<V: IntoMaybeReactive<f32>>(mut self, w: V) -> Self {
        self.decoration_mut().border_width = Some(w.into_maybe_reactive());
        self
    }

    /// Border color. Only visible when `border_width > 0`.
    fn border_color<V: IntoMaybeReactive<Color>>(mut self, c: V) -> Self {
        self.decoration_mut().border_color = Some(c.into_maybe_reactive());
        self
    }

    /// Drop-shadow color. Only visible when `shadow_opacity > 0`.
    fn shadow_color<V: IntoMaybeReactive<Color>>(mut self, c: V) -> Self {
        self.decoration_mut().shadow_color = Some(c.into_maybe_reactive());
        self
    }

    /// Drop-shadow opacity, 0.0..=1.0. `0.0` disables. Setting a
    /// shadow disables `masksToBounds`, so pair with an inner
    /// clipping view when children must clip to rounded corners.
    fn shadow_opacity<V: IntoMaybeReactive<f32>>(mut self, o: V) -> Self {
        self.decoration_mut().shadow_opacity = Some(o.into_maybe_reactive());
        self
    }

    /// Drop-shadow blur radius in points.
    fn shadow_radius<V: IntoMaybeReactive<f32>>(mut self, r: V) -> Self {
        self.decoration_mut().shadow_radius = Some(r.into_maybe_reactive());
        self
    }

    /// Drop-shadow offset as `(dx, dy)` points.
    fn shadow_offset<V: IntoMaybeReactive<(f32, f32)>>(mut self, o: V) -> Self {
        self.decoration_mut().shadow_offset = Some(o.into_maybe_reactive());
        self
    }
}

// ---------------------------------------------------------------------
// Common builder state — the attrs/handlers every builder carries.
// ---------------------------------------------------------------------

/// The builder state shared by every element builder: event handlers,
/// spread attrs, `node_ref`, and the four chainable attr structs.
/// Builders embed one of these as `common` and get the accessor-trait
/// impls + `on` / `node_ref` methods from [`impl_common!`]. Mirrors
/// cocoa's `Common`.
#[derive(Default)]
pub struct Common {
    handlers: Vec<PendingHandler>,
    pending_spreads: Vec<Box<dyn FnOnce(UikitElem) + Send + 'static>>,
    node_ref: Option<NodeRef<UikitElem>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    decoration: IosDecoration,
    /// All-`None` (installs nothing) on builders that don't expose
    /// [`WithText`].
    text: IosText,
}

impl Common {
    /// The shared tail of every `Render::build`: install event
    /// handlers + spread attrs, apply the attr structs (layout LAST —
    /// see [`apply_common`]), load the `node_ref`.
    fn finish(self, el: UikitElem, effects: &mut Vec<RenderEffect<()>>) {
        for h in self.handlers {
            h.apply_to(el);
        }
        for f in self.pending_spreads {
            f(el);
        }
        effects.extend(apply_decoration(el, self.decoration));
        effects.extend(apply_common(
            el,
            self.universal,
            Some(self.text),
            self.layout,
        ));
        if let Some(r) = self.node_ref {
            r.load(el);
        }
    }
}

/// Generate the boilerplate every builder repeats over its `common`
/// field: the `WithLayout` / `WithUniversal` / `WithDecoration`
/// accessor impls plus the `on` / `node_ref` methods. Add `: text`
/// for builders that render text (also impls [`WithText`]).
macro_rules! impl_common {
    ($ty:ident $(<$g:ident>)?) => {
        impl $(<$g>)? WithLayout for $ty $(<$g>)? {
            fn layout_mut(&mut self) -> &mut LayoutAttrs {
                &mut self.common.layout
            }
        }
        impl $(<$g>)? WithUniversal for $ty $(<$g>)? {
            fn universal_mut(&mut self) -> &mut UniversalAttrs {
                &mut self.common.universal
            }
        }
        impl $(<$g>)? WithDecoration for $ty $(<$g>)? {
            fn decoration_mut(&mut self) -> &mut IosDecoration {
                &mut self.common.decoration
            }
        }
        impl $(<$g>)? $ty $(<$g>)? {
            /// `on:event=handler` — install an event handler. Which
            /// events a control supports is expressed via
            /// `SupportsEvent<E>` impls; unsupported events are a
            /// compile error.
            pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
            where
                Self: SupportsEvent<E>,
                E: EventDescriptor,
                F: FnMut(E::EventType) + Send + 'static,
            {
                self.common.handlers.push(E::into_pending(handler));
                self
            }

            /// Capture the built element in a `NodeRef` for imperative
            /// access (focus, measurement) after mount.
            pub fn node_ref(mut self, r: NodeRef<UikitElem>) -> Self {
                self.common.node_ref = Some(r);
                self
            }
        }
    };
    ($ty:ident $(<$g:ident>)? : text) => {
        impl_common!($ty $(<$g>)?);
        impl $(<$g>)? WithText for $ty $(<$g>)? {
            fn text_attrs_mut(&mut self) -> &mut IosText {
                &mut self.common.text
            }
        }
    };
}

// ---------------------------------------------------------------------
// ElementState — generic state for every builder
// ---------------------------------------------------------------------

pub struct ElementState<ChildState> {
    pub el: UikitElem,
    pub(crate) _effects: Vec<RenderEffect<()>>,
    pub(crate) children: ChildState,
}

impl<ChildState: Mountable<IosBackend>> Mountable<IosBackend>
    for ElementState<ChildState>
{
    fn unmount(&mut self) {
        self.children.unmount();
        // Drop reactive-attr effects before tearing the node down, so a
        // signal write queued just before unmount can't re-run a setter
        // against a freed node (and so the subscription is released
        // promptly rather than leaking until the `ElementState` value
        // drops). Each `RenderEffect` is the sole strong owner of its
        // `EffectInner`; dropping it ends the effect's driver future.
        self._effects.clear();
        self.el.remove();
    }

    fn mount(
        &mut self,
        parent: UikitElem,
        marker: Option<UikitElem>,
    ) {
        parent.insert_node(self.el, marker);
        self.children.mount(self.el, None);
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<IosBackend>) -> bool {
        self.el.insert_before_this(child)
    }

    fn elements(&self) -> Vec<UikitElem> {
        vec![self.el.clone()]
    }
}

impl<ChildState> Drop for ElementState<ChildState> {
    /// Safety net for an `ElementState` dropped without `unmount`
    /// (orphaned before mount, or a panic mid-`build`): free our
    /// store entry so it doesn't leak. `teardown` (→ `renderer::remove`)
    /// is idempotent, so this is a no-op after a normal `unmount`.
    fn drop(&mut self) {
        self.el.remove();
    }
}

// ---------------------------------------------------------------------
// view() — generic UIView container
// ---------------------------------------------------------------------

pub struct View<Children> {
    flex_direction: Option<FlexDirection>,
    gap: Option<f32>,
    align_content: Option<MaybeReactive<AlignContent>>,
    justify_items: Option<MaybeReactive<JustifyItems>>,
    aspect_ratio: Option<f32>,
    position_absolute: bool,
    /// Insets used when `position_absolute`. `None` = `auto`.
    inset_top: Option<f32>,
    inset_right: Option<f32>,
    inset_bottom: Option<f32>,
    inset_left: Option<f32>,
    children: Children,
    common: Common,
}

/// Generic flex container with no direction preset. Use
/// `<stack>` in the macro; alias of the legacy internal `view()`
/// builder.
pub fn stack() -> View<()> {
    view()
}

pub fn view() -> View<()> {
    View {
        flex_direction: None,
        gap: None,
        align_content: None,
        justify_items: None,
        aspect_ratio: None,
        position_absolute: false,
        inset_top: None,
        inset_right: None,
        inset_bottom: None,
        inset_left: None,
        children: (),
        common: Common::default(),
    }
}

impl<Ch> View<Ch> {
    pub fn flex_direction(mut self, dir: FlexDirection) -> Self {
        self.flex_direction = Some(dir);
        self
    }
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = Some(g);
        self
    }
    /// Cross-axis content distribution when wrap is enabled — same
    /// as CSS `align-content`. Only meaningful with wrapped flex lines.
    pub fn align_content<V: IntoMaybeReactive<AlignContent>>(
        mut self,
        v: V,
    ) -> Self {
        self.align_content = Some(v.into_maybe_reactive());
        self
    }
    /// Default cross-axis alignment for items within their flex line
    /// — same as CSS `justify-items`. Override per child with
    /// `align_self`.
    pub fn justify_items<V: IntoMaybeReactive<JustifyItems>>(
        mut self,
        v: V,
    ) -> Self {
        self.justify_items = Some(v.into_maybe_reactive());
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
    pub fn child<NewCh>(self, child: NewCh) -> View<(Ch, NewCh)> {
        View {
            flex_direction: self.flex_direction,
            gap: self.gap,
            align_content: self.align_content,
            justify_items: self.justify_items,
            aspect_ratio: self.aspect_ratio,
            position_absolute: self.position_absolute,
            inset_top: self.inset_top,
            inset_right: self.inset_right,
            inset_bottom: self.inset_bottom,
            inset_left: self.inset_left,
            children: (self.children, child),
            common: self.common,
        }
    }
}

// `<view on:click=...>` works via UITapGestureRecognizer (installed
// in `Element::on_click` when the underlying view isn't a UIControl).
// Plain UIView, UILabel, UIImageView etc. all route through that
// fallback.
impl<Ch> SupportsEvent<crate::event_ios::ClickEvent> for View<Ch> {}

impl_common!(View<Children>);

impl<Ch: Render<IosBackend>> Render<IosBackend> for View<Ch> {
    type State = ElementState<Ch::State>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_vstack();
        let mut effects = Vec::new();
        if let Some(dir) = self.flex_direction {
            set_flex_direction(el, dir);
        }
        if let Some(g) = self.gap {
            set_gap(el, g);
        }
        if let Some(v) = self.align_content {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_content(e, a))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.justify_items {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |j| set_justify_items(e, j))
            {
                effects.push(eff);
            }
        }
        if let Some(r) = self.aspect_ratio {
            set_aspect_ratio(el, r);
        }
        if self.position_absolute {
            crate::dom::layout::set_position(el, Position::Absolute);
            set_inset(
                el,
                self.inset_top,
                self.inset_right,
                self.inset_bottom,
                self.inset_left,
            );
        }
        self.common.finish(el, &mut effects);
        let child_state = self.children.build();
        ElementState {
            el,
            _effects: effects,
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
// grid() — Taffy CSS-Grid container (2-D layout)
// ---------------------------------------------------------------------

/// CSS-Grid container. Mirrors the cocoa `Grid` shape; differs only
/// in the UIView-specific chrome attrs (`corner_radius`, `border_*`).
/// The underlying Taffy node uses `Display::Grid`.
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

    children:         Children,
    common: Common,
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
        children: (),
        common: Common::default(),
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
            children: (self.children, child),
            common: self.common,
        }
    }
}

impl<Ch> SupportsEvent<crate::event_ios::ClickEvent> for Grid<Ch> {}

impl_common!(Grid<Children>);

impl<Ch: Render<IosBackend>> Render<IosBackend> for Grid<Ch> {
    type State = ElementState<Ch::State>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_grid();
        let mut effects = Vec::new();

        if let Some(c) = self.columns {
            set_grid_template_columns(el, c);
        }
        if let Some(r) = self.rows {
            set_grid_template_rows(el, r);
        }
        if let Some(c) = self.auto_columns {
            set_grid_auto_columns(el, c);
        }
        if let Some(r) = self.auto_rows {
            set_grid_auto_rows(el, r);
        }
        if let Some(v) = self.auto_flow {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |f| set_grid_auto_flow(e, f))
            {
                effects.push(eff);
            }
        }
        // Shorthand gap first; per-axis overrides win.
        if let Some(v) = self.gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_gap(e, g)) {
                effects.push(eff);
            }
        }
        if let Some(v) = self.column_gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_column_gap(e, g))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.row_gap {
            let e = el.clone();
            if let Some(eff) = install(v, move |g| set_row_gap(e, g)) {
                effects.push(eff);
            }
        }
        if let Some(v) = self.justify_items {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |j| set_justify_items(e, j))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align_items {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_items(e, a))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.justify_content {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |j| set_justify_content(e, j))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align_content {
            let e = el.clone();
            if let Some(eff) =
                install(v, move |a| set_align_content(e, a))
            {
                effects.push(eff);
            }
        }

        self.common.finish(el, &mut effects);
        let child_state = self.children.build();
        ElementState {
            el,
            _effects: effects,
            children: child_state,
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

impl<Children> AddAnyAttr<crate::IosBackend> for Grid<Children> {
    fn add_any_attr<__A>(mut self, attr: __A) -> Self
    where
        __A: ApplyAttr<crate::IosBackend>,
    {
        self.common.pending_spreads.push(Box::new(move |el: UikitElem| {
            attr.apply_to(el);
        }));
        self
    }
}

// ---------------------------------------------------------------------
// button() — UIButton with title + on:click
// ---------------------------------------------------------------------

pub struct Button {
    title: MaybeReactive<String>,
    enabled: Option<MaybeReactive<bool>>,
    sf_symbol: Option<MaybeReactive<String>>,
    common: Common,
}

pub fn button() -> Button {
    Button {
        title: MaybeReactive::Static(String::new()),
        enabled: None,
        sf_symbol: None,
        common: Common::default(),
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
    /// Set an SF Symbol as the button's image. Pair with `text_color=`
    /// to tint template symbols. iOS 13+; older systems are no-op.
    pub fn sf_symbol<V: IntoMaybeReactive<String>>(mut self, name: V) -> Self {
        self.sf_symbol = Some(name.into_maybe_reactive());
        self
    }
}

impl SupportsEvent<crate::event_ios::ClickEvent> for Button {}

impl_common!(Button: text);

impl Render<IosBackend> for Button {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_button().0;
        let mut effects = Vec::new();

        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_title(&t);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for_en = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for_en.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(name) = self.sf_symbol {
            let el_for_sym = el.clone();
            if let Some(eff) = install(name, move |n| {
                el_for_sym.set_sf_symbol(&n);
            }) {
                effects.push(eff);
            }
        }


        self.common.finish(el, &mut effects);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// label() — UILabel
// ---------------------------------------------------------------------

pub struct Label {
    text_value: MaybeReactive<String>,
    lines: Option<MaybeReactive<i32>>,
    pending_bind_text:
        Option<Box<dyn Fn() -> String + Send + 'static>>,
    common: Common,
}

pub fn label() -> Label {
    Label {
        text_value: MaybeReactive::Static(String::new()),
        lines: None,
        pending_bind_text: None,
        common: Common::default(),
    }
}

impl Label {
    pub fn text<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.text_value = v.into_maybe_reactive();
        self
    }
    /// `<label>"X"</label>` or `<label>{closure}</label>`.
    pub fn child<V: IntoMaybeReactive<String>>(self, value: V) -> Self {
        self.text(value)
    }
    /// Maximum line count — `0` means unlimited (wrapping). Defaults
    /// to UIKit's single line with tail truncation.
    pub fn lines<V: IntoMaybeReactive<i32>>(mut self, v: V) -> Self {
        self.lines = Some(v.into_maybe_reactive());
        self
    }
    /// Internal: stash a `bind:value=...` for installation in `build`.
    pub(crate) fn set_pending_bind_text(
        &mut self,
        getter: Box<dyn Fn() -> String + Send + 'static>,
    ) {
        self.pending_bind_text = Some(getter);
    }
}

impl SupportsEvent<crate::event_ios::ClickEvent> for Label {}

impl_common!(Label: text);

impl Render<IosBackend> for Label {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_label().0;
        let mut effects = Vec::new();

        // bind:value getter wins over .text(...) — same as cocoa.
        let text = match self.pending_bind_text {
            Some(getter) => MaybeReactive::Reactive(getter),
            None => self.text_value,
        };
        let el_for_text = el.clone();
        if let Some(eff) = install(text, move |s| {
            el_for_text.set_title(&s);
        }) {
            effects.push(eff);
        }

        if let Some(lines) = self.lines {
            let el_for = el.clone();
            if let Some(eff) = install(lines, move |n| {
                el_for.set_label_lines(n);
            }) {
                effects.push(eff);
            }
        }

        self.common.finish(el, &mut effects);

        ElementState {
            el,
            _effects: effects,
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
    common: Common,
}

pub fn text_field() -> TextField {
    TextField {
        value: MaybeReactive::Static(String::new()),
        placeholder: None,
        enabled: None,
        secure: false,
        pending_bind: None,
        common: Common::default(),
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
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::ios::bind::BoundValue,
    ) {
        self.pending_bind = Some(bound);
    }
}

// Text fields fire on every keystroke (`input`) and on commit
// (`change` — return key / focus loss). Click is deliberately not
// supported — clicking inside the field places the caret. Focus/
// blur are UIControl `editingDidBegin` / `editingDidEnd`.
impl SupportsEvent<crate::event_ios::InputEvent> for TextField {}
impl SupportsEvent<crate::event_ios::ChangeEvent> for TextField {}
impl SupportsEvent<crate::event_ios::CommitEvent> for TextField {}
impl SupportsEvent<crate::event_ios::FocusEvent> for TextField {}
impl SupportsEvent<crate::event_ios::BlurEvent> for TextField {}

impl_common!(TextField: text);

impl Render<IosBackend> for TextField {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = if self.secure {
            UikitElem::create_secure_text_field().0
        } else {
            UikitElem::create_text_field().0
        };
        let mut effects = Vec::new();

        if let Some(p) = self.placeholder {
            el.set_placeholder(&p);
        }

        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        // One-way `.value(...)` — applied even when bind:value is
        // present; the bind effect installs second so it wins on
        // subsequent ticks.
        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_value(&v);
        }) {
            effects.push(eff);
        }

        // Two-way `bind:value=signal`.
        if let Some(bound) = self.pending_bind {
            let eff = crate::ios::bind::install_text_field_value_bind(el, bound);
            effects.push(eff);
        }



        self.common.finish(el, &mut effects);


        ElementState {
            el,
            _effects: effects,
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
    common: Common,
}

/// Portable name for the boolean toggle. On iOS this is the
/// same widget as `<switch>` (UISwitch); on Cocoa/GTK it maps
/// to a checkbox.
pub fn toggle() -> Switch {
    switch_()
}

pub fn switch_() -> Switch {
    Switch {
        checked: MaybeReactive::Static(false),
        enabled: None,
        pending_bind_checked: None,
        common: Common::default(),
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
    pub(crate) fn set_pending_bind_checked(
        &mut self,
        bound: crate::ios::bind::BoundChecked,
    ) {
        self.pending_bind_checked = Some(bound);
    }
}

impl SupportsEvent<crate::event_ios::ClickEvent> for Switch {}

impl_common!(Switch);

impl Render<IosBackend> for Switch {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_switch().0;
        let mut effects = Vec::new();

        // One-way `.checked(...)`.
        let el_for_checked = el.clone();
        if let Some(eff) = install(self.checked, move |b| {
            el_for_checked.set_checked(b);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        // Two-way `bind:checked=signal`.
        if let Some(bound) = self.pending_bind_checked {
            let eff =
                crate::ios::bind::install_switch_checked_bind(el, bound);
            effects.push(eff);
        }



        self.common.finish(el, &mut effects);


        ElementState {
            el,
            _effects: effects,
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
    common: Common,
}

pub fn slider() -> Slider {
    Slider {
        value: MaybeReactive::Static(0.0),
        min_value: 0.0,
        max_value: 1.0,
        enabled: None,
        pending_bind: None,
        common: Common::default(),
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
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::ios::bind::BoundFloat,
    ) {
        self.pending_bind = Some(bound);
    }
}

impl SupportsEvent<crate::event_ios::ChangeEvent> for Slider {}

impl_common!(Slider);

impl Render<IosBackend> for Slider {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_slider().0;
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
                el_for.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind {
            let eff =
                crate::ios::bind::install_slider_value_bind(el, bound);
            effects.push(eff);
        }



        self.common.finish(el, &mut effects);


        ElementState {
            el,
            _effects: effects,
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
    common: Common,
}

pub fn stepper() -> Stepper {
    Stepper {
        value: MaybeReactive::Static(0.0),
        min_value: 0.0,
        max_value: 100.0,
        increment: 1.0,
        enabled: None,
        pending_bind: None,
        common: Common::default(),
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
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::ios::bind::BoundFloat,
    ) {
        self.pending_bind = Some(bound);
    }
}

impl SupportsEvent<crate::event_ios::ChangeEvent> for Stepper {}

impl_common!(Stepper);

impl Render<IosBackend> for Stepper {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_stepper().0;
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
                el_for.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind {
            let eff =
                crate::ios::bind::install_stepper_value_bind(el, bound);
            effects.push(eff);
        }



        self.common.finish(el, &mut effects);


        ElementState {
            el,
            _effects: effects,
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
    common: Common,
}

pub fn progress_indicator() -> ProgressIndicator {
    ProgressIndicator {
        value: MaybeReactive::Static(0.0),
        common: Common::default(),
    }
}

impl ProgressIndicator {
    pub fn value<V: IntoMaybeReactive<f64>>(mut self, v: V) -> Self {
        self.value = v.into_maybe_reactive();
        self
    }
}

impl_common!(ProgressIndicator);

impl Render<IosBackend> for ProgressIndicator {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_progress_indicator().0;
        let mut effects = Vec::new();

        let el_for = el.clone();
        if let Some(eff) =
            install(self.value, move |v| el_for.set_progress_value(v))
        {
            effects.push(eff);
        }

        self.common.finish(el, &mut effects);


        ElementState {
            el,
            _effects: effects,
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
    bytes: Option<MaybeReactive<Option<Vec<u8>>>>,
    sf_symbol: Option<MaybeReactive<String>>,
    tint: Option<MaybeReactive<Color>>,
    common: Common,
}

pub fn image_view() -> ImageView {
    ImageView {
        source: MaybeReactive::Static(String::new()),
        bytes: None,
        sf_symbol: None,
        tint: None,
        common: Common::default(),
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
    /// In-memory image bytes (PNG/JPEG/GIF/TIFF/HEIC, auto-detected
    /// by UIImage). `None` clears. Reactive. Use this for HTTP-
    /// fetched images — see the Working with Async docs for the
    /// bridging pattern.
    pub fn bytes<V: IntoMaybeReactive<Option<Vec<u8>>>>(mut self, v: V) -> Self {
        self.bytes = Some(v.into_maybe_reactive());
        self
    }
    /// Render an SF Symbol as the image. iOS 13+; older systems
    /// are no-op. Pair with `.tint(...)` to colour a template
    /// symbol.
    pub fn sf_symbol<V: IntoMaybeReactive<String>>(mut self, name: V) -> Self {
        self.sf_symbol = Some(name.into_maybe_reactive());
        self
    }
    /// Tint the image. Most useful with SF Symbols / template
    /// images; UIKit propagates the tint through automatically.
    pub fn tint<V: IntoMaybeReactive<Color>>(mut self, c: V) -> Self {
        self.tint = Some(c.into_maybe_reactive());
        self
    }
}

// `<image_view on:click=...>` lands on a UITapGestureRecognizer via
// the on_click → on_tap_gesture fallback.
impl SupportsEvent<crate::event_ios::ClickEvent> for ImageView {}

impl_common!(ImageView);

impl Render<IosBackend> for ImageView {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_image_view().0;
        let mut effects = Vec::new();

        let el_for = el.clone();
        if let Some(eff) =
            install(self.source, move |s| el_for.set_image_view_path(&s))
        {
            effects.push(eff);
        }

        if let Some(b) = self.bytes {
            let el_for_b = el.clone();
            if let Some(eff) = install(b, move |bytes| {
                el_for_b.set_image_view_bytes(bytes.as_deref())
            }) {
                effects.push(eff);
            }
        }

        if let Some(name) = self.sf_symbol {
            let el_for_sym = el.clone();
            if let Some(eff) = install(name, move |n| {
                el_for_sym.set_sf_symbol(&n);
            }) {
                effects.push(eff);
            }
        }

        if let Some(t) = self.tint {
            let el_for_tint = el.clone();
            if let Some(eff) = install(t, move |c| {
                el_for_tint.set_tint(Some(c));
            }) {
                effects.push(eff);
            }
        }



        self.common.finish(el, &mut effects);


        ElementState {
            el,
            _effects: effects,
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
    common: Common,
}

pub fn segmented_control() -> SegmentedControl {
    SegmentedControl {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        enabled: None,
        pending_bind_selection: None,
        common: Common::default(),
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
    pub(crate) fn set_pending_bind_selection(
        &mut self,
        bound: crate::ios::bind::BoundIndex,
    ) {
        self.pending_bind_selection = Some(bound);
    }
}

impl SupportsEvent<crate::event_ios::ChangeEvent> for SegmentedControl {}

impl_common!(SegmentedControl);

impl Render<IosBackend> for SegmentedControl {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_segmented_control().0;
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
                el_for.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind_selection {
            let eff = crate::ios::bind::install_segmented_selection_bind(el, bound);
            effects.push(eff);
        }



        self.common.finish(el, &mut effects);


        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// tab_bar() — UITabBar (the bar view, not UITabBarController).
// `.items([(title, sf_symbol)])`, `bind:selection=usize_signal`.
// Content switching is the app's job (reactive `hidden` / `Show`).
//
// Do NOT force a `height` on this element: the bar's own
// `sizeThatFits` drives the leaf measure, and modern UITabBar item
// layouts need their natural height — a forced smaller height makes
// titles overlap icons.
// ---------------------------------------------------------------------

pub struct TabBar {
    items: Vec<(String, String)>,
    selection: MaybeReactive<usize>,
    pending_bind_selection: Option<crate::ios::bind::BoundIndex>,
    common: Common,
}

pub fn tab_bar() -> TabBar {
    TabBar {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        pending_bind_selection: None,
        common: Common::default(),
    }
}

impl TabBar {
    pub fn items<I, S1, S2>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = (S1, S2)>,
        S1: Into<String>,
        S2: Into<String>,
    {
        self.items = items
            .into_iter()
            .map(|(title, symbol)| (title.into(), symbol.into()))
            .collect();
        self
    }
    pub fn selection<V: IntoMaybeReactive<usize>>(mut self, v: V) -> Self {
        self.selection = v.into_maybe_reactive();
        self
    }
    pub(crate) fn set_pending_bind_selection(
        &mut self,
        bound: crate::ios::bind::BoundIndex,
    ) {
        self.pending_bind_selection = Some(bound);
    }
}

// No `SupportsEvent<ChangeEvent>`: UITabBar is a UIView, not a
// UIControl, so target/action (`on_value_change`) never fires on it.
// Selection changes are delivered through the tab-bar delegate, wired
// by `bind:selection`. Withholding the impl makes `<tab_bar on:change>`
// a compile error that steers callers to `bind:selection` instead of
// silently dropping the handler.

impl_common!(TabBar);

impl Render<IosBackend> for TabBar {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_tab_bar().0;
        let mut effects = Vec::new();

        el.set_tab_items(&self.items);

        let el_for = el.clone();
        if let Some(eff) = install(self.selection, move |i| {
            el_for.set_tab_selection(i);
        }) {
            effects.push(eff);
        }

        if let Some(bound) = self.pending_bind_selection {
            let eff = crate::ios::bind::install_tab_selection_bind(el, bound);
            effects.push(eff);
        }

        self.common.finish(el, &mut effects);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// blur_view() — UIVisualEffectView + UIBlurEffect. Non-interactive
// underlay (touches pass through); layer behind bar content with
// `position_absolute`. Not a container — children are unsupported.
// ---------------------------------------------------------------------

pub struct BlurView {
    style: Option<MaybeReactive<crate::dom::objc_enums::BlurStyle>>,
    common: Common,
}

pub fn blur_view() -> BlurView {
    BlurView {
        style: None,
        common: Common::default(),
    }
}

impl BlurView {
    pub fn style<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<crate::dom::objc_enums::BlurStyle>,
    {
        self.style = Some(v.into_maybe_reactive());
        self
    }
}

impl_common!(BlurView);

impl Render<IosBackend> for BlurView {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_blur_view(
            crate::dom::objc_enums::BlurStyle::SYSTEM_MATERIAL,
        )
        .0;
        let mut effects = Vec::new();

        if let Some(style) = self.style {
            let el_for = el.clone();
            if let Some(eff) = install(style, move |s| {
                el_for.set_blur_style(s);
            }) {
                effects.push(eff);
            }
        }

        self.common.finish(el, &mut effects);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// pop_up_button() — UIButton + UIMenu (iOS 14+).
// API mirrors the Cocoa `pop_up_button()` for portability:
// `.items(...)`, `.selection(...)`, `bind:value=index_signal`.
// ---------------------------------------------------------------------

pub struct PopUpButton {
    items: Vec<String>,
    selection: MaybeReactive<usize>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_selection: Option<crate::ios::bind::BoundIndex>,
    common: Common,
}

pub fn pop_up_button() -> PopUpButton {
    PopUpButton {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        enabled: None,
        pending_bind_selection: None,
        common: Common::default(),
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
    pub fn selection<V: IntoMaybeReactive<usize>>(mut self, v: V) -> Self {
        self.selection = v.into_maybe_reactive();
        self
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub(crate) fn set_pending_bind_selection(
        &mut self,
        bound: crate::ios::bind::BoundIndex,
    ) {
        self.pending_bind_selection = Some(bound);
    }
}

impl SupportsEvent<crate::event_ios::ChangeEvent> for PopUpButton {}

impl_common!(PopUpButton);

impl Render<IosBackend> for PopUpButton {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_pop_up_button().0;
        let mut effects = Vec::new();

        // The change callback: invoked when a menu item is picked.
        // `bind:value` threads a real setter through; otherwise it's
        // a no-op (the user-visible title still updates because we
        // refresh it from the reactive `selection` below, but
        // there's no signal to write back to).
        let mut on_change: Box<dyn FnMut(usize) + 'static> =
            if let Some(bound) = self.pending_bind_selection {
                let mut setter = bound.setter;
                Box::new(move |i: usize| setter(i))
            } else {
                Box::new(|_| {})
            };

        // Build the menu once. Items are static-after-build for now
        // (matches the Cocoa popup's behaviour).
        let initial = match &self.selection {
            MaybeReactive::Static(i) => *i,
            MaybeReactive::Reactive(f) => f(),
        };
        el.set_popup_items(&self.items, initial, move |i| {
            on_change(i);
        });

        // Reactive selection — programmatic changes to the bound
        // signal update the displayed title.
        let items_clone = self.items.clone();
        let el_for = el.clone();
        if let Some(eff) = install(self.selection, move |i| {
            el_for.set_popup_selection(&items_clone, i);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }


        self.common.finish(el, &mut effects);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// color_well() — UIColorWell (iOS 14+). Inline swatch that manages
// its own UIColorPickerViewController presentation on tap.
// ---------------------------------------------------------------------

pub struct ColorWell {
    value: MaybeReactive<Color>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_value: Option<crate::ios::bind::BoundColor>,
    common: Common,
}

pub fn color_well() -> ColorWell {
    ColorWell {
        value: MaybeReactive::Static(Color::BLACK),
        enabled: None,
        pending_bind_value: None,
        common: Common::default(),
    }
}

impl ColorWell {
    pub fn value<V: IntoMaybeReactive<Color>>(mut self, v: V) -> Self {
        self.value = v.into_maybe_reactive();
        self
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::ios::bind::BoundColor,
    ) {
        self.pending_bind_value = Some(bound);
    }
}

impl SupportsEvent<crate::event_ios::ChangeEvent> for ColorWell {}

impl_common!(ColorWell);

impl Render<IosBackend> for ColorWell {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_color_well().0;
        let mut effects = Vec::new();

        let el_for = el.clone();
        if let Some(eff) = install(self.value, move |c| {
            el_for.set_color_well_value(c);
        }) {
            effects.push(eff);
        }

        if let Some(enabled) = self.enabled {
            let el_for = el.clone();
            if let Some(eff) = install(enabled, move |b| {
                el_for.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind_value {
            let eff = crate::ios::bind::install_color_well_value_bind(el, bound);
            effects.push(eff);
        }


        self.common.finish(el, &mut effects);

        ElementState {
            el,
            _effects: effects,
            children: (),
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}

// ---------------------------------------------------------------------
// date_picker() — UIDatePicker
// ---------------------------------------------------------------------

pub struct DatePicker {
    value: MaybeReactive<Date>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::ios::bind::BoundDate>,
    style: Option<MaybeReactive<DatePickerStyle>>,
    min_date: Option<MaybeReactive<Date>>,
    max_date: Option<MaybeReactive<Date>>,
    common: Common,
}

pub fn date_picker() -> DatePicker {
    DatePicker {
        value: MaybeReactive::Static(Date::now()),
        enabled: None,
        pending_bind: None,
        style: None,
        min_date: None,
        max_date: None,
        common: Common::default(),
    }
}

impl DatePicker {
    pub fn value<V: IntoMaybeReactive<Date>>(mut self, v: V) -> Self {
        self.value = v.into_maybe_reactive();
        self
    }
    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }
    pub(crate) fn set_pending_bind_date(
        &mut self,
        bound: crate::ios::bind::BoundDate,
    ) {
        self.pending_bind = Some(bound);
    }
    /// Visual style: `Wheels`, `Compact` (default), `Inline`,
    /// `Automatic`. See `DatePickerStyle`.
    pub fn style<V>(mut self, s: V) -> Self
    where
        V: IntoMaybeReactive<DatePickerStyle>,
    {
        self.style = Some(s.into_maybe_reactive());
        self
    }
    pub fn min_date<V: IntoMaybeReactive<Date>>(mut self, d: V) -> Self {
        self.min_date = Some(d.into_maybe_reactive());
        self
    }
    pub fn max_date<V: IntoMaybeReactive<Date>>(mut self, d: V) -> Self {
        self.max_date = Some(d.into_maybe_reactive());
        self
    }
}

impl SupportsEvent<crate::event_ios::ChangeEvent> for DatePicker {}

impl_common!(DatePicker);

impl Render<IosBackend> for DatePicker {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_date_picker().0;
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
                el_for.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind {
            let eff = crate::ios::bind::install_date_picker_bind(el, bound);
            effects.push(eff);
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

        self.common.finish(el, &mut effects);


        ElementState {
            el,
            _effects: effects,
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
    children: Children,
    has_horizontal_scroller: Option<MaybeReactive<bool>>,
    has_vertical_scroller: Option<MaybeReactive<bool>>,
    common: Common,
}

pub fn scroll_view() -> ScrollView<()> {
    ScrollView {
        children: (),
        has_horizontal_scroller: None,
        has_vertical_scroller: None,
        common: Common::default(),
    }
}

impl<Ch> ScrollView<Ch> {
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
            children: (self.children, child),
            has_horizontal_scroller: self.has_horizontal_scroller,
            has_vertical_scroller: self.has_vertical_scroller,
            common: self.common,
        }
    }
}

impl_common!(ScrollView<Children>);

impl<Ch: Render<IosBackend>> Render<IosBackend> for ScrollView<Ch> {
    type State = ElementState<Ch::State>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_scroll_view().0;
        let mut effects = Vec::new();

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

        self.common.finish(el, &mut effects);

        let child_state = self.children.build();

        ElementState {
            el,
            _effects: effects,
            children: child_state,
        }
    }
    fn rebuild(self, _state: &mut Self::State) {}
}


// ---------------------------------------------------------------------
// table_view() — UITableView (.plain style, sticky section headers)
//
// A LEAF element: content comes from the reactive `sections`
// snapshot (Vec<TableSection>, each section carrying TableRow view-
// builder closures), served to UIKit by the driver in `dom::table`.
// Cells rebuild their leptos content on every dequeue; see the
// module docs on `dom::table` for the full model.
//
// Like `<scroll_view>`, the element must be bounded by its parent
// (typically `flex_grow=1.0` in a bounded stack) — UITableView owns
// everything inside its frame.
// ---------------------------------------------------------------------

pub struct TableView {
    sections: MaybeReactive<Vec<crate::dom::TableSection>>,
    header: Option<crate::dom::table::HeaderBuild>,
    row_height: Option<f64>,
    header_height: Option<f64>,
    separators: Option<MaybeReactive<bool>>,
    content_inset: Option<leptos_native::renderer::attrs::Edges>,
    common: Common,
}

pub fn table_view() -> TableView {
    TableView {
        sections: MaybeReactive::Static(Vec::new()),
        header: None,
        row_height: None,
        header_height: None,
        separators: None,
        content_inset: None,
        common: Common::default(),
    }
}

impl TableView {
    /// The table's content, as one snapshot: sections in order, each
    /// with its header title and row view-builders. Reactive — pass a
    /// closure reading signals and the table reloads when they change.
    pub fn sections<V: IntoMaybeReactive<Vec<crate::dom::TableSection>>>(
        mut self,
        v: V,
    ) -> Self {
        self.sections = v.into_maybe_reactive();
        self
    }

    /// Custom section-header view builder (receives the section
    /// title). Without it, UIKit's default plain-style header renders
    /// the title. Pair with `header_height` — the leptos-hosted
    /// header is laid out against the height UIKit grants it.
    pub fn header<F, V>(mut self, f: F) -> Self
    where
        F: Fn(String) -> V + Send + Sync + 'static,
        V: Render<IosBackend>,
        V::State: Mountable<IosBackend> + 'static,
    {
        self.header = Some(crate::dom::table::make_header_build(f));
        self
    }

    /// Fixed row height in points. Strongly recommended: leptos-
    /// hosted cells have no Auto Layout constraints for UIKit's
    /// self-sizing to measure, so without a fixed height rows
    /// collapse.
    pub fn row_height(mut self, h: f64) -> Self {
        self.row_height = Some(h);
        self
    }

    /// Fixed section-header height in points (used with `header`).
    pub fn header_height(mut self, h: f64) -> Self {
        self.header_height = Some(h);
        self
    }

    /// Show the system row separators (default: off).
    pub fn separators<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.separators = Some(v.into_maybe_reactive());
        self
    }

    /// `UIScrollView.contentInset` — clears overlaid bars while
    /// sticky headers pin below them (a padding attr would instead
    /// shrink the viewport and pin headers at the padding edge).
    pub fn content_inset(
        mut self,
        e: leptos_native::renderer::attrs::Edges,
    ) -> Self {
        self.content_inset = Some(e);
        self
    }
}

impl_common!(TableView);

impl Render<IosBackend> for TableView {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_table_view().0;
        let mut effects = Vec::new();

        let owner = reactive_graph::owner::Owner::current().expect(
            "table_view must be built inside a reactive Owner \
             (mount / navigation::push provide one)",
        );
        let model =
            crate::dom::table::TableModel::new(self.header, self.header_height, owner);
        crate::dom::table::install_table_driver(el, model.clone());

        if let Some(h) = self.row_height {
            el.set_table_row_height(h);
        }
        if let Some(e) = self.content_inset {
            el.set_scroll_content_inset(
                e.top as f64,
                e.left as f64,
                e.bottom as f64,
                e.right as f64,
            );
        }
        if let Some(s) = self.separators {
            let el_for = el.clone();
            if let Some(eff) =
                install(s, move |v| el_for.set_table_separators(v))
            {
                effects.push(eff);
            }
        }

        let model_for = model.clone();
        if let Some(eff) = install(self.sections, move |secs| {
            crate::dom::table::set_table_sections(el, &model_for, secs);
        }) {
            effects.push(eff);
        }

        self.common.finish(el, &mut effects);

        ElementState {
            el,
            _effects: effects,
            children: (),
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
    common: Common,
}

pub fn text_view() -> TextView {
    TextView {
        value: MaybeReactive::Static(String::new()),
        enabled: None,
        pending_bind: None,
        common: Common::default(),
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
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::ios::bind::BoundValue,
    ) {
        self.pending_bind = Some(bound);
    }
}

impl_common!(TextView: text);

impl Render<IosBackend> for TextView {
    type State = ElementState<()>;
    fn build(self) -> Self::State {
        let el = UikitElem::create_text_view().0;
        let mut effects = Vec::new();

        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_value(&v);
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
                crate::ios::bind::install_text_view_value_bind(el, bound);
            effects.push(eff);
        }

        self.common.finish(el, &mut effects);


        ElementState {
            el,
            _effects: effects,
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
            impl leptos_native::renderer::view::AddAnyAttr<crate::IosBackend> for $builder {
                fn add_any_attr<__A>(mut self, attr: __A) -> Self
                where
                    __A: leptos_native::renderer::view::ApplyAttr<crate::IosBackend>,
                {
                    self.common.pending_spreads.push(Box::new(move |el: UikitElem| {
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
    ImageView, SegmentedControl, DatePicker, TabBar, BlurView, TableView,
);

// ProgressIndicator + TextView don't carry a handlers/pending_spreads
// Vec, so attaching a spread here has no install path. Panic rather
// than silently drop. (UITextView and UIProgressView CAN take tap
// gesture recognizers in principle, but adding the storage is
// scope-creep for this commit.)
impl AddAnyAttr<crate::IosBackend> for ProgressIndicator {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where __A: ApplyAttr<crate::IosBackend> {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on ProgressIndicator — \
             UIProgressView doesn't carry handler/spread storage in \
             this fork. Attach the handler to a sibling control instead."
        )
    }
}

impl AddAnyAttr<crate::IosBackend> for TextView {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where __A: ApplyAttr<crate::IosBackend> {
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
impl<Children> AddAnyAttr<crate::IosBackend> for View<Children> {
    fn add_any_attr<__A>(mut self, attr: __A) -> Self
    where
        __A: ApplyAttr<crate::IosBackend>,
    {
        self.common.pending_spreads.push(Box::new(move |el: UikitElem| {
            attr.apply_to(el);
        }));
        self
    }
}

// ScrollView lacks pending_spreads in its struct — same treatment as
// ProgressIndicator/TextView. UIScrollView could host a tap gesture
// recognizer in principle but isn't wired here.
impl<Children> AddAnyAttr<crate::IosBackend> for ScrollView<Children> {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where
        __A: ApplyAttr<crate::IosBackend>,
    {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on ScrollView — no spread \
             storage on ScrollView<Children>. Attach to inner content \
             or wait for gesture-recognizer support."
        )
    }
}
