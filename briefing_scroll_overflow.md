# Briefing: scroll_view single-pass layout, `overflow` attribute, and Taffy semantics

Covers a refactor session on the macOS port's `<scroll_view>` and a
small cross-port API addition. Three things landed; one thing we
explicitly deferred. Plus a useful set of findings about what
Taffy's `overflow` style field does (and does not) actually mean,
which informed the design.

## What landed

### 1. `<scroll_view>` is now a single Taffy pass

**Before.** `cocoa_dom::layout::compute_layout` ran the tree's
primary Taffy pass, then walked the tree twice more:

- `relayout_scroll_views` — for each `<scroll_view>`, save the
  current style, swap in a probe style (`height: auto`), re-run a
  layout pass on that subtree with `AvailableSpace::MaxContent`,
  restore the original style, restore the original final layout
  for the scroll_view's own rect.
- `fixup_scroll_view_documents` — walk again and compute the
  envelope of each scroll_view's children, set the NSScrollView's
  `documentView.frame` to that envelope.

Both walks existed because the Taffy tree was structurally smaller
than the AppKit view tree. NSScrollView has *two* views internally
(the outer scroll view + a `documentView` that holds the content),
but Taffy only saw one node, so the documentView's size had to be
back-computed in a separate pass.

**After.** Each `<scroll_view>` is two Taffy nodes:

- The outer node, backed by the NSScrollView, with the existing
  `flex_basis: 0, min_size.height: 0, overflow: Hidden` style.
  Sized by its flex parent (the viewport).
- A wrapper node, backed by the NSScrollView's documentView, with
  `flex_direction: Column, flex_shrink: 0`. Always sized to its
  natural content extent because `flex_shrink: 0` prevents Taffy
  from squashing it to fit the outer node's viewport. The user's
  children become children of the wrapper at the Taffy level (and
  remain children of the documentView at the AppKit level — that
  routing already existed via `Element::subview_parent`).

The wrapper is allocated lazily in `register_in_tree` when the
scroll_view's `is_scroll_view` flag is set, and removed in
`drop_node`. `attach_child` / `insert_child_at` / `detach_child` go
through a new `taffy_child_parent` helper that follows the redirect
(scroll_view → wrapper). A new `child_taffy_parent: Option<NodeId>`
field on `CocoaMeta` carries the link.

`compute_layout_inner` is now just "run one Taffy pass, apply
frames." `relayout_scroll_views` and `fixup_scroll_view_documents`
were deleted (~110 lines).

The four `scroll_view_*` tests in
`cocoa/dom/tests/element_creation.rs` plus
`scroll_view_bounds_parent_to_viewport` in `layout.rs` all still
pass, and the `scroll_view_cocoa` example launches cleanly. No
behavior change visible to user code.

### 2. `Overflow` user-facing attribute

A new CSS-shaped attribute usable on any container. Three values:

| `Overflow`    | Visual clip | Auto-min-size (as flex/grid item) |
|---------------|-------------|-----------------------------------|
| `Visible`     | no          | content-based (default)           |
| `Clip`        | yes         | content-based                     |
| `Hidden`      | yes         | `0`                               |

- `Visible`: pure default; no clip, content-based auto-min.
- `Clip`: visual clip only. Use for "round corners and clip
  children to the rounded shape" recipes — pair with
  `corner_radius`. Does not change layout shape.
- `Hidden`: visual clip *and* Taffy auto-min-size becomes 0,
  letting a flex parent shrink this element below its content's
  intrinsic size. The right value for content expected to overflow
  under pressure.

Per-port wiring (`cocoa/dom/src/layout.rs` etc.):

- **Cocoa**: `set_clip` calls into the existing
  `cocoa_dom::layout::set_clip` (CALayer `masksToBounds`).
- **GTK**: `set_clip` calls `gtk::Widget::set_overflow(Hidden|Visible)`.
- **iOS**: `set_clip` is the trait default no-op until UIView's
  `clipsToBounds` gets wired. Layout half (auto-min-size 0 from
  `Hidden`) takes effect on iOS today; visual clip doesn't.

`set_overflow` (in `common/renderer/src/setters.rs`) writes
`style.overflow.{x,y}` uniformly — both axes the same. Per-axis
overflow isn't exposed yet; the user-facing enum is whole-element.

### 3. `clip=true` removed; `overflow=` is now the single attribute

The previous `clip=bool` decoration attribute mapped exactly to
CSS `overflow: clip` semantics (visual clip, no layout effect),
which is now `overflow=Overflow::Clip`. Since the codebase hasn't
shipped, we collapsed the two surfaces:

- `DecorationAttrs.clip` field, `WithDecoration::clip(...)`
  setter, `DecorationElement::set_clip` trait method, and the
  port-local `clip(...)` builder methods are all gone.
- Nine `clip=true` call sites in examples
  (`show_fallback`, `multi_error_boundary`, `dark_mode`, `spotify`,
  `pages`) were rewritten to `overflow=Overflow::Clip`.
- User-facing docs (`docs/book/src/layout/attributes.md`,
  `scroll.md`, `elements/stack.md`, `elements/color_well.md`,
  `platform/ios/deltas.md`) updated.

The combined attribute also closes the install-order footgun: there
used to be two attributes that both ended up controlling
`masksToBounds`, applied through different `apply_*` functions, with
"whichever fires last wins" semantics. One attribute, three values,
no conflict.

## What we deferred: `Overflow::Scroll`

The natural fourth value isn't implemented. `<scroll_view>` is still
the explicit scroll container today. Two reasons we held off:

- `overflow=scroll` would have to be a *structural* attribute — the
  backing view needs to change at create time (NSView →
  NSScrollView, with a documentView and the wrapper-node redirect).
  All other style attributes can be set or animated reactively after
  the backing view exists; making `overflow` the one that can't is
  awkward.
- `<scroll_view>` already exposes scroller-specific configuration
  (`has_vertical_scroller`, `has_horizontal_scroller`,
  `autohides_scrollers`). If `overflow=scroll` were the surface, those
  knobs would need a different home.

A reasonable future path: `overflow=scroll` becomes the canonical
surface, `<scroll_view>` becomes a thin alias for
`<view overflow=Overflow::Scroll has_vertical_scroller=...>`. Not
forced; user is still thinking.

## Taffy findings: what `overflow` does (and doesn't)

The two design questions above hinged on understanding Taffy's
`overflow` semantics precisely. We ran a small probe
(`/tmp/taffy_probe`) and read `src/style/mod.rs`,
`src/compute/common/content_size.rs`, and the compute paths in
`src/compute/{leaf,block,flexbox}.rs` (taffy 0.10.1). Findings:

### Taffy's `Overflow` has four values, with two effects

```rust
// src/style/mod.rs
pub enum Overflow { Visible, Clip, Hidden, Scroll }
```

Two layout-only effects (Taffy doesn't paint or visually clip —
that's the consumer's job):

1. **Auto-minimum-size override for flex/grid items.** A flex/grid
   item's automatic minimum size is normally its content's
   min-size — even with `flex_shrink: 1`, an item won't shrink
   below this. Setting the item's `overflow` to `Hidden` or
   `Scroll` (`is_scroll_container() == true`) forces the auto-min
   to `0`:

   ```rust
   // src/style/mod.rs:371
   pub(crate) fn maybe_into_automatic_min_size(self) -> Option<f32> {
       match self.is_scroll_container() {  // Hidden | Scroll → true
           true => Some(0.0),
           false => None,                  // Visible | Clip → content
       }
   }
   ```

   This is the only thing that lets our `<scroll_view>` be a
   fully-shrinkable flex item with no content-based size floor.

2. **Scrollbar gutter reservation (Scroll only).** `Overflow::Scroll`
   reserves `style.scrollbar_width()` of space inside the node for
   a scrollbar. With `scrollbar_width: 0` (Taffy's default), Scroll
   behaves identically to Hidden. The doc says so explicitly:

   > "If this is `0` then `Scroll` behaves identically to `Hidden`."
   > — `src/style/mod.rs:335`

### What Taffy's `overflow` does *not* do

Three things people coming from CSS expect that Taffy's `overflow`
field doesn't deliver:

- **It doesn't change `flex_shrink` semantics on the node's
  children.** A child with `flex_shrink: 1` still shrinks under
  pressure regardless of the parent's overflow. The probe showed
  this directly: a parent with `overflow: Scroll` plus
  `flex_shrink: 1` children gives the same squashed output as
  `overflow: Visible` plus the same children. The only lever for
  "don't squish my children" is `flex_shrink: 0` (or a wrapper
  that has it).

- **It doesn't establish a "natural-sizing" formatting context.**
  Real CSS gives scroll containers a different sizing rule for
  their children on the scroll axis — max-content treatment that
  ignores the container's resolved size. Taffy doesn't model that.

- **It doesn't visually clip anything.** That's deliberate — Taffy
  is layout-only. `Hidden` vs `Clip` in Taffy is purely about
  whether the node's content_size contribution propagates to its
  ancestors' scroll regions (via `compute_content_size_contribution`
  in `src/compute/common/content_size.rs`).

### Implications for the design choices

These findings drove three concrete decisions:

- **Why the wrapper node was the right call** instead of "set
  `flex_shrink: 0` on the user's direct scroll_view children." The
  probe confirmed both approaches give the same child sizing
  output. But: putting `flex_shrink: 0` on user children mutates
  styles the user might want to control; the wrapper insulates
  them. The wrapper also gives us the documentView's frame size for
  free via `tree.layout(wrapper)`, where the bare approach would
  need to back-compute the envelope.

- **Why we kept `overflow: Hidden` on the scroll_view's outer
  style** rather than switching to `Scroll`. They produce
  identical layout output when `scrollbar_width: 0`. `Hidden`
  documents intent better ("absorb overflow, no gutter") and is
  robust against any future change to `scrollbar_width` defaults
  or exposure as a user attribute.

- **Why the user-facing `Overflow` enum is three values, not
  four.** `Scroll` in Taffy is the same as `Hidden` plus a gutter
  reservation we don't currently need (AppKit scrollers are
  overlay; GTK and iOS scrollbars on overlay-style platforms
  similarly). Adding a `Scroll` variant before the structural
  story for scroll containers is settled would have meant the
  enum value's behavior would shift later — better to delay.

## When does content overflow at all?

For the `Overflow::Hidden` semantics to matter, the user has to be
in a situation where content can actually exceed its container.
The precise condition is:

> The container's resolved size on an axis < its content's
> laid-out total on that axis.

In a flex-default world (everything `flex_shrink: 1`), content
normally adapts to container size, so overflow doesn't happen.
The cases where it *does* happen, in roughly decreasing frequency:

1. **Container has a binding upper-size constraint AND content
   can't shrink to fit.** Children either have `flex_shrink: 0`,
   or have hit their CSS automatic-minimum-size floor (= their
   content min-size, unless their *own* overflow is non-Visible —
   that's Taffy effect #1 above). For text leaves the floor is
   real and routine.
2. **Container is sized to 0 by a stronger constraint.** Same
   problem the scroll_view "no bounded ancestor" case hits.
3. **Absolutely-positioned children** out of the flex flow.
4. **flex_wrap: NoWrap** with too many items.
5. **Grid items placed outside the implicit tracks.**
6. **Children with explicit `flex_shrink: 0`** that don't fit.

## When `<scroll_view>` is "broken"

Tangent we covered while discussing the error story. The exact
constraint for a scroll_view to function is:

> The scroll_view's resolved size on its scroll axis > 0.

Not "the parent must be fixed-size" (the earlier framing). Wrapper
content size is irrelevant — a scroll_view with content that fits
isn't broken, just not currently scrolling. The single
broken case is `scroll_view.resolved_size == 0`, which happens when
neither (a) an explicit `height` / `min_height` / `max_height` nor
(b) `flex_grow > 0` with a flex_grow chain that reaches a
definite-sized ancestor delivers any space to the scroll_view.

This is **warn + degrade** material per `CLAUDE.md`'s failure-mode
hierarchy: runtime-dependent, context-sensitive, recoverable (the
app keeps running; the scroll_view is just invisible). Detection
goes at the end of `compute_layout` — check each scroll_view, warn
once per element if its resolved size on the scroll axis is zero.
Not implemented yet; noted for follow-up.

## Files touched (summary)

- `common/renderer/src/attrs.rs` — `Overflow` enum, `LayoutAttrs.overflow`,
  removed `DecorationAttrs.clip`.
- `common/renderer/src/setters.rs` — `set_overflow`, `LayoutElement::set_clip`
  with default no-op, removed `DecorationElement::set_clip`,
  removed clip install in `apply_decoration`, added overflow install
  in `apply_layout`.
- `cocoa/dom/src/layout.rs` — wrapper-node allocation in
  `register_in_tree`, redirect helper `taffy_child_parent`,
  removed two-pass logic, `LayoutElement::set_clip` override,
  removed `DecorationElement::set_clip` impl, added
  `child_taffy_parent: Option<NodeId>` to `CocoaMeta`.
- `cocoa/leptos_cocoa/src/cocoa/element.rs` — removed local
  `WithDecoration::clip()` setter; updated `corner_radius` doc.
- `cocoa/leptos_cocoa/src/lib.rs` — re-export `Overflow`.
- `uikit/dom/src/layout.rs` — re-export `set_overflow`; comment
  noting iOS clip isn't wired yet.
- `uikit/leptos_uikit/src/lib.rs` — re-export `Overflow`.
- `gtk/dom/src/layout.rs` — `LayoutElement::set_clip` override
  using `gtk::Widget::set_overflow`; re-export `set_overflow`.
- `gtk/leptos_gtk/src/gtk/decoration.rs` — removed `clip()` shim
  and updated warning text.
- `gtk/leptos_gtk/src/lib.rs` — re-export `Overflow`.
- Nine examples migrated `clip=true` → `overflow=Overflow::Clip`.
- `docs/book/src/layout/{attributes,scroll}.md`,
  `docs/book/src/elements/{stack,color_well}.md`,
  `docs/book/src/platform/ios/deltas.md` — doc updates.
