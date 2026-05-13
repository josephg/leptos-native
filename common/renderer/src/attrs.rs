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
//!    layout attributes. Renderer-agnostic by design; each backend
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
// AlignSelf — flex item cross-axis alignment override
// ---------------------------------------------------------------------

/// Per-child override of the parent flex container's `align_items`.
///
/// Renderer-agnostic; each backend converts to its layout-engine
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
/// Renderer-agnostic; each backend's `apply_layout` converts to its
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
// Generic IntoMaybeReactive impls
// ---------------------------------------------------------------------
//
// One static + one closure impl per renderer-owned value type. The
// macro is module-private; external crates can't use it because the
// orphan rule blocks closure-form `impl<F: Fn() -> T>
// renderer::IntoMaybeReactive<T> for F` from any crate that doesn't
// own this trait (F is the impl's Self type and appears before the
// trait's first local type parameter). Each port crate therefore
// declares its own `IntoMaybeReactive` shadow trait + impls; see
// `cocoa/leptos_cocoa/src/cocoa/attr.rs` and equivalents.

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
    String, bool, i32, f32, f64, usize, Dim, AlignSelf, GridLine,
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
}

// `Default` by hand: derive(Default) would require `C: Default + A: Default`
// which we don't want to demand of caller types.
impl<C: 'static, A: 'static> Default for TextAttrs<C, A> {
    fn default() -> Self {
        Self {
            text_color: None,
            alignment: None,
            font_size: None,
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
    pub padding: Option<MaybeReactive<f32>>,
    pub margin: Option<MaybeReactive<f32>>,
    pub width: Option<MaybeReactive<Dim>>,
    pub height: Option<MaybeReactive<Dim>>,
    pub min_width: Option<MaybeReactive<Dim>>,
    pub min_height: Option<MaybeReactive<Dim>>,
    pub max_width: Option<MaybeReactive<Dim>>,
    pub max_height: Option<MaybeReactive<Dim>>,
    pub flex_grow: Option<MaybeReactive<f32>>,
    pub align_self: Option<MaybeReactive<AlignSelf>>,

    // Grid placement (no-op when the parent isn't a grid).
    pub grid_column_start: Option<MaybeReactive<GridLine>>,
    pub grid_column_end:   Option<MaybeReactive<GridLine>>,
    pub grid_row_start:    Option<MaybeReactive<GridLine>>,
    pub grid_row_end:      Option<MaybeReactive<GridLine>>,
}

/// Builder accessor for [`LayoutAttrs`]. Implementations expose
/// `&mut self.layout`; the default methods supply the chainable
/// setters.
pub trait WithLayout: Sized {
    fn layout_mut(&mut self) -> &mut LayoutAttrs;

    fn padding<V: IntoMaybeReactive<f32>>(mut self, p: V) -> Self {
        self.layout_mut().padding = Some(p.into_maybe_reactive());
        self
    }

    fn margin<V: IntoMaybeReactive<f32>>(mut self, m: V) -> Self {
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

    /// Per-child override of the parent flex container's `align_items`.
    fn align_self<V: IntoMaybeReactive<AlignSelf>>(mut self, a: V) -> Self {
        self.layout_mut().align_self = Some(a.into_maybe_reactive());
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
}
