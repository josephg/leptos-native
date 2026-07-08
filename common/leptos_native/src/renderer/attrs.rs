//! Cross-backend attribute plumbing.
//!
//! Three things live here:
//!
//! 1. **`MaybeReactive<T>` + `IntoMaybeReactive<T>` + `install`** —
//!    the static-or-closure attribute-value abstraction every native
//!    backend uses. A builder method like `.title(...)` accepts
//!    anything that implements `IntoMaybeReactive<String>`; the value
//!    is later applied through `install`, which either calls the
//!    setter once (static) or wraps it in a `RenderEffect` (reactive).
//!
//! 2. **`Dim` + `AlignSelf`** — small enums used by the universal
//!    layout attributes. Backend-agnostic by design; each backend
//!    converts to its layout-engine-native type at apply time.
//!
//! 3. **`LayoutAttrs` / `UniversalAttrs` / `TextAttrs<C, A>`** + their
//!    accessor traits **`WithLayout` / `WithUniversal` / `WithText`**.
//!    Each builder embeds these structs as fields and implements the
//!    corresponding trait by handing back a `&mut` to its field.
//!    The trait's default methods then provide the chainable setters
//!    (`.padding(...)`, `.alpha(...)`, etc.) consistently across every
//!    builder, so adding a new attribute is one edit, not N.
//!
//! Backends provide their own `apply_layout` / `apply_universal` /
//! `apply_text` functions that read these structs and install effects
//! against the live element. Those functions are free to depend on
//! backend-specific setters; the trait surface stays here.

#[cfg(feature = "reactive_graph")]
pub use reactive_graph::effect::RenderEffect;

// ---------------------------------------------------------------------
// MaybeReactive + IntoMaybeReactive
// ---------------------------------------------------------------------

/// Either a static value or a closure that produces one reactively.
///
/// The closure is `Send` so that `MaybeReactive<T>` itself is `Send`,
/// which is required by leptos's `IntoView` blanket impl. Most user
/// closures are Send already (reactive_graph signals are Send).
///
/// `Fn` (not `FnMut`): we only ever READ the value through this
/// closure — `RenderEffect` re-runs the closure on each signal
/// change to fetch a fresh value, never mutates closure state.
pub enum MaybeReactive<T: 'static> {
    Static(T),
    Reactive(Box<dyn Fn() -> T + Send + 'static>),
}

/// Conversion trait so attribute setters can take either a bare
/// value or a `Fn() -> T` closure transparently.
pub trait IntoMaybeReactive<T: 'static> {
    fn into_maybe_reactive(self) -> MaybeReactive<T>;
}

/// Drives `apply` whenever the underlying signal(s) change.
///
/// For `Static`, calls `apply(value)` once and returns `None`.
/// For `Reactive`, builds a `RenderEffect` that calls
/// `apply(closure())` on every reactive run. The effect's internal
/// constructor runs the closure synchronously inside the reactive
/// observer, so the initial value is set before this returns.
#[cfg(feature = "reactive_graph")]
pub fn install<T: 'static>(
    value: MaybeReactive<T>,
    mut apply: impl FnMut(T) + 'static,
) -> Option<RenderEffect<()>> {
    match value {
        MaybeReactive::Static(v) => {
            apply(v);
            None
        }
        MaybeReactive::Reactive(f) => {
            let effect = RenderEffect::new(move |_prev| {
                let v = f();
                apply(v);
            });
            Some(effect)
        }
    }
}

// ---------------------------------------------------------------------
// Dim — sizing primitive
// ---------------------------------------------------------------------

/// Dimension for sizing attrs (`width`, `height`, `min_*`, `max_*`).
///
/// - `Px(v)` — fixed length in points.
/// - `Pct(v)` — fraction of the parent's content axis, `0.0..=1.0`.
/// - `Auto` — defer to the layout engine.
///
/// `From<f32>` constructs a `Px` so existing call sites that pass
/// raw floats keep working — `width(520.0)` and `width(Dim::pct(0.5))`
/// are both valid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dim {
    Px(f32),
    Pct(f32),
    Auto,
}

impl Dim {
    pub const fn px(v: f32) -> Self { Self::Px(v) }
    pub const fn pct(v: f32) -> Self { Self::Pct(v) }
    pub const AUTO: Self = Self::Auto;
}

impl From<f32> for Dim {
    fn from(v: f32) -> Self { Self::Px(v) }
}

// ---------------------------------------------------------------------
// Edges — per-side padding / margin
// ---------------------------------------------------------------------

/// Per-side inset, in points. Used by `padding` and `margin`.
///
/// Construction:
/// - `Edges::all(n)` — uniform on all four sides.
/// - `Edges::axis(h, v)` — horizontal then vertical pair (`h` =
///   left+right, `v` = top+bottom). The argument order is fixed in
///   the method name (`axis` = "horizontal-axis, vertical-axis")
///   rather than implicit in a tuple, so call sites read
///   unambiguously.
/// - `Edges::trbl(t, r, b, l)` — explicit per-side, CSS-shorthand
///   order.
/// - Builder form: `Edges::ZERO.top(8.0).left(12.0)`.
/// - `From<f32>` lifts a uniform value: `padding=8.0` ⇒
///   `Edges::all(8.0)`.
///
/// Tuple `From` impls are **deliberately not provided**: `(h, v)`
/// vs `(v, h)` and `(t, r, b, l)` vs CSS's other shorthand
/// orderings is a famous source of bugs. Use the named
/// constructors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub top:    f32,
    pub right:  f32,
    pub bottom: f32,
    pub left:   f32,
}

impl Edges {
    pub const ZERO: Self = Self { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };

    pub const fn all(n: f32) -> Self {
        Self { top: n, right: n, bottom: n, left: n }
    }

    /// `axis(h, v)` — `h` left+right, `v` top+bottom. The named
    /// method makes the order explicit at the call site
    /// (`Edges::axis(16.0, 8.0)` → 16 horizontal, 8 vertical).
    pub const fn axis(h: f32, v: f32) -> Self {
        Self { top: v, right: h, bottom: v, left: h }
    }

    /// Per-side `top right bottom left` — matches CSS shorthand
    /// `padding: 8px 12px 8px 12px;`.
    pub const fn trbl(t: f32, r: f32, b: f32, l: f32) -> Self {
        Self { top: t, right: r, bottom: b, left: l }
    }

    /// `Edges::ZERO.top(8.0)` — only the top edge. Builder methods
    /// for each side; pair to set arbitrary subsets.
    pub const fn top(mut self, n: f32) -> Self { self.top = n; self }
    pub const fn right(mut self, n: f32) -> Self { self.right = n; self }
    pub const fn bottom(mut self, n: f32) -> Self { self.bottom = n; self }
    pub const fn left(mut self, n: f32) -> Self { self.left = n; self }
}

impl From<f32> for Edges {
    fn from(n: f32) -> Self { Self::all(n) }
}

// ---------------------------------------------------------------------
// AlignSelf — flex item cross-axis alignment override
// ---------------------------------------------------------------------

/// Per-child override of the parent flex container's `align_items`.
///
/// Backend-agnostic; each backend converts to its layout-engine
/// equivalent (e.g. `taffy::AlignItems`) at apply time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlignSelf {
    Auto,
    Start,
    End,
    Center,
    Stretch,
    Baseline,
}

// ---------------------------------------------------------------------
// GridLine — per-item placement on a grid container
// ---------------------------------------------------------------------

/// One end of a grid-cell's row or column placement.
///
/// - `Auto` — auto-place this end (the layout engine picks).
/// - `Line(n)` — explicit 1-based line number; negative counts from
///   the end (line `-1` = last line, like CSS).
/// - `Span(n)` — N tracks from whichever end isn't pinned.
///
/// Backend-agnostic; each backend's `apply_layout` converts to its
/// layout-engine type (`taffy::GridPlacement`) at install time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridLine {
    Auto,
    Line(i16),
    Span(u16),
}

impl GridLine {
    pub const AUTO: Self = Self::Auto;
}

impl From<i16> for GridLine {
    fn from(n: i16) -> Self { Self::Line(n) }
}

/// `From<i32>` exists so integer literals (Rust's default `i32`) work
/// with the placement methods without an explicit suffix. Out-of-range
/// values panic rather than silently truncating to `i16` — a 50,000-row
/// grid is almost certainly user error, not a coherent layout request.
impl From<i32> for GridLine {
    fn from(n: i32) -> Self {
        Self::Line(i16::try_from(n).unwrap_or_else(|_| {
            panic!(
                "grid line {n} is out of range for i16 (-32768..=32767). \
                 If you genuinely need a line index this far from the \
                 origin, construct `GridLine::Line(...)` directly."
            )
        }))
    }
}

/// `Span(n)` constructor — sugar for `GridLine::Span(n)`. Mirrors CSS
/// `span N` keyword in placement shorthands.
pub const fn span(n: u16) -> GridLine { GridLine::Span(n) }

/// `Auto` constructor — sugar for `GridLine::Auto`. Useful as a
/// placeholder for the un-pinned end of a `grid_column((start, end))`.
pub const fn auto_line() -> GridLine { GridLine::Auto }

/// A `(start, end)` pair of grid lines, accepted by the
/// [`WithLayout::grid_column`] / [`WithLayout::grid_row`] shorthands.
/// Built via `From` impls from tuples of various integer or
/// `GridLine` types so the `view!{}` macro syntax
/// `grid_column=(1, -1)` works without explicit constructors.
#[derive(Clone, Copy, Debug)]
pub struct GridRange(pub GridLine, pub GridLine);

impl<S: Into<GridLine>, E: Into<GridLine>> From<(S, E)> for GridRange {
    fn from((s, e): (S, E)) -> Self { Self(s.into(), e.into()) }
}

// ---------------------------------------------------------------------
// Overflow — CSS-style overflow control
// ---------------------------------------------------------------------

/// Container overflow behaviour.
///
/// Three values mapping directly onto CSS's `overflow` keyword:
///
/// - [`Overflow::Visible`] (default): no clip. Children extending
///   past the container's frame paint outside it. As a flex/grid
///   item, this container's automatic minimum size stays
///   content-based — its parent can't shrink it below its content's
///   intrinsic min-size.
/// - [`Overflow::Clip`]: visual clip at the container's frame, but
///   the layout shape is otherwise unchanged — auto-min-size stays
///   content-based. Use this for "round corners and clip children
///   to the rounded shape" recipes (pair with `corner_radius`) where
///   you want the visual clip without changing how flex parents are
///   allowed to shrink this element.
/// - [`Overflow::Hidden`]: visual clip **and** the Taffy
///   auto-min-size becomes `0` — the container's parent can shrink
///   it all the way down to zero on the scroll axis. This is the
///   right value for content that's expected to overflow under
///   pressure (e.g. a `<scroll_view>` viewport, or any "shrink to
///   fit, clip the rest" container).
///
/// One attribute, two effects: visual clip (port-specific —
/// `masksToBounds` on CALayer / `clipsToBounds` on UIView /
/// `gtk_widget_set_overflow` on GtkWidget) and the Taffy layout
/// effect for `Hidden` only. `Scroll` is not modelled here —
/// `<scroll_view>` is the explicit scroll container today.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Clip,
    Hidden,
}

// ---------------------------------------------------------------------
// Generic IntoMaybeReactive impls
// ---------------------------------------------------------------------
//
// One static + one closure impl per renderer-owned value type, via the
// exported `impl_pair!` above. Each port crate declares its own
// `IntoMaybeReactive` shadow trait + impls with the same macro (the
// orphan rule blocks the closure-form impl from any crate that doesn't
// own the trait); see `cocoa/leptos_cocoa/src/cocoa/attr.rs`.

/// Generate the static-value + closure impls of `IntoMaybeReactive<T>`
/// for one or more concrete `T`s.
///
/// Each port keeps a **port-local** `IntoMaybeReactive` trait (the
/// orphan rule blocks the blanket closure impl
/// `impl<F: Fn() -> Local> IntoMaybeReactive<Local> for F` from any
/// crate that doesn't own the trait), but the macro body is identical
/// everywhere — so it lives here once. The unqualified `IntoMaybeReactive`
/// and `MaybeReactive` it references resolve at the **call site**, i.e.
/// against the port's local trait + its re-exported `MaybeReactive`.
/// Invoke as `leptos_native::impl_pair!(TypeA, TypeB, …)` (or `use
/// leptos_native::impl_pair;` first).
#[macro_export]
macro_rules! impl_pair {
    ($($t:ty),* $(,)?) => {
        $(
            impl IntoMaybeReactive<$t> for $t {
                fn into_maybe_reactive(self) -> MaybeReactive<$t> {
                    MaybeReactive::Static(self)
                }
            }
            impl<F> IntoMaybeReactive<$t> for F
            where
                F: Fn() -> $t + Send + 'static,
            {
                fn into_maybe_reactive(self) -> MaybeReactive<$t> {
                    MaybeReactive::Reactive(Box::new(self))
                }
            }
        )*
    };
}

impl_pair!(
    String, bool, i32, f32, f64, usize, Dim, AlignSelf, GridLine, Edges,
    Overflow, (f32, f32),
);

// Sugar: pass raw integer literals to grid_column / grid_row methods
// without wrapping in `GridLine::Line(...)`.
impl IntoMaybeReactive<GridLine> for i16 {
    fn into_maybe_reactive(self) -> MaybeReactive<GridLine> {
        MaybeReactive::Static(GridLine::Line(self))
    }
}
impl IntoMaybeReactive<GridLine> for i32 {
    fn into_maybe_reactive(self) -> MaybeReactive<GridLine> {
        MaybeReactive::Static(GridLine::from(self))
    }
}

// `&str` → `String` is special: not a `T → MaybeReactive<T>` shape.
impl IntoMaybeReactive<String> for &str {
    fn into_maybe_reactive(self) -> MaybeReactive<String> {
        MaybeReactive::Static(self.to_owned())
    }
}

// `f32 → MaybeReactive<Dim>` so callers can pass `width(520.0)`
// without wrapping in `Dim::px(...)`.
impl IntoMaybeReactive<Dim> for f32 {
    fn into_maybe_reactive(self) -> MaybeReactive<Dim> {
        MaybeReactive::Static(Dim::Px(self))
    }
}

// f32 → Edges so call sites can keep `padding=8.0` shorthand for
// uniform padding. The reactive form is `move || Edges::all(...)`
// rather than `move || 8.0_f32` — keeping the lift unambiguous
// (we don't infer "uniform" from a closure returning f32).
impl IntoMaybeReactive<Edges> for f32 {
    fn into_maybe_reactive(self) -> MaybeReactive<Edges> {
        MaybeReactive::Static(Edges::all(self))
    }
}

// ---------------------------------------------------------------------
// Universal attrs (alpha, tool_tip)
// ---------------------------------------------------------------------

/// Attrs every native control supports: opacity + tooltip text.
#[derive(Default)]
pub struct UniversalAttrs {
    pub alpha: Option<MaybeReactive<f64>>,
    pub tool_tip: Option<MaybeReactive<String>>,
}

/// Builder accessor trait for [`UniversalAttrs`]. Implementations
/// expose `&mut self.universal`; the default methods supply the
/// chainable setters.
pub trait WithUniversal: Sized {
    fn universal_mut(&mut self) -> &mut UniversalAttrs;

    /// View opacity, 0.0..=1.0. Reactive: pass an f64 or a closure.
    fn alpha<V: IntoMaybeReactive<f64>>(mut self, a: V) -> Self {
        self.universal_mut().alpha = Some(a.into_maybe_reactive());
        self
    }

    /// Tooltip text shown on mouse hover. Empty string removes
    /// any previous tooltip.
    fn tool_tip<V: IntoMaybeReactive<String>>(mut self, s: V) -> Self {
        self.universal_mut().tool_tip = Some(s.into_maybe_reactive());
        self
    }
}

// ---------------------------------------------------------------------
// Text attrs (text_color, alignment, font_size)
// ---------------------------------------------------------------------

/// Text-styling attrs. Generic over `C = Color` and `A = TextAlignment`
/// because those are backend-specific (cocoa: `Color` /
/// `NSTextAlignment`; uikit: `Color` / `NSTextAlignment`).
pub struct TextAttrs<C: 'static, A: 'static> {
    pub text_color: Option<MaybeReactive<C>>,
    pub alignment: Option<MaybeReactive<A>>,
    pub font_size: Option<MaybeReactive<f64>>,
    pub font_weight: Option<MaybeReactive<i32>>,
}

// `Default` by hand: derive(Default) would require `C: Default + A: Default`
// which we don't want to demand of caller types.
impl<C: 'static, A: 'static> Default for TextAttrs<C, A> {
    fn default() -> Self {
        Self {
            text_color: None,
            alignment: None,
            font_size: None,
            font_weight: None,
        }
    }
}

/// Builder accessor for [`TextAttrs`]. Each builder picks the
/// concrete `C`/`A` for its backend.
pub trait WithText<C: 'static, A: 'static>: Sized {
    fn text_attrs_mut(&mut self) -> &mut TextAttrs<C, A>;

    fn text_color<V: IntoMaybeReactive<C>>(mut self, c: V) -> Self {
        self.text_attrs_mut().text_color = Some(c.into_maybe_reactive());
        self
    }

    /// Text alignment within the control's frame.
    fn alignment<V: IntoMaybeReactive<A>>(mut self, a: V) -> Self {
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

// ---------------------------------------------------------------------
// Decoration attrs (background_color, corner_radius, border, clip)
// ---------------------------------------------------------------------

/// "Rectangle styling" attrs available on every visual element —
/// fill color, rounded corners, border, and a clip-to-bounds toggle.
/// Generic over `C` because the color type is backend-specific
/// (cocoa: `Color`; gtk: `gdk::RGBA`; etc.).
pub struct DecorationAttrs<C: 'static> {
    pub background_color: Option<MaybeReactive<C>>,
    pub corner_radius:    Option<MaybeReactive<f32>>,
    pub border_width:     Option<MaybeReactive<f32>>,
    pub border_color:     Option<MaybeReactive<C>>,
    pub shadow_color:     Option<MaybeReactive<C>>,
    pub shadow_opacity:   Option<MaybeReactive<f32>>,
    pub shadow_radius:    Option<MaybeReactive<f32>>,
    pub shadow_offset:    Option<MaybeReactive<(f32, f32)>>,
}

impl<C: 'static> Default for DecorationAttrs<C> {
    fn default() -> Self {
        Self {
            background_color: None,
            corner_radius:    None,
            border_width:     None,
            border_color:     None,
            shadow_color:     None,
            shadow_opacity:   None,
            shadow_radius:    None,
            shadow_offset:    None,
        }
    }
}

/// Builder accessor for [`DecorationAttrs`]. Each builder picks the
/// concrete `C` for its backend.
///
/// Like [`WithText`], each port typically defines a non-generic
/// shadow trait (pinning `C = port::Color`) so the chainable setters
/// can use the port-local [`IntoMaybeReactive`] machinery.
pub trait WithDecoration<C: 'static>: Sized {
    fn decoration_mut(&mut self) -> &mut DecorationAttrs<C>;

    /// Layer-backed background fill.
    fn background_color<V: IntoMaybeReactive<C>>(mut self, c: V) -> Self {
        self.decoration_mut().background_color = Some(c.into_maybe_reactive());
        self
    }

    /// Round the corners. Pair with [`WithLayout::overflow`]`(Overflow::Clip)`
    /// to clip children to the rounded shape; without it the fill is
    /// rounded but children draw through.
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
    fn border_color<V: IntoMaybeReactive<C>>(mut self, c: V) -> Self {
        self.decoration_mut().border_color = Some(c.into_maybe_reactive());
        self
    }

    /// Drop-shadow color. Only visible when `shadow_opacity > 0`.
    fn shadow_color<V: IntoMaybeReactive<C>>(mut self, c: V) -> Self {
        self.decoration_mut().shadow_color = Some(c.into_maybe_reactive());
        self
    }

    /// Drop-shadow opacity, 0.0..=1.0. `0.0` disables.
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
// Layout attrs (padding, margin, sizing, flex_grow, align_self)
// ---------------------------------------------------------------------

/// Universal layout attributes — applied uniformly to every element
/// (leaves and containers). The backend's `apply_layout` reads these
/// and installs effects against the underlying Taffy node.
///
/// `padding` and `margin` are uniform (single `f32`) for now; per-
/// edge values aren't supported here yet.
#[derive(Default)]
pub struct LayoutAttrs {
    pub padding: Option<MaybeReactive<Edges>>,
    pub margin: Option<MaybeReactive<Edges>>,
    pub width: Option<MaybeReactive<Dim>>,
    pub height: Option<MaybeReactive<Dim>>,
    pub min_width: Option<MaybeReactive<Dim>>,
    pub min_height: Option<MaybeReactive<Dim>>,
    pub max_width: Option<MaybeReactive<Dim>>,
    pub max_height: Option<MaybeReactive<Dim>>,
    // Flex-item sizing (active when this element is a child of a
    // flex container — `<vstack>` / `<hstack>` / `<view>`).
    pub flex_grow:   Option<MaybeReactive<f32>>,
    pub flex_shrink: Option<MaybeReactive<f32>>,
    pub flex_basis:  Option<MaybeReactive<f32>>,
    pub align_self:  Option<MaybeReactive<AlignSelf>>,

    // Grid placement (no-op when the parent isn't a grid).
    pub grid_column_start: Option<MaybeReactive<GridLine>>,
    pub grid_column_end:   Option<MaybeReactive<GridLine>>,
    pub grid_row_start:    Option<MaybeReactive<GridLine>>,
    pub grid_row_end:      Option<MaybeReactive<GridLine>>,

    /// `hidden=true` removes the element from layout (CSS `display:
    /// none` — the slot collapses; siblings reflow as if the element
    /// weren't there) AND hides the underlying view. `false` restores
    /// the element to its normal display mode.
    pub hidden: Option<MaybeReactive<bool>>,

    /// CSS-style overflow. See [`Overflow`] for semantics.
    pub overflow: Option<MaybeReactive<Overflow>>,
}

/// Builder accessor for [`LayoutAttrs`]. Implementations expose
/// `&mut self.layout`; the default methods supply the chainable
/// setters.
pub trait WithLayout: Sized {
    fn layout_mut(&mut self) -> &mut LayoutAttrs;

    /// Inner padding. Accepts any of:
    /// - `padding=8.0` — uniform (via `From<f32> for Edges`).
    /// - `padding=(16.0, 8.0)` — `(horizontal, vertical)`.
    /// - `padding=(t, r, b, l)` — explicit per-side.
    /// - `padding=Edges::ZERO.with_top(8.0)` — builder form.
    fn padding<V: IntoMaybeReactive<Edges>>(mut self, p: V) -> Self {
        self.layout_mut().padding = Some(p.into_maybe_reactive());
        self
    }

    /// Outer margin. Same shapes as `padding`.
    fn margin<V: IntoMaybeReactive<Edges>>(mut self, m: V) -> Self {
        self.layout_mut().margin = Some(m.into_maybe_reactive());
        self
    }

    fn width<V: IntoMaybeReactive<Dim>>(mut self, w: V) -> Self {
        self.layout_mut().width = Some(w.into_maybe_reactive());
        self
    }

    fn height<V: IntoMaybeReactive<Dim>>(mut self, h: V) -> Self {
        self.layout_mut().height = Some(h.into_maybe_reactive());
        self
    }

    fn min_width<V: IntoMaybeReactive<Dim>>(mut self, w: V) -> Self {
        self.layout_mut().min_width = Some(w.into_maybe_reactive());
        self
    }

    fn min_height<V: IntoMaybeReactive<Dim>>(mut self, h: V) -> Self {
        self.layout_mut().min_height = Some(h.into_maybe_reactive());
        self
    }

    fn max_width<V: IntoMaybeReactive<Dim>>(mut self, w: V) -> Self {
        self.layout_mut().max_width = Some(w.into_maybe_reactive());
        self
    }

    fn max_height<V: IntoMaybeReactive<Dim>>(mut self, h: V) -> Self {
        self.layout_mut().max_height = Some(h.into_maybe_reactive());
        self
    }

    /// Flex grow factor along the parent's main axis. 0 (default)
    /// means don't grow into extra space; 1+ means take a share.
    fn flex_grow<V: IntoMaybeReactive<f32>>(mut self, g: V) -> Self {
        self.layout_mut().flex_grow = Some(g.into_maybe_reactive());
        self
    }

    /// Flex shrink factor. Taffy default is `1` (will shrink to
    /// fit). Set to `0` to refuse shrinking — used by [`size`] to
    /// keep fixed-size chrome rigid even when a sibling overflows.
    fn flex_shrink<V: IntoMaybeReactive<f32>>(mut self, s: V) -> Self {
        self.layout_mut().flex_shrink = Some(s.into_maybe_reactive());
        self
    }

    /// Flex basis — the element's initial main-axis size before
    /// `flex_grow` / `flex_shrink` distribute remaining space.
    fn flex_basis<V: IntoMaybeReactive<f32>>(mut self, b: V) -> Self {
        self.layout_mut().flex_basis = Some(b.into_maybe_reactive());
        self
    }

    /// Per-child override of the parent flex container's `align_items`.
    fn align_self<V: IntoMaybeReactive<AlignSelf>>(mut self, a: V) -> Self {
        self.layout_mut().align_self = Some(a.into_maybe_reactive());
        self
    }

    /// Lock the element to an `n×n` square that flex layout can't
    /// squeeze. Sets `width`, `height`, `min_width`, `min_height`,
    /// and `flex_shrink=0` simultaneously — the magic five that
    /// keep an avatar/icon/checkbox rigid even when a sibling
    /// would otherwise compress it along the flex axis.
    ///
    /// Static-only: reactive square sizing would mean cloning the
    /// same closure into five separate reactive setters, which
    /// requires `Sync` and `Clone` on the user's closure. The
    /// trade-off isn't worth it for the rare case; if you need
    /// reactive size, set `width`/`height` directly.
    fn size(mut self, n: f32) -> Self {
        let l = self.layout_mut();
        l.width       = Some(MaybeReactive::Static(Dim::Px(n)));
        l.height      = Some(MaybeReactive::Static(Dim::Px(n)));
        l.min_width   = Some(MaybeReactive::Static(Dim::Px(n)));
        l.min_height  = Some(MaybeReactive::Static(Dim::Px(n)));
        l.flex_shrink = Some(MaybeReactive::Static(0.0));
        self
    }

    // -- grid placement ---------------------------------------------------
    //
    // Long-form: one line at a time.

    fn grid_column_start<V: IntoMaybeReactive<GridLine>>(mut self, v: V) -> Self {
        self.layout_mut().grid_column_start = Some(v.into_maybe_reactive());
        self
    }
    fn grid_column_end<V: IntoMaybeReactive<GridLine>>(mut self, v: V) -> Self {
        self.layout_mut().grid_column_end = Some(v.into_maybe_reactive());
        self
    }
    fn grid_row_start<V: IntoMaybeReactive<GridLine>>(mut self, v: V) -> Self {
        self.layout_mut().grid_row_start = Some(v.into_maybe_reactive());
        self
    }
    fn grid_row_end<V: IntoMaybeReactive<GridLine>>(mut self, v: V) -> Self {
        self.layout_mut().grid_row_end = Some(v.into_maybe_reactive());
        self
    }

    /// CSS `grid-column: <start> / <end>` shorthand. Takes a single
    /// `(start, end)` tuple so the `view!{}` macro syntax
    /// `grid_column=(1, -1)` works (the macro emits one arg per attr).
    /// Each element of the tuple accepts `Into<GridLine>` — `i16`,
    /// `i32`, `GridLine`, or `span(n)`.
    fn grid_column<R: Into<GridRange>>(mut self, r: R) -> Self {
        let GridRange(s, e) = r.into();
        let l = self.layout_mut();
        l.grid_column_start = Some(MaybeReactive::Static(s));
        l.grid_column_end   = Some(MaybeReactive::Static(e));
        self
    }

    /// CSS `grid-row: <start> / <end>` shorthand. See [`grid_column`].
    fn grid_row<R: Into<GridRange>>(mut self, r: R) -> Self {
        let GridRange(s, e) = r.into();
        let l = self.layout_mut();
        l.grid_row_start = Some(MaybeReactive::Static(s));
        l.grid_row_end   = Some(MaybeReactive::Static(e));
        self
    }

    /// Place at a single column line; end auto-resolves to span 1.
    /// Equivalent to CSS `grid-column: <line>` (single number).
    fn grid_column_at<V: Into<GridLine>>(mut self, line: V) -> Self {
        let l = self.layout_mut();
        l.grid_column_start = Some(MaybeReactive::Static(line.into()));
        l.grid_column_end   = Some(MaybeReactive::Static(GridLine::Auto));
        self
    }

    /// Place at a single row line; end auto-resolves to span 1.
    fn grid_row_at<V: Into<GridLine>>(mut self, line: V) -> Self {
        let l = self.layout_mut();
        l.grid_row_start = Some(MaybeReactive::Static(line.into()));
        l.grid_row_end   = Some(MaybeReactive::Static(GridLine::Auto));
        self
    }

    /// CSS `grid-column: span N`. Start auto-placed; end spans N tracks.
    fn grid_column_span(mut self, n: u16) -> Self {
        let l = self.layout_mut();
        l.grid_column_start = Some(MaybeReactive::Static(GridLine::Auto));
        l.grid_column_end   = Some(MaybeReactive::Static(GridLine::Span(n)));
        self
    }

    /// CSS `grid-row: span N`. Start auto-placed; end spans N tracks.
    fn grid_row_span(mut self, n: u16) -> Self {
        let l = self.layout_mut();
        l.grid_row_start = Some(MaybeReactive::Static(GridLine::Auto));
        l.grid_row_end   = Some(MaybeReactive::Static(GridLine::Span(n)));
        self
    }

    /// Hide the element and remove it from layout — CSS `display: none`
    /// semantics. The slot collapses; siblings reflow as if the element
    /// weren't there. The underlying view is also marked hidden so it
    /// doesn't draw. Setting `hidden=false` restores normal display.
    ///
    /// (Earlier this only set NSView's `isHidden`, which kept the slot
    /// reserved. The current behaviour matches what most callers
    /// actually want — and what `<Show>` would do, with less ceremony.)
    fn hidden<V: IntoMaybeReactive<bool>>(mut self, h: V) -> Self {
        self.layout_mut().hidden = Some(h.into_maybe_reactive());
        self
    }

    /// CSS `overflow`. See [`Overflow`] — `Hidden` clips content
    /// visually at this element's frame **and** changes the Taffy
    /// automatic-min-size for this element (as a flex/grid item)
    /// from content-based to `0`, letting a flex parent shrink it
    /// past its content's intrinsic size.
    fn overflow<V: IntoMaybeReactive<Overflow>>(mut self, o: V) -> Self {
        self.layout_mut().overflow = Some(o.into_maybe_reactive());
        self
    }
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Edges ------------------------------------------------------

    #[test]
    fn edges_zero_is_zero() {
        let e = Edges::ZERO;
        assert_eq!(e.top, 0.0);
        assert_eq!(e.right, 0.0);
        assert_eq!(e.bottom, 0.0);
        assert_eq!(e.left, 0.0);
    }

    #[test]
    fn edges_all_is_uniform() {
        let e = Edges::all(8.0);
        assert_eq!(e, Edges { top: 8.0, right: 8.0, bottom: 8.0, left: 8.0 });
    }

    #[test]
    fn edges_axis_maps_h_to_left_right_and_v_to_top_bottom() {
        // `axis(h, v)` — explicit in the name. h goes to left+right,
        // v goes to top+bottom. Documented at call site; verify here.
        let e = Edges::axis(16.0, 8.0);
        assert_eq!(e.left, 16.0);
        assert_eq!(e.right, 16.0);
        assert_eq!(e.top, 8.0);
        assert_eq!(e.bottom, 8.0);
    }

    #[test]
    fn edges_trbl_is_top_right_bottom_left() {
        let e = Edges::trbl(1.0, 2.0, 3.0, 4.0);
        assert_eq!(e.top, 1.0);
        assert_eq!(e.right, 2.0);
        assert_eq!(e.bottom, 3.0);
        assert_eq!(e.left, 4.0);
    }

    #[test]
    fn edges_builder_per_side() {
        let e = Edges::ZERO.top(1.0).right(2.0).bottom(3.0).left(4.0);
        assert_eq!(e, Edges::trbl(1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn edges_builder_is_const() {
        // The whole builder chain is `const fn`; this constructs at
        // compile time. If any method loses `const` we lose const
        // construction across the whole pipeline.
        const E: Edges = Edges::ZERO.top(8.0).left(12.0);
        assert_eq!(E.top, 8.0);
        assert_eq!(E.left, 12.0);
        assert_eq!(E.right, 0.0);
        assert_eq!(E.bottom, 0.0);
    }

    #[test]
    fn edges_from_f32_is_uniform() {
        let e: Edges = 5.0.into();
        assert_eq!(e, Edges::all(5.0));
    }

    #[test]
    fn into_maybe_reactive_for_f32_to_edges_is_uniform_static() {
        // `padding=8.0` ⇒ `MaybeReactive::Static(Edges::all(8.0))`.
        // f32 implements `IntoMaybeReactive<T>` for several Ts
        // (f32, Dim, Edges), so the target type needs to be named.
        let mr: MaybeReactive<Edges> =
            <f32 as IntoMaybeReactive<Edges>>::into_maybe_reactive(8.0);
        match mr {
            MaybeReactive::Static(e) => assert_eq!(e, Edges::all(8.0)),
            MaybeReactive::Reactive(_) => panic!("expected Static"),
        }
    }

    // ---- Dim / GridLine sanity --------------------------------------

    #[test]
    fn dim_from_f32_is_px() {
        let d: Dim = Dim::from(120.0_f32);
        assert_eq!(d, Dim::Px(120.0));
    }

    #[test]
    fn grid_line_from_i32_clamps_to_i16_or_panics() {
        // i32 within i16 range: passes through.
        let g: GridLine = 5_i32.into();
        assert_eq!(g, GridLine::Line(5));
        let g: GridLine = (-3_i32).into();
        assert_eq!(g, GridLine::Line(-3));
    }

    #[test]
    #[should_panic(expected = "out of range for i16")]
    fn grid_line_from_oversized_i32_panics() {
        let _: GridLine = 50_000_i32.into();
    }

    #[test]
    fn span_helper_is_span_variant() {
        assert_eq!(span(3), GridLine::Span(3));
    }

    #[test]
    fn auto_line_helper_is_auto_variant() {
        assert_eq!(auto_line(), GridLine::Auto);
    }
}
