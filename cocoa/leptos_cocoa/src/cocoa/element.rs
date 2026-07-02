//! Element builder types: `view()`, `button()`, `label()`, etc.
//!
//! Each builder returns a struct that implements [`Render`] from
//! tachys' view core. Building emits a [`CocoaElem`] (or
//! similar leaf), wires attributes (with reactive effects for
//! signal-driven values), recursively builds children, and mounts
//! them.

use super::attr::{install, IntoMaybeReactive, MaybeReactive};
use crate::dom::{event, layout::*, CocoaElem, CocoaMakeView, CocoaNodeExt, Color, Date, DatePickerStyle, LineBreak, SegmentStyle, TextAlignment};
use crate::CocoaBackend;
use reactive_graph::effect::RenderEffect;
use leptos_native::node_ref::NodeRef;
use leptos_native::prelude::AddAnyAttr;
use leptos_native::renderer::attrs::{
    DecorationAttrs, LayoutAttrs, TextAttrs, UniversalAttrs, WithLayout,
    WithUniversal,
};
use leptos_native::renderer::view::{ApplyAttr, Mountable, Render};

// `apply_universal` and `apply_layout` live in `renderer`. The
// The `Backend` native-setter hooks for `CocoaBackend` live
// in `cocoa_dom` (orphan rule).
use leptos_native::renderer::{apply_decoration, apply_universal, directive, apply_layout};

/// Cocoa's text-attr struct alias — `TextAttrs` with cocoa's `Color`
/// and `NSTextAlignment`.
pub type CocoaText = TextAttrs<Color, TextAlignment>;

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
    fn alignment<V: IntoMaybeReactive<TextAlignment>>(
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

/// Cocoa's decoration-attr struct alias — `DecorationAttrs<Color>`.
pub type CocoaDecoration = DecorationAttrs<Color>;

/// Port-local `WithDecoration` shadow. Same shape as the generic
/// `renderer::attrs::WithDecoration<C>`, but pins `C = Color` and
/// uses the port-local [`IntoMaybeReactive`] so chainable setters
/// accept either bare `Color` or `Fn() -> Color` closures.
pub trait WithDecoration: Sized {
    fn decoration_mut(&mut self) -> &mut CocoaDecoration;

    /// Layer-backed background fill.
    fn background_color<V: IntoMaybeReactive<Color>>(mut self, c: V) -> Self {
        self.decoration_mut().background_color = Some(c.into_maybe_reactive());
        self
    }

    /// Round the corners. Pair with `overflow=Overflow::Clip` (or
    /// `Hidden`) on the same element to also clip children to the
    /// rounded shape — `corner_radius` alone rounds the fill, not
    /// the children.
    fn corner_radius<V: IntoMaybeReactive<f32>>(mut self, r: V) -> Self {
        self.decoration_mut().corner_radius = Some(r.into_maybe_reactive());
        self
    }

    /// Border width in points. `0.0` disables. Pair with
    /// [`Self::border_color`] for non-default (opaque black) borders.
    fn border_width<V: IntoMaybeReactive<f32>>(mut self, w: V) -> Self {
        self.decoration_mut().border_width = Some(w.into_maybe_reactive());
        self
    }

    /// Border color. Only visible when `border_width > 0`.
    fn border_color<V: IntoMaybeReactive<Color>>(mut self, c: V) -> Self {
        self.decoration_mut().border_color = Some(c.into_maybe_reactive());
        self
    }
}

/// Apply [`CocoaText`] (text_color, alignment, font_size) to the live
/// NSView. Each leaf decides whether to invoke this — NSButton
/// skips `text_color` (uses `attributedTitle` if styling is needed).
fn apply_text(el: CocoaElem, attrs: CocoaText) -> Vec<RenderEffect<()>> {
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

/// Apply the four "always there" attribute structs every builder
/// owns: `decoration`, `universal`, optional `text` (for leaf
/// controls), and `layout`. Ordering is significant:
///
/// 1. `decoration` — background_color / corner_radius / border —
///    pure visual layer, no layout impact.
/// 2. `universal` — alpha + tooltip. Also no layout.
/// 3. `text` (when present) — text_color / alignment / font_size on
///    controls that render text. Some controls (e.g. Stepper) layer
///    extra effects (`bold`) after this; those stay in the builder.
/// 4. `layout` LAST because `hidden=Display::None` lives in
///    `LayoutAttrs` and the Taffy display flip needs to happen after
///    the visual chrome is in place.
///
/// The macro consolidates the 3-or-4-line apply-cascade tail of
/// every `Render::build` into one call.
fn apply_common(
    el: CocoaElem,
    decoration: CocoaDecoration,
    universal: UniversalAttrs,
    text: Option<CocoaText>,
    layout: LayoutAttrs,
) -> Vec<RenderEffect<()>> {
    let mut effects = apply_decoration(el, decoration);
    effects.extend(apply_universal(el, universal));
    if let Some(text) = text {
        effects.extend(apply_text(el, text));
    }
    effects.extend(apply_layout(el, layout));
    effects
}

/// `wire_attr!(effects, el, opt_value, setter)` — DRY out the
/// `if let Some(v) = self.foo { ... install ... effects.push ... }`
/// pattern that every `Render::build` repeats per attribute. The
/// `setter` is a free fn that takes `(&Node, Value)`.
///
/// Named to *not* collide with the `install` free function (from
/// `renderer::attrs`) that the macro itself invokes.
macro_rules! wire_attr {
    ($effects:expr, $el:expr, $opt:expr, $setter:expr) => {
        if let Some(__v) = $opt {
            let __e = $el.clone();
            let __setter = $setter;
            if let Some(__eff) = install(__v, move |__val| __setter(__e, __val))
            {
                $effects.push(__eff);
            }
        }
    };
}

// ---------------------------------------------------------------------
// Common builder state — the attrs/handlers every builder carries.
// ---------------------------------------------------------------------

/// The builder state shared by every element builder: event handlers,
/// `node_ref`, `use:` directives (also the install path for spread
/// attrs), and the four chainable attr structs. Builders embed one of
/// these as `common` and get the accessor-trait impls + `on` /
/// `node_ref` / `directive` methods from [`impl_common!`].
#[derive(Default)]
pub struct Common {
    handlers: Vec<crate::event_macos::PendingHandler>,
    node_ref: Option<NodeRef<CocoaElem>>,
    directives: Vec<Box<dyn FnOnce(CocoaElem) + Send + 'static>>,
    universal: UniversalAttrs,
    layout: LayoutAttrs,
    decoration: CocoaDecoration,
    /// All-`None` (installs nothing) on builders that don't expose
    /// [`WithText`] — carrying it unconditionally is cheaper than a
    /// per-builder `Option`.
    text: CocoaText,
}

impl Common {
    /// The shared tail of every `Render::build`: install event
    /// handlers, apply the attr structs (see [`apply_common`] for the
    /// ordering contract), load the `node_ref`, run `use:` directives.
    /// Call AFTER the builder's control-specific wiring. (Label rolls
    /// its own tail — it routes Click through `on_action` and layers
    /// `bold` between text and layout.)
    fn finish(self, el: CocoaElem, effects: &mut Vec<RenderEffect<()>>) {
        for h in self.handlers {
            h.apply_to(el);
        }
        effects.extend(apply_common(
            el,
            self.decoration,
            self.universal,
            Some(self.text),
            self.layout,
        ));
        if let Some(r) = self.node_ref {
            r.load(el);
        }
        crate::cocoa::directives::run_all(self.directives, el);
    }
}

/// Generate the boilerplate every builder repeats over its `common`
/// field: the `WithLayout` / `WithUniversal` / `WithDecoration`
/// accessor impls plus the `on` / `node_ref` / `directive` methods.
/// Add `: text` for builders that render text (also impls
/// [`WithText`]). Handles both plain and single-type-param builders.
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
            fn decoration_mut(&mut self) -> &mut CocoaDecoration {
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
                Self: crate::event_macos::SupportsEvent<E>,
                E: crate::event_macos::EventDescriptor,
                F: FnMut(E::EventType) + Send + 'static,
            {
                self.common.handlers.push(E::into_pending(handler));
                self
            }

            /// Capture the built element in a `NodeRef` for imperative
            /// access (focus, measurement) after mount.
            pub fn node_ref(mut self, r: NodeRef<CocoaElem>) -> Self {
                self.common.node_ref = Some(r);
                self
            }

            /// `use:directive=param` — run `handler(el, param)` once
            /// after the element is built. Inherent method (Rust
            /// resolves it before `DirectiveAttribute::directive`).
            pub fn directive<D, T, P>(mut self, handler: D, param: P) -> Self
            where
                D: directive::IntoDirective<CocoaElem, T, P> + Send + 'static,
                P: Send + 'static,
                T: 'static,
            {
                self.common
                    .directives
                    .push(crate::cocoa::directives::pack(handler, param));
                self
            }
        }
    };
    ($ty:ident $(<$g:ident>)? : text) => {
        impl_common!($ty $(<$g>)?);
        impl $(<$g>)? WithText for $ty $(<$g>)? {
            fn text_attrs_mut(&mut self) -> &mut CocoaText {
                &mut self.common.text
            }
        }
    };
}

// ---------------------------------------------------------------------
// Generic State machinery
// ---------------------------------------------------------------------

/// State retained for an element instance between build and rebuild.
///
/// Holds the underlying `CocoaNode`, any active reactive
/// effects (so they survive as long as the element is mounted), and
/// the children's State.
pub struct ElementState<ChildState> {
    /// Pub for test inspection — consider using `Mountable::elements()`
    /// in production code paths instead.
    pub el: CocoaElem,
    /// Effects driving reactive attributes. Dropped on unmount;
    /// dropping unsubscribes from the reactive graph.
    pub(crate) _effects: Vec<RenderEffect<()>>,
    pub(crate) children: ChildState,
}

impl<ChildState: Mountable<CocoaBackend>> Mountable<CocoaBackend>
    for ElementState<ChildState>
{
    fn unmount(&mut self) {
        // Recurse first so children drop their Taffy/handler entries
        // before we drop ours.
        self.children.unmount();
        // Drop our reactive-attr effects BEFORE tearing the node down.
        // Each `RenderEffect` is the sole strong owner of its
        // `EffectInner` (the spawned driver future holds only a Weak —
        // `to_any_subscriber` downgrades). Dropping the handle closes
        // the effect's notification channel, so its driver future ends
        // and it can never re-run. If we left them (the old behavior),
        // a signal write queued just before unmount would be delivered
        // on the next run-loop tick and fire the setter against a
        // node that `teardown` already removed — the use-after-free the
        // setters' `try_*` guards otherwise have to swallow. This also
        // releases the subscription promptly instead of leaking it
        // until the `ElementState` value itself drops.
        self._effects.clear();
        self.el.remove();
    }

    fn mount(
        &mut self,
        parent: CocoaElem,
        marker: Option<CocoaElem>,
    ) {
        // Step 1: insert self.el under parent. If parent has a Taffy
        // tree handle (i.e. is descended from a Window's content_root),
        // this also registers self.el in that tree.
        parent.insert_node(self.el, marker);
        // Step 2: cascade — mount children under self.el. This is what
        // propagates the tree to descendants. We deliberately don't
        // mount children during build (which would try to attach them
        // before self.el is in any tree). The tree-aware
        // `insert_node` here registers each child as it goes.
        self.children.mount(self.el, None);
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<CocoaBackend>) -> bool {
        self.el.insert_before_this(child)
    }

    fn elements(&self) -> Vec<CocoaElem> {
        vec![self.el.clone()]
    }
}

impl<ChildState> Drop for ElementState<ChildState> {
    /// Safety net for the case where an `ElementState` is dropped
    /// *without* `unmount` ever running — e.g. a view orphaned before
    /// mount, or a panic partway through `build`. Under the
    /// NodeId-over-thread-local-store model nothing else would free
    /// our entry, so it would leak for the life of the thread.
    ///
    /// `teardown` (→ `renderer::remove`) is idempotent: after a normal
    /// `unmount` our `el` id is stale and this is a no-op. The cascade
    /// in `remove` also frees any children still in the store, and
    /// `ChildState` / `_effects` drop via ordinary field drop.
    fn drop(&mut self) {
        self.el.remove();
    }
}

// ---------------------------------------------------------------------
// stack() — Taffy flexbox container (canonical linear layout primitive)
// ---------------------------------------------------------------------

pub struct Stack<Children> {
    direction:        Option<MaybeReactive<FlexDirection>>,
    gap:              Option<MaybeReactive<f32>>,
    justify_content:  Option<MaybeReactive<JustifyContent>>,
    align:            Option<MaybeReactive<AlignItems>>,
    align_content:    Option<MaybeReactive<AlignContent>>,
    justify_items:    Option<MaybeReactive<JustifyItems>>,
    wrap:             Option<MaybeReactive<FlexWrap>>,
    /// Uniform layer scale (Phase 1.5). Identity = 1.0. Animates
    /// via `transform.scale.{x,y}` inside `with_animation`. Only
    /// surfaced on `Stack` for now — add to other builders as
    /// needed. Always present in the struct (so `.child()` /
    /// rebuild keep their shape stable across feature builds);
    /// the `.scale()` builder method and the wire-up are
    /// `#[cfg(feature = "animation")]`-gated.
    #[cfg(feature = "animation")]
    scale:            Option<MaybeReactive<f64>>,
    /// Layer translation_y in points (Phase 1.5). Independent of
    /// frame; animates via `transform.translation.y` inside
    /// `with_animation`.
    #[cfg(feature = "animation")]
    translation_y:    Option<MaybeReactive<f64>>,
    /// `bind:mouse_hover=signal` — receives `true` on cursor
    /// enter, `false` on exit. Boxed setter so the Stack stays
    /// non-generic over the signal type.
    pending_bind_mouse_hover: Option<Box<dyn FnMut(bool) + Send + 'static>>,
    children:         Children,
    common: Common,
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
        #[cfg(feature = "animation")]
        scale: None,
        #[cfg(feature = "animation")]
        translation_y: None,
        pending_bind_mouse_hover: None,
        children: (),
        common: Common::default(),
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

    // `shrink` / `basis` / `size` / `hidden` removed from Stack's
    // inherent surface — they live on the shared `WithLayout` trait
    // now, available on every builder with identical semantics.
    // Similarly `background_color` / `corner_radius` /
    // `border_width` / `border_color` / `clip` live on the shared
    // `WithDecoration` trait.

    pub fn child<NewCh>(self, child: NewCh) -> Stack<(Ch, NewCh)> {
        Stack {
            direction: self.direction,
            gap: self.gap,
            justify_content: self.justify_content,
            align: self.align,
            align_content: self.align_content,
            justify_items: self.justify_items,
            wrap: self.wrap,
            #[cfg(feature = "animation")]
            scale: self.scale,
            #[cfg(feature = "animation")]
            translation_y: self.translation_y,
            pending_bind_mouse_hover: self.pending_bind_mouse_hover,
            children: (self.children, child),
            common: self.common,
        }
    }

    pub(crate) fn set_pending_bind_mouse_hover(
        &mut self,
        setter: Box<dyn FnMut(bool) + Send + 'static>,
    ) {
        self.pending_bind_mouse_hover = Some(setter);
    }

    /// Uniform layer scale (animation-feature only). `1.0` is
    /// identity. Use inside `with_animation(...)` for a smooth
    /// press / pop effect.
    #[cfg(feature = "animation")]
    pub fn scale<V>(mut self, s: V) -> Self
    where
        V: IntoMaybeReactive<f64>,
    {
        self.scale = Some(s.into_maybe_reactive());
        self
    }

    /// Layer translation_y in points (animation-feature only).
    /// `0.0` is identity. Independent of layout — moves the
    /// rendered layer without touching Taffy. Use inside
    /// `with_animation(...)` for slide-in / slide-out effects.
    #[cfg(feature = "animation")]
    pub fn translation_y<V>(mut self, ty: V) -> Self
    where
        V: IntoMaybeReactive<f64>,
    {
        self.translation_y = Some(ty.into_maybe_reactive());
        self
    }
}

impl_common!(Stack<Children>);

impl<Ch> Render<CocoaBackend> for Stack<Ch>
where
    Ch: Render<CocoaBackend>,
{
    type State = ElementState<Ch::State>;

    fn build(self) -> Self::State {
        let el = CocoaElem::create_container();
        let mut effects = Vec::new();

        // Default direction = Column when caller didn't specify (the
        // bare `stack()` constructor). vstack/hstack pre-fill Some.
        let direction = self
            .direction
            .unwrap_or(MaybeReactive::Static(FlexDirection::Column));
        wire_attr!(effects, el, Some(direction), set_flex_direction);
        wire_attr!(effects, el, self.gap,             set_gap);
        wire_attr!(effects, el, self.justify_content, set_justify_content);
        wire_attr!(effects, el, self.align,           set_align_items);
        wire_attr!(effects, el, self.wrap,            set_flex_wrap);
        wire_attr!(effects, el, self.align_content,   set_align_content);
        wire_attr!(effects, el, self.justify_items,   set_justify_items);
        #[cfg(feature = "animation")]
        wire_attr!(
            effects, el, self.scale,
            |n: CocoaElem, s: f64| set_scale(n, s, s)
        );
        #[cfg(feature = "animation")]
        wire_attr!(
            effects, el, self.translation_y,
            |n: CocoaElem, ty: f64| set_translation(n, 0.0, ty)
        );
        // bind:mouse_hover=signal — one-way hover state writer.
        if let Some(mut setter) = self.pending_bind_mouse_hover {
            event::on_hover(
                el,
                move |entered| setter(entered),
            );
        }
        // flex_shrink / flex_basis are applied via apply_layout
        // (they're LayoutAttrs fields). Decoration attrs go through
        // apply_decoration. `hidden` goes through apply_layout (it
        // toggles Taffy display + NSView isHidden in one place).
        self.common.finish(el, &mut effects);

        // Build children but DON'T mount them yet. Mounting is
        // deferred until ElementState::mount runs (when self.el has
        // joined a tree); the recursive mount cascade then registers
        // every descendant in the right Taffy tree.
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

    children:         Children,
    common: Common,
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
        children: (),
        common: Common::default(),
    }
}

impl<Ch> Grid<Ch> {
    /// `grid-template-columns` — explicit track list. Takes anything
    /// that converts into `Vec<GridTemplateComponent>` (e.g.
    /// `[fr(1.0), fr(2.0), auto()]`).
    ///
    /// Static-only for now: making the track list reactive would
    /// require an `IntoMaybeReactive<Vec<GridTemplateComponent>>`
    /// impl, and animating tracks is a v2 feature.
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

    /// Cross-axis alignment of grid items within their cell.
    /// Same name as on `Stack` for consistency.
    pub fn align<V: IntoMaybeReactive<AlignItems>>(mut self, v: V) -> Self {
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

    // `background_color` / `corner_radius` / `border_width` /
    // `border_color` / `clip` are on the shared `WithDecoration`
    // trait. `hidden` is on `WithLayout`.

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

impl_common!(Grid<Children>);

impl<Ch> Render<CocoaBackend> for Grid<Ch>
where
    Ch: Render<CocoaBackend>,
{
    type State = ElementState<Ch::State>;

    fn build(self) -> Self::State {
        let el = CocoaElem::create_grid();
        let mut effects = Vec::new();

        // Static template-track lists go straight onto the node (no
        // reactive wrapper — animating the track shape is a v2 thing).
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
            if let Some(eff) =
                install(v, move |f| set_grid_auto_flow(el, f))
            {
                effects.push(eff);
            }
        }

        // Apply shorthand `gap` first so per-axis overrides win.
        if let Some(v) = self.gap {
            if let Some(eff) = install(v, move |g| set_gap(el, g)) {
                effects.push(eff);
            }
        }
        if let Some(v) = self.column_gap {
            if let Some(eff) = install(v, move |g| set_column_gap(el, g))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.row_gap {
            if let Some(eff) = install(v, move |g| set_row_gap(el, g)) {
                effects.push(eff);
            }
        }

        if let Some(v) = self.justify_items {
            if let Some(eff) =
                install(v, move |j| set_justify_items(el, j))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align_items {
            if let Some(eff) =
                install(v, move |a| set_align_items(el, a))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.justify_content {
            if let Some(eff) =
                install(v, move |j| set_justify_content(el, j))
            {
                effects.push(eff);
            }
        }
        if let Some(v) = self.align_content {
            if let Some(eff) =
                install(v, move |a| set_align_content(el, a))
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
// button()
// ---------------------------------------------------------------------

pub struct Button {
    title: MaybeReactive<String>,
    enabled: Option<MaybeReactive<bool>>,
    // Button-specific.
    bordered: Option<MaybeReactive<bool>>,
    key_equivalent: Option<MaybeReactive<String>>,
    // Title-tint + bold are NSButton-specific (contentTintColor +
    // boldSystemFontOfSize), not part of the generic decoration set.
    text_color:       Option<MaybeReactive<Color>>,
    bold:             Option<MaybeReactive<bool>>,
    sf_symbol:        Option<MaybeReactive<String>>,
    common: Common,
}

pub fn button() -> Button {
    Button {
        title: MaybeReactive::Static(String::new()),
        enabled: None,
        bordered: None,
        key_equivalent: None,
        text_color: None,
        bold: None,
        sf_symbol: None,
        common: Common::default(),
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
        self.common.handlers
            .push(crate::event_macos::PendingHandler::Click(Box::new(
                move || cb(),
            )));
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

    // `background_color` / `corner_radius` / `border_width` /
    // `border_color` / `clip` live on the shared `WithDecoration`
    // trait. Setting `background_color` or `corner_radius` here
    // implies `bordered=false` (handled in `Render::build`) so the
    // system bezel doesn't fight the custom paint.

    /// Custom title color. Applied via `NSButton.contentTintColor`
    /// (macOS 10.14+); no attributedTitle round-trip, so it
    /// survives title and font_size changes automatically.
    pub fn text_color<V>(mut self, c: V) -> Self
    where
        V: IntoMaybeReactive<Color>,
    {
        self.text_color = Some(c.into_maybe_reactive());
        self
    }

    /// Bold title — uses `boldSystemFontOfSize:` at the current
    /// font size. Reactive.
    pub fn bold<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.bold = Some(b.into_maybe_reactive());
        self
    }

    /// Render an SF Symbol as the button's icon (`NSButton.image`),
    /// positioned `ImageAbove` the title (toolbar-style) when a
    /// title is set, `ImageOnly` otherwise. Reactive — pass a
    /// closure to swap symbols on signal change.
    ///
    /// macOS 11+ required. Empty name clears the image. Use the
    /// `text_color=` setter to tint template symbols.
    pub fn sf_symbol<V>(mut self, name: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.sf_symbol = Some(name.into_maybe_reactive());
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

}

// Buttons fire on click (NSButton target/action). Generic over
// At because every type-level attribute extension still describes
// the same control kind.
impl crate::event_macos::SupportsEvent<crate::event_macos::ClickEvent>
    for Button
{
}

// AddAnyAttr — the typed-attribute pipeline. Each call extends
// `attrs` from `At` to `<At as NextAttribute>::Output<NewAttr>`.
// At Render::build time, `attrs.build(&el)` walks the resulting
// tuple and runs each attribute's `build(&el)` against the live
// NSView.

impl_common!(Button: text);

impl Render<CocoaBackend> for Button
where
{
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let (el, _) = CocoaElem::create_button();
        let mut effects = Vec::new();

        // Wire the title — install handles both static and reactive.
        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_title(&t);
        }) {
            effects.push(eff);
        }

        wire_attr!(effects, el, self.enabled, |n: CocoaElem, b: bool| n.set_enabled(b));


        if let Some(b) = self.bordered {
            let el_for = el.clone();
            if let Some(eff) =
                install(b, move |v| el_for.set_button_bordered(v))
            {
                effects.push(eff);
            }
        } else if self.common.decoration.background_color.is_some()
            || self.common.decoration.corner_radius.is_some()
        {
            // Caller is doing custom paint — turn off the bezel so the
            // CALayer fill isn't fighting it.
            //
            // NOTE: this runs once at build time, not reactively. If
            // `background_color` is itself reactive and resolves to
            // transparent at runtime, the bezel will still be off and
            // the button will look unrimmed. That's fine for the
            // chip / tag patterns we use this for; if a caller needs
            // true reactive bezel control they should pass an
            // explicit reactive `bordered=...` themselves.
            el.set_button_bordered(false);
        }
        wire_attr!(effects, el, self.key_equivalent, |n: CocoaElem, k: String| n.set_key_equivalent(&k));
        wire_attr!(effects, el, self.text_color, |n: CocoaElem, c: Color| n.set_button_title_color(c));
        wire_attr!(effects, el, self.bold, |n: CocoaElem, b: bool| n.set_bold(b));
        // SF symbol AFTER title is wired, so `set_button_sf_symbol`
        // can read `button.title().length()` to pick the right
        // `imagePosition`.
        wire_attr!(effects, el, self.sf_symbol, |n: CocoaElem, s: String| n.set_button_sf_symbol(&s));
        self.common.finish(el, &mut effects);

        // Run the typed-attribute pipeline. For the empty-tuple
        // default this is `().build(&el)` — a no-op.

        ElementState {
            el,
            _effects: effects,
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
    common: Common,
}

/// Portable name for the boolean toggle. On Cocoa this is the
/// same widget as `<checkbox>` (NSButton in switch style); on
/// iOS it maps to UISwitch. Use `<toggle>` in code that should
/// compile against any port; use `<checkbox>` / `<switch>` to
/// be explicit about the native widget.
pub fn toggle() -> Checkbox {
    checkbox()
}

pub fn checkbox() -> Checkbox {
    Checkbox {
        title: MaybeReactive::Static(String::new()),
        checked: MaybeReactive::Static(false),
        pending_bind_checked: None,
        common: Common::default(),
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



}

// A checkbox toggles on click.
impl crate::event_macos::SupportsEvent<crate::event_macos::ClickEvent>
    for Checkbox
{
}

impl_common!(Checkbox: text);

impl Render<CocoaBackend> for Checkbox
where
{
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let (el, _) = CocoaElem::create_checkbox();
        let mut effects = Vec::new();

        // Title: drive via the standard install pipeline.
        let el_for_title = el.clone();
        if let Some(eff) = install(self.title, move |t| {
            el_for_title.set_title(&t);
        }) {
            effects.push(eff);
        }

        // One-way `checked=...` — install fires the closure on every
        // Effect tick with a typed bool, routed through
        // `set_bool_attribute(BoolAttr::Checked, ...)`.
        let el_for_checked = el.clone();
        if let Some(eff) = install(self.checked, move |b| {
            el_for_checked.set_checked(b);
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
// slider() — NSSlider with min/max + bind:value
// ---------------------------------------------------------------------

pub struct Slider {
    value: MaybeReactive<f64>,
    min_value: MaybeReactive<f64>,
    max_value: MaybeReactive<f64>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::cocoa::bind::BoundFloat>,
    vertical: Option<MaybeReactive<bool>>,
    num_tick_marks: Option<MaybeReactive<usize>>,
    snaps_to_ticks: Option<MaybeReactive<bool>>,
    common: Common,
}

pub fn slider() -> Slider {
    Slider {
        value: MaybeReactive::Static(0.0),
        min_value: MaybeReactive::Static(0.0),
        max_value: MaybeReactive::Static(1.0),
        enabled: None,
        pending_bind: None,
        vertical: None,
        num_tick_marks: None,
        snaps_to_ticks: None,
        common: Common::default(),
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

    pub fn min_value<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<f64>,
    {
        self.min_value = v.into_maybe_reactive();
        self
    }

    pub fn max_value<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<f64>,
    {
        self.max_value = v.into_maybe_reactive();
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

impl crate::event_macos::SupportsEvent<crate::event_macos::ChangeEvent>
    for Slider
{
}

impl_common!(Slider);

impl Render<CocoaBackend> for Slider
where
{
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let (el, _) = CocoaElem::create_slider();
        let mut effects = Vec::new();

        // min/max set FIRST so initial setDoubleValue clamps correctly.
        let el_for_min = el.clone();
        if let Some(eff) =
            install(self.min_value, move |v| el_for_min.set_slider_min(v))
        {
            effects.push(eff);
        }
        let el_for_max = el.clone();
        if let Some(eff) =
            install(self.max_value, move |v| el_for_max.set_slider_max(v))
        {
            effects.push(eff);
        }

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
                el_for_enabled.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        // bind:value=signal.
        if let Some(bound) = self.pending_bind {
            let eff = crate::cocoa::bind::install_slider_value_bind(&el, bound);
            effects.push(eff);
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
// pop_up_button() — NSPopUpButton with items + bind:value (usize index)
// ---------------------------------------------------------------------

pub struct PopUpButton {
    items: Vec<String>,
    selection: MaybeReactive<usize>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_selection: Option<crate::cocoa::bind::BoundIndex>,
    pulls_down: Option<MaybeReactive<bool>>,
    common: Common,
}

pub fn pop_up_button() -> PopUpButton {
    PopUpButton {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        enabled: None,
        pending_bind_selection: None,
        pulls_down: None,
        common: Common::default(),
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

impl crate::event_macos::SupportsEvent<crate::event_macos::ChangeEvent>
    for PopUpButton
{
}

impl_common!(PopUpButton);

impl Render<CocoaBackend> for PopUpButton
where
{
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let (el, _) = CocoaElem::create_pop_up_button();
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
                el_for_enabled.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        // bind:value=signal — wires both directions.
        if let Some(bound) = self.pending_bind_selection {
            let eff = crate::cocoa::bind::install_popup_selection_bind(
                &el, bound,
            );
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
// label() — static or reactive text
// ---------------------------------------------------------------------
//
// Backed by an Element wrapping a non-editable NSTextField (the
// "label" tag). Treated as a regular Element rather than a Text so
// `<label on:click=…>` and `<MyComponent>` returning a bare label
// can flow attached events / attributes through the standard
// AddAnyAttr pipeline.

type LabelTryTextFn =
    Box<dyn FnMut() -> Result<String, throw_error::Error> + Send + 'static>;

pub struct Label {
    value: MaybeReactive<String>,
    try_text: Option<LabelTryTextFn>,
    selectable: Option<MaybeReactive<bool>>,
    bold: Option<MaybeReactive<bool>>,
    line_break: Option<MaybeReactive<LineBreak>>,
    common: Common,
}

pub fn label() -> Label {
    Label {
        value: MaybeReactive::Static(String::new()),
        try_text: None,
        selectable: None,
        bold: None,
        line_break: None,
        common: Common::default(),
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

    /// Set the label's text from a fallible closure. On `Ok(s)`
    /// the label renders `s`. On `Err(e)` the label renders
    /// nothing and registers the error with the nearest
    /// `<ErrorBoundary>` (so its `fallback` takes over).
    ///
    /// This is the ergonomic shortcut for what you'd otherwise
    /// write as `<stack>{closure_returning_result}</stack>`.
    /// Calling `.try_text()` after (or before) `.text()` replaces
    /// the previously-set fallible source.
    pub fn try_text<F, E>(mut self, mut f: F) -> Self
    where
        F: FnMut() -> Result<String, E> + Send + 'static,
        E: Into<throw_error::Error> + 'static,
    {
        self.try_text = Some(Box::new(move || f().map_err(Into::into)));
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

impl Label {
    /// Use the bold system font of the current size. Reactive.
    pub fn bold<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.bold = Some(b.into_maybe_reactive());
        self
    }

    /// Set the line-break mode. The full control over how the label
    /// handles text that doesn't fit — wrap, truncate (head/tail/
    /// middle), or clip. See [`cocoa_dom::LineBreak`].
    pub fn line_break<V>(mut self, m: V) -> Self
    where
        V: IntoMaybeReactive<LineBreak>,
    {
        self.line_break = Some(m.into_maybe_reactive());
        self
    }

    /// Shorthand: `multiline(true)` ⇒ `line_break(WORD_WRAP)`,
    /// `multiline(false)` ⇒ `line_break(TRUNCATE_TAIL)`. Use
    /// [`Self::line_break`] for the truncate-head / middle / clip
    /// variants.
    pub fn multiline<V>(self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        match b.into_maybe_reactive() {
            MaybeReactive::Static(true) => {
                self.line_break(LineBreak::WORD_WRAP)
            }
            MaybeReactive::Static(false) => {
                self.line_break(LineBreak::TRUNCATE_TAIL)
            }
            MaybeReactive::Reactive(f) => self.line_break(move || {
                if f() {
                    LineBreak::WORD_WRAP
                } else {
                    LineBreak::TRUNCATE_TAIL
                }
            }),
        }
    }

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


impl_common!(Label: text);

impl Render<CocoaBackend> for Label
where
{
    type State = ElementState<()>;

    fn build(mut self) -> Self::State {
        let (el, _) = CocoaElem::create_label();
        let mut effects = Vec::new();

        let el_for_text = el.clone();
        if let Some(eff) = install(self.value, move |s| {
            el_for_text.set_value(&s);
        }) {
            effects.push(eff);
        }

        if let Some(mut tt) = self.try_text {
            use std::cell::RefCell;
            use std::rc::Rc;
            let el_for_try = el.clone();
            // Persists across reactive runs; cleared on drop so we
            // don't leave a stale error in the boundary after the
            // label is unmounted.
            let active_error: Rc<
                RefCell<Option<crate::cocoa::error_guard::ErrorGuard>>,
            > = Rc::new(RefCell::new(None));
            let active = active_error.clone();
            let eff = RenderEffect::new(
                move |_prev: Option<()>| {
                    // Clear any prior error registered on a previous
                    // run before throwing the new one.
                    active.borrow_mut().take();
                    match tt() {
                        Ok(s) => {
                            el_for_try
                                .set_value(&s);
                        }
                        Err(e) => {
                            el_for_try
                                .set_value("");
                            *active.borrow_mut() = Some(
                                crate::cocoa::error_guard::ErrorGuard(
                                    throw_error::throw(e),
                                ),
                            );
                        }
                    }
                },
            );
            effects.push(eff);
            // `active_error` is held by the closure inside `eff`;
            // when `eff` (and via it the closure) drops, the
            // ErrorGuard inside drops and clears any active error.
        }

        for h in std::mem::take(&mut self.common.handlers) {
            // NSTextField (label) is an NSControl but not an
            // NSButton. Route Click via on_action; other events
            // fall through to apply_to (no-ops on non-NSTextField
            // events, which is most of them on a label).
            match h {
                crate::event_macos::PendingHandler::Click(cb) => {
                    el.on_action(cb);
                }
                other => other.apply_to(el),
            }
        }

        if let Some(s) = self.selectable {
            let el_for = el.clone();
            if let Some(eff) = install(s, move |v| el_for.set_selectable(v)) {
                effects.push(eff);
            }
        }
        if let Some(m) = self.line_break {
            let el_for = el.clone();
            if let Some(eff) = install(m, move |v| el_for.set_line_break(v)) {
                effects.push(eff);
            }
        }
        effects.extend(apply_decoration(el, self.common.decoration));
        effects.extend(apply_universal(el, self.common.universal));
        effects.extend(apply_text(el, self.common.text));
        // Apply bold AFTER apply_text so font_size is set first; bold
        // reads the current point size to preserve it.
        if let Some(b) = self.bold {
            let el_for = el.clone();
            if let Some(eff) = install(b, move |v| el_for.set_bold(v)) {
                effects.push(eff);
            }
        }
        effects.extend(apply_layout(el, self.common.layout));

        if let Some(r) = self.common.node_ref {
            r.load(el);
        }

        crate::cocoa::directives::run_all(self.common.directives, el);


        ElementState {
            el,
            _effects: effects,
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
    placeholder: Option<MaybeReactive<String>>,
    enabled: Option<MaybeReactive<bool>>,
    /// If `true`, build an NSSecureTextField instead of NSTextField.
    /// Used by the `secure_text_field()` constructor; same builder
    /// otherwise (NSSecureTextField is a subclass).
    secure: bool,
    /// `bind:value=...` state, applied at build time by
    /// `install_text_field_value_bind`. Distinct from `.value(...)`
    /// (which is one-way: signal → field).
    pending_bind: Option<crate::cocoa::bind::BoundValue>,
    bordered: Option<MaybeReactive<bool>>,
    bezeled: Option<MaybeReactive<bool>>,
    intrinsic_width: Option<MaybeReactive<IntrinsicWidth>>,
    common: Common,
}

/// Controls the field's behaviour during the measure pass.
///
/// AppKit's NSTextField has a content-driven intrinsic width — it
/// reports a size that fits its current text. For editable fields,
/// this means the field grows with every keystroke unless something
/// pins its width. The default in this fork is to override the
/// measure callback to return width=0 so the parent (a vstack with
/// flex_grow, a fixed-width container, etc.) decides the width.
///
/// Set to `FromContent` if you want the natural AppKit behaviour —
/// useful for read-only fields used as labels that should grow with
/// their text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IntrinsicWidth {
    /// Width=0 in the measure pass; parent decides. The 95% case.
    /// Default.
    #[default]
    FromParent,
    /// Read NSTextField's natural content width. Field grows with
    /// its text.
    FromContent,
}

impl IntoMaybeReactive<IntrinsicWidth> for IntrinsicWidth {
    fn into_maybe_reactive(self) -> MaybeReactive<IntrinsicWidth> {
        MaybeReactive::Static(self)
    }
}
impl<F> IntoMaybeReactive<IntrinsicWidth> for F
where
    F: Fn() -> IntrinsicWidth + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<IntrinsicWidth> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

pub fn text_field() -> TextField {
    TextField {
        value: MaybeReactive::Static(String::new()),
        placeholder: None,
        enabled: None,
        secure: false,
        pending_bind: None,
        bordered: None,
        bezeled: None,
        intrinsic_width: None,
        common: Common::default(),
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
        bordered: None,
        bezeled: None,
        intrinsic_width: None,
        common: Common::default(),
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

    /// Placeholder text shown when the field is empty. Reactive —
    /// accepts a `&str`, `String`, or `Fn() -> String` closure.
    pub fn placeholder<V: IntoMaybeReactive<String>>(mut self, s: V) -> Self {
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

    /// Internal: stash a `bind:value=...` for installation in
    /// `Render::build`. Used by the `BindAttribute` impl in
    /// `crate::cocoa::bind`.
    pub(crate) fn set_pending_bind_value(
        &mut self,
        bound: crate::cocoa::bind::BoundValue,
    ) {
        self.pending_bind = Some(bound);
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
impl crate::event_macos::SupportsEvent<crate::event_macos::CommitEvent>
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
    /// Choose how the field's intrinsic width is computed during
    /// the measure pass. Default: [`IntrinsicWidth::FromParent`]
    /// (the parent decides via flex_grow / fixed width). Set to
    /// [`IntrinsicWidth::FromContent`] to let the field grow with
    /// its text — useful for read-only fields used as labels.
    pub fn intrinsic_width<V>(mut self, w: V) -> Self
    where
        V: IntoMaybeReactive<IntrinsicWidth>,
    {
        self.intrinsic_width = Some(w.into_maybe_reactive());
        self
    }
}


impl_common!(TextField: text);

impl Render<CocoaBackend> for TextField
where
{
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let el = if self.secure {
            CocoaElem::create_secure_text_field().0
        } else {
            CocoaElem::create_text_field().0
        };
        let mut effects = Vec::new();

        if let Some(p) = self.placeholder {
            let el_for = el.clone();
            if let Some(eff) = install(p, move |s| {
                el_for.set_placeholder(&s);
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

        // Install one-way `.value(...)` if used.
        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_value(&v);
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
        if let Some(iw) = self.intrinsic_width {
            let el_for = el.clone();
            if let Some(eff) = install(iw, move |w| {
                el_for.set_intrinsic_width_from_content(
                    matches!(w, IntrinsicWidth::FromContent),
                );
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
// date_picker() — NSDatePicker
// ---------------------------------------------------------------------

pub struct DatePicker {
    value: MaybeReactive<Date>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::cocoa::bind::BoundDate>,
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
    pub fn value<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<Date>,
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



}

// NSDatePicker fires target/action when the user changes the date.
// As with ColorWell, we use the existing Click marker — semantically
// "change" but that's what the macro emits and the wiring works.
impl crate::event_macos::SupportsEvent<crate::event_macos::ChangeEvent>
    for DatePicker
{
}

impl DatePicker {
    /// Set the picker's visual style. `Textual`,
    /// `TextualAndStepper` (default), or `ClockAndCalendar`.
    pub fn style<V>(mut self, s: V) -> Self
    where
        V: IntoMaybeReactive<DatePickerStyle>,
    {
        self.style = Some(s.into_maybe_reactive());
        self
    }
    /// Earliest selectable date. Builder API doesn't expose a
    /// "clear" path; use a directive to call
    /// `Element::set_date_picker_min(None)` if you need that.
    pub fn min_date<V>(mut self, d: V) -> Self
    where
        V: IntoMaybeReactive<Date>,
    {
        self.min_date = Some(d.into_maybe_reactive());
        self
    }
    /// Latest selectable date.
    pub fn max_date<V>(mut self, d: V) -> Self
    where
        V: IntoMaybeReactive<Date>,
    {
        self.max_date = Some(d.into_maybe_reactive());
        self
    }
}


impl_common!(DatePicker);

impl Render<CocoaBackend> for DatePicker
where
{
    type State = ElementState<()>;

    fn build(mut self) -> Self::State {
        let (el, _) = CocoaElem::create_date_picker();
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
                el_for_enabled.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind {
            let eff =
                crate::cocoa::bind::install_date_picker_bind(&el, bound);
            effects.push(eff);
        }

        for h in std::mem::take(&mut self.common.handlers) {
            // Date picker is an NSControl, not an NSButton — route
            // Click via on_action.
            match h {
                crate::event_macos::PendingHandler::Click(cb) => {
                    el.on_action(cb);
                }
                other => other.apply_to(el),
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
// stepper() — NSStepper, +/- numeric increment
// ---------------------------------------------------------------------

pub struct Stepper {
    value: MaybeReactive<f64>,
    min_value: MaybeReactive<f64>,
    max_value: MaybeReactive<f64>,
    increment: MaybeReactive<f64>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::cocoa::bind::BoundFloat>,
    common: Common,
}

pub fn stepper() -> Stepper {
    Stepper {
        value: MaybeReactive::Static(0.0),
        min_value: MaybeReactive::Static(0.0),
        max_value: MaybeReactive::Static(100.0),
        increment: MaybeReactive::Static(1.0),
        enabled: None,
        pending_bind: None,
        common: Common::default(),
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

    pub fn min_value<V: IntoMaybeReactive<f64>>(mut self, v: V) -> Self {
        self.min_value = v.into_maybe_reactive();
        self
    }

    pub fn max_value<V: IntoMaybeReactive<f64>>(mut self, v: V) -> Self {
        self.max_value = v.into_maybe_reactive();
        self
    }

    pub fn increment<V: IntoMaybeReactive<f64>>(mut self, v: V) -> Self {
        self.increment = v.into_maybe_reactive();
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



}

impl crate::event_macos::SupportsEvent<crate::event_macos::ChangeEvent>
    for Stepper
{
}

impl_common!(Stepper);

impl Render<CocoaBackend> for Stepper
where
{
    type State = ElementState<()>;

    fn build(mut self) -> Self::State {
        let (el, _) = CocoaElem::create_stepper();
        let mut effects = Vec::new();

        // Bounds + increment first so the initial setDoubleValue
        // clamps correctly. Each is independently reactive — when
        // min_value changes, `configure_stepper` is re-applied to
        // the live NSStepper.
        let el_for_min = el.clone();
        if let Some(eff) =
            install(self.min_value, move |v| el_for_min.set_stepper_min(v))
        {
            effects.push(eff);
        }
        let el_for_max = el.clone();
        if let Some(eff) =
            install(self.max_value, move |v| el_for_max.set_stepper_max(v))
        {
            effects.push(eff);
        }
        let el_for_inc = el.clone();
        if let Some(eff) = install(self.increment, move |v| {
            el_for_inc.set_stepper_increment(v)
        }) {
            effects.push(eff);
        }

        let el_for_val = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_val.set_stepper_value(v);
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
            let eff =
                crate::cocoa::bind::install_stepper_value_bind(&el, bound);
            effects.push(eff);
        }

        for h in std::mem::take(&mut self.common.handlers) {
            match h {
                crate::event_macos::PendingHandler::Click(cb) => {
                    el.on_action(cb);
                }
                other => other.apply_to(el),
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
// progress_indicator() — NSProgressIndicator. Bar (determinate) or
// spinner (indeterminate). Read-only — `value=` drives the bar but
// there's no bind here, since user input doesn't reach a progress
// indicator.
// ---------------------------------------------------------------------

pub struct ProgressIndicator {
    value: MaybeReactive<f64>,
    max_value: MaybeReactive<f64>,
    indeterminate: MaybeReactive<bool>,
    displayed_when_stopped: Option<MaybeReactive<bool>>,
    common: Common,
}

pub fn progress_indicator() -> ProgressIndicator {
    ProgressIndicator {
        value: MaybeReactive::Static(0.0),
        max_value: MaybeReactive::Static(1.0),
        indeterminate: MaybeReactive::Static(false),
        displayed_when_stopped: None,
        common: Common::default(),
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

    pub fn max_value<V: IntoMaybeReactive<f64>>(mut self, v: V) -> Self {
        self.max_value = v.into_maybe_reactive();
        self
    }

    /// `true` switches to spinner mode and starts the animation;
    /// `false` is a determinate progress bar.
    pub fn indeterminate<V: IntoMaybeReactive<bool>>(mut self, b: V) -> Self {
        self.indeterminate = b.into_maybe_reactive();
        self
    }


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


impl_common!(ProgressIndicator);

impl Render<CocoaBackend> for ProgressIndicator
where
{
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let (el, _) = CocoaElem::create_progress_indicator();
        let mut effects = Vec::new();

        // Order matters: max before value so the value clamps
        // correctly; indeterminate after both because indeterminate
        // mode ignores value (and starts the animation).
        let el_for_max = el.clone();
        if let Some(eff) = install(self.max_value, move |v| {
            el_for_max.set_progress_max(v);
        }) {
            effects.push(eff);
        }

        let el_for_val = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_val.set_progress_value(v);
        }) {
            effects.push(eff);
        }

        let el_for_ind = el.clone();
        if let Some(eff) = install(self.indeterminate, move |b| {
            el_for_ind.set_progress_indeterminate(b);
        }) {
            effects.push(eff);
        }

        if let Some(d) = self.displayed_when_stopped {
            let el_for = el.clone();
            if let Some(eff) = install(d, move |v| {
                el_for.set_progress_displayed_when_stopped(v)
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
// color_well() — NSColorWell, opens system color picker on click.
// `value=` for one-way; `bind:value=` for two-way.
// ---------------------------------------------------------------------

pub struct ColorWell {
    value: MaybeReactive<Color>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind: Option<crate::cocoa::bind::BoundColor>,
    common: Common,
}

pub fn color_well() -> ColorWell {
    ColorWell {
        value: MaybeReactive::Static(Color::WHITE),
        enabled: None,
        pending_bind: None,
        common: Common::default(),
    }
}

impl ColorWell {
    pub fn value<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<Color>,
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



}

// NSColorWell fires target/action when the user picks a color and
// dismisses the picker. We use the existing `Click` event marker
// here because that's what the macro emits for `on:click=…` — but
// semantically it's a "value committed" event, more like
// `on:change` would be on the web. Document this divergence
// rather than introduce a separate Color-payload event for now.
impl crate::event_macos::SupportsEvent<crate::event_macos::ChangeEvent>
    for ColorWell
{
}

impl_common!(ColorWell);

impl Render<CocoaBackend> for ColorWell
where
{
    type State = ElementState<()>;

    fn build(mut self) -> Self::State {
        let (el, _) = CocoaElem::create_color_well();
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
                el_for_enabled.set_enabled(b);
            }) {
                effects.push(eff);
            }
        }

        if let Some(bound) = self.pending_bind {
            let eff =
                crate::cocoa::bind::install_color_well_bind(&el, bound);
            effects.push(eff);
        }

        for h in std::mem::take(&mut self.common.handlers) {
            // ColorWell is an NSControl, not an NSButton — route
            // Click via on_action so the target/action wiring fires
            // when the user picks a color.
            match h {
                crate::event_macos::PendingHandler::Click(cb) => {
                    el.on_action(cb);
                }
                other => other.apply_to(el),
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
// segmented_control() — NSSegmentedControl with items + bind:value (usize index)
// ---------------------------------------------------------------------

pub struct SegmentedControl {
    items: Vec<String>,
    selection: MaybeReactive<usize>,
    enabled: Option<MaybeReactive<bool>>,
    pending_bind_selection: Option<crate::cocoa::bind::BoundIndex>,
    segment_style: Option<MaybeReactive<SegmentStyle>>,
    common: Common,
}

pub fn segmented_control() -> SegmentedControl {
    SegmentedControl {
        items: Vec::new(),
        selection: MaybeReactive::Static(0),
        enabled: None,
        pending_bind_selection: None,
        segment_style: None,
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



}

// Click semantics for segmented_control match popup: a "click"
// is a selection change.
impl crate::event_macos::SupportsEvent<crate::event_macos::ChangeEvent>
    for SegmentedControl
{
}

impl SegmentedControl {
    /// Visual style: `Rounded`, `RoundRect`, `Capsule`,
    /// `SmallSquare`, `Separated`, etc. See
    /// `cocoa_dom::SegmentStyle`.
    pub fn segment_style<V>(mut self, s: V) -> Self
    where
        V: IntoMaybeReactive<SegmentStyle>,
    {
        self.segment_style = Some(s.into_maybe_reactive());
        self
    }
}


impl_common!(SegmentedControl);

impl Render<CocoaBackend> for SegmentedControl
where
{
    type State = ElementState<()>;

    fn build(mut self) -> Self::State {
        let (el, _) = CocoaElem::create_segmented_control();
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
                el_for_enabled.set_enabled(b);
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

        for h in std::mem::take(&mut self.common.handlers) {
            // Click on a segmented control is conceptually
            // "selection changed" — install via on_action (NSControl
            // path) rather than on_click (NSButton subtree only).
            match h {
                crate::event_macos::PendingHandler::Click(cb) => {
                    el.on_action(cb);
                }
                other => other.apply_to(el),
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
    axis: ScrollAxis,
    autohides_scrollers: Option<MaybeReactive<bool>>,
    has_horizontal_scroller: Option<MaybeReactive<bool>>,
    has_vertical_scroller: Option<MaybeReactive<bool>>,
    common: Common,
}

pub fn scroll_view() -> ScrollView<()> {
    ScrollView {
        children: (),
        axis: ScrollAxis::Vertical,
        autohides_scrollers: None,
        has_horizontal_scroller: None,
        has_vertical_scroller: None,
        common: Common::default(),
    }
}

impl<Ch> ScrollView<Ch> {
    /// Set which axis (or axes) the scroll view scrolls along.
    /// Default is `Vertical`. Picks the documentView wrapper's
    /// Taffy style (so content overflows on the chosen axis and is
    /// bounded on the other) and adjusts the scroller-visibility
    /// defaults to match. Explicit `has_*_scroller` setters can
    /// still override the scroller visibility afterward.
    ///
    /// Not reactive — the wrapper's style is picked at registration
    /// time and not swapped later.
    pub fn axis(mut self, axis: ScrollAxis) -> Self {
        self.axis = axis;
        self
    }

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
            axis: self.axis,
            autohides_scrollers: self.autohides_scrollers,
            has_horizontal_scroller: self.has_horizontal_scroller,
            has_vertical_scroller: self.has_vertical_scroller,
            common: self.common,
        }
    }
}

impl_common!(ScrollView<Children>);

impl<Ch> Render<CocoaBackend> for ScrollView<Ch>
where
    Ch: Render<CocoaBackend>,
{
    type State = ElementState<Ch::State>;

    fn build(self) -> Self::State {
        let (el, _) = CocoaElem::create_scroll_view();
        let mut effects = Vec::new();

        // Apply axis FIRST — it sets the documentView wrapper's
        // Taffy style and the scroller-visibility defaults. The
        // explicit `has_*_scroller` setters below run after and
        // can override.
        el.set_scroll_axis(self.axis);

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
        self.common.finish(el, &mut effects);

        // Same cascade pattern as View: defer child mount to
        // ElementState::mount, so the tree-aware insert_node
        // registers each descendant in the right Taffy tree.
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
// image_view() — NSImageView, source from a file path
// ---------------------------------------------------------------------

pub struct ImageView {
    source: Option<MaybeReactive<String>>,
    bytes: Option<MaybeReactive<Option<Vec<u8>>>>,
    sf_symbol: Option<MaybeReactive<String>>,
    tint: Option<MaybeReactive<Color>>,
    common: Common,
}

pub fn image_view() -> ImageView {
    ImageView {
        source: None,
        bytes: None,
        sf_symbol: None,
        tint: None,
        common: Common::default(),
    }
}

impl ImageView {
    /// File path to the image. Empty string clears the image.
    /// Network URLs aren't supported here — fetch them yourself
    /// (e.g. via reqwest) and write to a temp file, then pass the
    /// path. NSImage's `initWithContentsOfFile:` handles PNG, JPEG,
    /// PDF, TIFF, etc.
    ///
    /// Mutually exclusive with [`Self::sf_symbol`]; if both are set
    /// `sf_symbol` wins (applied last).
    pub fn source<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.source = Some(v.into_maybe_reactive());
        self
    }

    /// In-memory image bytes (PNG/JPEG/GIF/TIFF/HEIC/PDF, auto-
    /// detected by NSImage). `None` clears the image. Reactive —
    /// pass a closure to swap the bytes on signal change.
    ///
    /// Use this when the image data isn't on disk: HTTP fetches,
    /// generated images, etc. Run the fetch on a background async
    /// runtime, hand the bytes back via the reactive bridge
    /// described in the async docs, then this setter applies them
    /// on the main thread.
    ///
    /// Mutually exclusive with [`Self::source`] / [`Self::sf_symbol`];
    /// last write wins (`bytes` is applied after `source` but
    /// before `sf_symbol`).
    pub fn bytes<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<Option<Vec<u8>>>,
    {
        self.bytes = Some(v.into_maybe_reactive());
        self
    }

    /// Render an SF Symbol by name (e.g. `"plus.circle"`, `"trash"`,
    /// `"square.and.arrow.up"`). Reactive — pass a closure to swap
    /// symbols on signal change. Empty name or an unknown symbol
    /// clears the image.
    ///
    /// Requires macOS 11+. SF Symbols are template images — set
    /// [`Self::tint`] to colour them; the default is the system
    /// accent / label colour.
    pub fn sf_symbol<V>(mut self, name: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.sf_symbol = Some(name.into_maybe_reactive());
        self
    }

    /// Tint colour applied via `NSImageView.contentTintColor`.
    /// Reactive. Most useful with [`Self::sf_symbol`] — regular
    /// RGBA images aren't recoloured.
    pub fn tint<V>(mut self, c: V) -> Self
    where
        V: IntoMaybeReactive<Color>,
    {
        self.tint = Some(c.into_maybe_reactive());
        self
    }


}

impl_common!(ImageView);

impl Render<CocoaBackend> for ImageView
where
{
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let (el, _) = CocoaElem::create_image_view();
        let mut effects = Vec::new();

        // `source` first so `sf_symbol` can replace it last-write-wins
        // if both were set on the same builder.
        if let Some(src) = self.source {
            let el_for = el.clone();
            if let Some(eff) =
                install(src, move |s| el_for.set_image_view_path(&s))
            {
                effects.push(eff);
            }
        }
        if let Some(b) = self.bytes {
            let el_for = el.clone();
            if let Some(eff) = install(b, move |bytes| {
                el_for.set_image_view_bytes(bytes.as_deref())
            }) {
                effects.push(eff);
            }
        }
        if let Some(sym) = self.sf_symbol {
            let el_for = el.clone();
            if let Some(eff) =
                install(sym, move |s| el_for.set_image_view_sf_symbol(&s))
            {
                effects.push(eff);
            }
        }
        if let Some(tint) = self.tint {
            let el_for = el.clone();
            if let Some(eff) =
                install(tint, move |c| el_for.set_image_view_tint(c))
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


}

impl_common!(TextView: text);

impl Render<CocoaBackend> for TextView
where
{
    type State = ElementState<()>;

    fn build(self) -> Self::State {
        let (el, _) = CocoaElem::create_text_view();
        let mut effects = Vec::new();

        // value=… one-way drive. Routes through StringAttr::Value
        // which knows how to find the inner NSTextView.
        let el_for_value = el.clone();
        if let Some(eff) = install(self.value, move |v| {
            el_for_value.set_value(&v);
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
// AddAnyAttr<Dom> for the 13 leaf builders. Routes spread attrs (e.g.
// `<MyComponent on:click=...>`) onto the existing `directives: Vec<...>`
// post-build hook, which gets drained in `Render::build` after the
// underlying NSView is constructed. Same install timing as a `use:`
// directive — the attr's `apply_to` is called with `&CocoaNode` and
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
            impl leptos_native::renderer::view::AddAnyAttr<crate::CocoaBackend> for $builder {
                fn add_any_attr<__A>(mut self, attr: __A) -> Self
                where
                    __A: leptos_native::renderer::view::ApplyAttr<crate::CocoaBackend>,
                {
                    self.common.directives.push(Box::new(move |el: CocoaElem| {
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

impl<Children> AddAnyAttr<CocoaBackend> for Stack<Children> {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where
        __A: ApplyAttr<CocoaBackend>,
    {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on Stack (vstack/hstack/stack_view). Containers have no NSControl target/action slot — click and other UIControl events have no install path. Attach to a child button/label/text_field instead."
        )
    }
}

impl<Children> AddAnyAttr<CocoaBackend> for Grid<Children> {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where
        __A: ApplyAttr<CocoaBackend>,
    {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on Grid. Containers have no NSControl target/action slot — click and other UIControl events have no install path. Attach to a child button/label/text_field instead."
        )
    }
}

impl<Children> AddAnyAttr<CocoaBackend> for ScrollView<Children> {
    #[track_caller]
    fn add_any_attr<__A>(self, _attr: __A) -> Self
    where
        __A: ApplyAttr<CocoaBackend>,
    {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on ScrollView. NSScrollView isn't an NSControl — click handlers have no install path. Attach to inner content instead."
        )
    }
}
