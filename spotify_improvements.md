# UI library improvements

Notes from building `cocoa/examples/spotify` (Spotify desktop UI) and
`cocoa/examples/pages` (Apple Pages document editor) as stress-tests
of the macOS port. Each mockup approximates the reference UI using
only library primitives. Library gaps surfaced here are either fixed
in-branch or documented as TODOs.

## Improvements made

### Cross-cutting (shared `common/`)

- **`<Switch>` + `<Match>` control-flow components.**
  `common/leptos/src/switch.rs`. First-match branching, 1..=8 arms via
  a `SwitchBranches` trait. Each `<Match>` is `transparent` and yields
  a typed `Match<W, C, R>`; `<Switch>` walks the tuple and renders the
  first arm whose `when()` is true via `EitherOf{N}` internally — no
  boxing, no `AnyView`. The "add another arm" path is "add another
  `<Match>`," not "switch to `EitherOf4`."

- **`AddAnyAttr<R>` for `EitherOf3..8`.**
  `common/renderer/src/view/add_any_attr.rs`. `Either<A, B>` already
  had a panic-on-spread impl; the rest of the `EitherOf{N}` family
  did not, which broke `IntoView` for `<Switch>`'s output.

- **`Edges` value type for `padding` / `margin`.**
  `common/renderer/src/attrs.rs`. `LayoutAttrs::padding` /
  `margin` are now `Option<MaybeReactive<Edges>>`. Existing
  `padding=8.0` call sites keep working via `From<f32> for Edges`;
  new shorthands are `padding=(h, v)` (axis pair) and `padding=(t, r,
  b, l)` (explicit per-side), plus builder form
  `Edges::ZERO.with_top(8.0).with_left(12.0)`. Pages's per-edge
  inspector inset (`Edges::trbl(48, 60, 60, 60)` on the canvas
  page) exercises this.

- **`size(n)` on `WithLayout`.**
  One method that sets `width=height=min_width=min_height=n`. Stack
  has an inherent override that also pins `flex_shrink=0`, so a
  long-titled sibling can't squeeze it (the case that broke the
  Spotify sidebar cover art). Replaces the five-method
  `.width(n).height(n).min_width(n).min_height(n).shrink(0.0)`
  incantation.

### macOS port

- **`background_color`, `corner_radius`, `border_width`,
  `border_color` on `<Stack>` / `<View>` / `<Button>`.**
  Layer-backed via CALayer. Border width and color are independent
  setters (no shared cached state). Buttons with any of these set
  auto-flip `bordered=false` so the system bezel doesn't fight the
  custom paint.

- **`set_corner_radius` no longer auto-enables `masksToBounds`.**
  Was the cause of the visible chip-title clipping in v1 — corner
  radius was implying a layer mask, which clipped the button's
  title text. Now `set_corner_radius` only sets the radius;
  `setMasksToBounds` is separate and exposed as `clip=true`. The
  CALayer's `backgroundColor` honors `cornerRadius` by itself, so
  rounded backgrounds look correct without masking; only children
  that need to clip to the rounded shape require the explicit
  `clip`.

- **`bold` and `line_break` on `<Label>`.**
  `set_bold` reads the current font's point size and replaces with
  `boldSystemFontOfSize:`; `set_font_size` preserves bold. The new
  `line_break` attr accepts `cocoa_dom::LineBreak` (`WORD_WRAPPING`
  / `CHAR_WRAPPING` / `CLIP` / `TRUNCATE_HEAD` / `_TAIL` /
  `_MIDDLE`). `multiline=true|false` is kept as shorthand for
  `WORD_WRAPPING` / `TRUNCATE_TAIL`.

- **`text_color` and `bold` on `<Button>`.**
  `set_button_title_color` uses `NSButton.contentTintColor` (no
  `attributedTitle` round-trip needed). Bold uses the same
  size-preserving path as labels.

- **`Mountable::insert_before_this` actually inserts now.**
  `cocoa/leptos_cocoa/src/renderer_cocoa.rs` +
  `cocoa/leptos_cocoa/src/cocoa/element.rs`. Was previously
  hardcoded `false` on `Element`/`Node`/`Text`/`Placeholder`/
  `ElementState`, which silently broke `<Switch>` / `Either`
  variant transitions (parent rendered blank after first switch).
  The fix delegates to the pre-existing
  `synthesise_parent_element` helper that
  `Dom::try_mount_before` already used; the four leaf impls now
  share an `insert_before_node(before, child)` helper.

- **`install!` macro inside `Render::build` bodies.**
  Compresses the `if let Some(v) = self.foo { let e = el.clone();
  if let Some(eff) = install(v, move |x| setter(e.as_node(), x)) {
  effects.push(eff); }}` boilerplate to one line. Used throughout
  Stack and Button's build now.

- **Prelude exposes `Dim`, `Edges`, `AlignSelf`, `FlexWrap`,
  `LineBreak`, `TextAlignment`.**

### Examples

- **`cocoa/examples/spotify`** — Spotify desktop mockup. Home /
  Playlist / Artist views switching via `<Switch>` on the sidebar
  click. Stress-tests `Edges`, `corner_radius`, `bold`,
  `background_color`, the `size()` helper, padded chips.
- **`cocoa/examples/pages`** — Apple Pages document editor.
  Toolbar + canvas + Document/Format toggle sidebar via `<Switch>`.
  Stress-tests per-edge canvas insets, label `line_break`, hairline
  dividers as 1-px stacks, and segmented controls built from
  `<button>`s rather than NSSegmentedControl (gives full styling
  control).

## Partially-implemented: NSSplitView-backed sidebar

`cocoa/dom/src/split_window.rs` + `mount_to_split_window` builds
a window whose content view is a native `NSSplitView` with two
panes — main + inspector — each rooted in its own Taffy tree.
`set_inspector_collapsed(true/false)` slides the divider closed
or open via `setPosition:ofDividerAtIndex:`. The infrastructure
**works at the AppKit level**: the split view, divider, and panes
are all created correctly, and the logical layout
(`arrangedSubviews[0]` at x=0..800, `[1]` at x=801..1100) matches
expectations.

What didn't work: the **visual rendering of pane positions**.
After `setPosition`, the split view's arranged-subview frames
report correct origins, but our flipped-coordinate FlippedView
inside the inspector pane renders its content at the wrong screen
x. Suspect: Taffy's `apply_frames` pass resets the pane root's
frame origin during `compute_layout`; the `recompute_pane()` helper
intended to restore the origin runs but doesn't fix the rendering
in practice. Suspect interaction with our coordinate-flipped
content trees.

The Pages mockup currently uses the simpler in-layout sidebar
(`<hstack flex_grow=1.0><Canvas /><Sidebar /></hstack>` with
`hidden=signal` driving collapse). It looks correct and the toggle
works; the sidebar just doesn't slide. The NSSplitView code is
kept for future iteration once the coordinate interaction is
understood.

To pick this up later, the path is probably:
1. Add `eprintln!` traces to `set_frame_from_layout` and walk the
   logged screen coords vs Taffy-local coords for the inspector
   pane on first layout. The discrepancy will reveal where the
   coordinates are getting reset.
2. Likely fix: have `compute_layout` skip the root node's
   `setFrame` (only apply frames to children), since the root's
   frame is owned by NSSplitView.

## Recommended improvements (not done)

These are real shortcomings each mockup hit but didn't block the
demo. Listed in rough cost/value order.

### Per-edge borders

`border_width` is a single value (matches CALayer.borderWidth).
Apple Pages's toolbar has a 0.5-pt hairline along just the bottom
edge; we work around it today with a 1-px-tall `<vstack>` sibling.
A real per-edge border would need a `CAShapeLayer` overlay or four
sublayer strips. Workaround works for hairlines; gets ugly fast for
asymmetric thick borders (e.g. focus rings on one side only).

### Linear-gradient / multi-stop backgrounds

Spotify's playlist hero and Pages's title bar use vertical
gradients. Today we render flat colors. CAGradientLayer can do
this; the wrinkle is keeping the sublayer's bounds in sync with the
view's frame (Taffy `setFrame:`s the view, the gradient sublayer
needs to follow). Options: (a) NSView subclass that uses
CAGradientLayer as its layer class; (b) post-`setFrame:` hook that
resizes a tagged sublayer. Skipped for now.

### Real image / asset support

Album art (spotify) and the photo placeholder (pages) are faked
with colored vstacks + a glyph. `<image_view>` takes a file path
only — nothing for embedded `&[u8]`, network URLs, SVG, or a
hash-tinted placeholder. Adding `<image_view source=&[u8]>` via
`NSImage::initWithData:` is the smallest useful step.

### Stack `on:click` / hover events

Pages's toolbar buttons today are NSButtons stacked with custom
text-color; a proper version would want hover-state styling.
Spotify's sidebar rows would benefit from `:hover` background
changes. Both need `on:mouseenter`/`on:mouseleave` on containers,
which requires installing an `NSTrackingArea` on the underlying
NSView. Same plumbing would let `<vstack on:click=...>` work for
custom tap targets.

### Numeric stepper / `NSStepper`-flavored fields

The Pages mockup fakes its margin/header fields with hardcoded
strings + tiny ▴/▾ glyphs. `<stepper>` already exists for raw
numbers; what's missing is the combined "labeled inset + stepper +
unit suffix" pattern (e.g. `"2.54 cm"` with a stepper that updates
just the number). A `<measure_field unit="cm">` builder would
encapsulate this and is one of the few real Pages-specific
additions.

### `<text_field>` placeholder color

Spotify's top-bar search needs a slightly-lighter placeholder than
NSTextField's default. `placeholderAttributedString` can tune it;
add `placeholder_color` on TextField.

### Lazy `<Match>` children

`<Switch>` calls every `<Match>`'s children-closure once at build
time (to capture into `Arc<dyn Fn>`). For deeply expensive
sub-trees in non-active arms this allocates state we never see.
A `lazy=true` mode that defers children construction to first
selection would scale better. Not yet a real problem.

### `<MatchDefault>` arm

If no `<Match>` matches, `<Switch>` renders nothing. A
`<MatchDefault>` arm would render-as-fallback. Easy add; the
trait already returns `Option<EitherOf{N}>`.

## Architectural observations

- **`insert_before_this` returning `false` everywhere was a
  long-latent bug.** Surfaced the moment any control-flow primitive
  needed to swap subtrees in place. None of the pre-existing
  examples did. Both Switch-based mockups exercise this path, so
  the regression test surface is good now.

- **Bordered NSButton + custom layer paint is fragile.** The
  intrinsic content size accounts for the bezel even with
  `bordered=false`; small / no `padding` makes the title look
  cramped or visually clipped at rounded corners. Pattern that
  worked in both mockups: `bordered=false`, explicit `padding`
  (uniform or `Edges::xy(h, v)`), and `corner_radius` ≤ half the
  resulting button height. A custom `<chip>` builder that captures
  this convention would save call sites from getting it wrong.

- **The `Edges` value type generalizes cleanly.** Same shape would
  work for borders (per-edge width) or sizing constraints
  (per-edge padding-aware constraints). Worth keeping in mind if
  per-edge borders ever become a real requirement.

- **Adding a feature now usually touches one place.** `LineBreak`
  was a new `objc_enum` newtype + `impl_pair!` line + one `pub use`
  + Label builder method. `Edges` was one struct + one setter
  signature change + `From` impls. The renderer-common abstractions
  (Edges, LayoutAttrs, install!) hold up well under the demands of
  these two visually-different mockups.
