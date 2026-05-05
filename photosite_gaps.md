# PhotoSite iOS — gaps to build the mockups

Audit of `~/src/photos/PhotoSite/mobile-screens.jsx` against the
current iOS port. Listed in priority order — Tier 1 = blockers
(can't render the mockup at all), Tier 2 = important (workable but
ugly without), Tier 3 = polish.

The mockups define eight screens: `MLibrary`, `MPhotoDetail`,
`MStorage`, `MPendingDelete`, `MCapture`, `MTagEditor`, `MImport`,
`MBackups`, plus a tab bar at the bottom of every screen. The
`ios-frame.jsx` primitives (StatusBar / NavBar / GlassPill /
List / ListRow / Keyboard) are all production-style iOS chrome
that's expected on top of those screens.

---

## Tier 1 — show-stoppers

### Navigation & screen stack

The mockups assume a standard iOS push/pop navigation:
- Library grid → tap photo → Photo detail → back chevron returns.
- Library grid → tap "Pending delete" → MPendingDelete → back.
- Storage → tap a rule → rule edit screen → back.

We have no `<navigation_view>` / `<navigation_link>` builder yet.
Needs:
- [ ] **`UINavigationController` wrapper** as a builder. Pushes
  the rendered children sequentially; back chevron pops.
- [ ] **Programmatic push/pop** — a `Navigator`/`NavigationContext`
  signal-like API: `nav.push(view! { <Detail/> })`,
  `nav.pop()`. Same shape as Leptos web's `Router` but for
  imperative push/pop.
- [ ] **Large-title header** (the iOS 13+ `largeTitleDisplayMode`)
  plus the leading/trailing pill buttons (left "Select", right
  "+ ⊕" / "•••" / "Edit" / etc.). `MTopBar` shows up on every
  screen.

### Tab bar

Every screen ends with `<MTabBar active="library">` (Library /
Albums / Storage / Me). We have no tab-bar concept — just a
single content root. Needs:
- [ ] **`UITabBarController` wrapper** as a builder. Children
  are tab-bar items, each a navigation root.
- [ ] **Tab bar item** with title + (eventually) icon, selected
  state.
- [ ] Content lives between top safe area and tab bar; the tab
  bar sits in the bottom safe area.

### Photo grid

`MLibrary` shows a multi-section, multi-column grid of photos
(3 cols @ "normal" density, 4 @ "tight"). Tapping a cell opens
detail. `MTagEditor`, `MImport`, `MPendingDelete` all use smaller
thumb grids too.

We have flexbox containers but no grid layout. Multi-column
grid via flex requires manual row-wrapping which doesn't recycle
cells.

- [ ] **Taffy grid layout** — enable Taffy's `grid` feature
  (currently `flexbox` only in `ios_dom/Cargo.toml`); add
  `grid_template_columns(...)`, `grid_gap(...)` to the View
  builder. Sufficient for 100s-of-photos screens; for 10000s
  we'd need cell recycling.
- [ ] **`UICollectionView`-backed list builder** (later) — the
  proper iOS grid for large datasets, with cell reuse. Big
  effort. For an initial PhotoSite that holds the user's photo
  library this is probably required eventually.

### Image loading

Every screen has photos. `<image_view>` currently loads from a
local file path via `UIImage::imageWithContentsOfFile:`. That
covers app-bundle assets but not:

- [ ] **Async image loading from a URL** (HTTPS thumbnail URL).
  Needs a fetch on a background thread, decode, set on the
  UIImageView on main. Useful even for local files past a
  certain size since decoding blocks.
- [ ] **`UIImage::imageNamed(...)`** — load named asset from the
  bundle. Currently only path-based.
- [ ] **PhotoKit / Photos framework integration** (PHAsset →
  UIImage) — out of scope for the leptos port (app-level
  concern), but PhotoSite needs it.
- [ ] **Cached thumbnail loader** — keep an LRU cache of decoded
  UIImages keyed by URL/path; otherwise scrolling re-decodes.

### Background colour, border, corner radius on any view

- `MSyncPellet` is a rounded outlined pill with a coloured dot.
- `MStorage`'s rules section has rows with custom borders and
  hatched backgrounds.
- `MPhotoDetail` device chips have rounded outlines.
- `MTagEditor` has solid-fill chips and dashed-outline chips.
- The tab bar and top bar have explicit borders.

We currently can't:
- [ ] **Set a UIView's `backgroundColor`** through a builder
  attribute (we only set the window background once in
  `app.rs`). Needs `.background_color(Color)` on every builder.
- [ ] **Set border radius** (`UIView.layer.cornerRadius`).
  Mockups use radii: 4 (boxes), 6 (callout cards), 8 (chips),
  10 (pills), 12 (chips), 16 (pellets), 26 (list cards).
- [ ] **Set border width + colour** (`UIView.layer.borderWidth` +
  `.borderColor`). Mockup uses 1px / 1.2px / 1.4px / 1.5px
  borders in various places.
- [ ] **Dashed borders** — UIView doesn't do dashed natively;
  needs a `CAShapeLayer` with `lineDashPattern`. Less common
  in real apps; the mockups use it as a sketch convention but
  in production we'd use solid / no border. Could defer.

### Multi-line / styled labels

Every screen has descriptive copy that wraps to multiple lines
(`"Connect to wifi · or tap to use cellular"`, `"34 photos will
be removed from all devices in 30 days. Tap any to restore."`).
Plus headline / body weight contrast.

- [ ] **`UILabel.numberOfLines = 0`** for wrapping. Currently we
  default to 1 line and truncate. Needs a `.lines(n)` /
  `.wrap(true)` builder method.
- [ ] **Line-break / truncation mode** (`.lineBreakMode`) —
  truncate-tail, truncate-middle, word-wrap.
- [ ] **Font weight** (`UIFont.systemFont(ofSize:weight:)`) —
  the mockups use 400 (body), 590 (status-bar), 700 (titles,
  hand-drawn). We expose `font_size` but always pick the
  default (regular) weight.
- [ ] **Italic / bold / mixed-style attributed text** —
  `MPhotoDetail` and `MPendingDelete` mix `<b>` with regular
  text inside one label. Needs `NSAttributedString` support
  on `UILabel` (currently we only `setText:`).
- [ ] **System fonts at sizes** — the iOS port should use
  `.preferredFont(forTextStyle: .body)` and friends so users
  can scale via Dynamic Type. Tier 1 for accessibility.

### Z-stacking / overlays

The photo cells have badges *on top* of the image:
- Star ★ in the top-right corner (`MLibrary` cells).
- "RAW" chip in the top-left (`MPhotoDetail`'s hero).
- A diagonal red line for pending-delete strikethrough.
- Device dots along the bottom edge.

The current builders are pure flexbox — children stack along
one axis. We have no `position: absolute` / `position: relative`
on builder types.

- [ ] **`Position::Absolute`** in the View builder
  (Taffy already supports it; we don't expose it). Combined with
  `top` / `right` / `bottom` / `left` / `inset` insets.
- [ ] **A `<z_stack>`** convenience: a `<view>` whose children
  layer on top of each other (z-order = source order). UIView
  natively renders later subviews on top of earlier ones, so
  this is just a ZStack-style container with all children at
  position absolute / inset 0.

### Aspect-ratio cells

`PhotoBox` is square (w x w). The grid sets cell width based on
column count and forces the cell to be square.

- [ ] **`aspect_ratio` style** (Taffy supports it; not exposed in
  our builders). `.aspect_ratio(1.0)` for square photo cells.

### Tab bar / sticky bottom bar

The tab bar at the bottom of every screen is positioned absolutely
within the screen, with `border-top` and a paper-coloured
background.

- [ ] **Bottom-anchored views** — same as a UITabBar but at the
  builder level: a view that sticks to the bottom safe area,
  not part of the scroll content. Easy with flexbox if the
  parent is a vstack with `flex_grow=1.0` on the scroll
  container above it; need to make sure the tab-bar height +
  safe-area-bottom gets reserved.

### Pull-to-refresh

`MLibrary` and `MCapture` would naturally support pull-to-refresh
to trigger a sync. UIScrollView has a built-in `refreshControl`
property.

- [ ] **`.on_refresh(handler)`** on `<scroll_view>`. Wires up
  `UIRefreshControl`, fires the closure on pull, ends the
  refresh when the user's update completes (probably via a
  signal).

---

## Tier 2 — important UX, but workable around

### Modal / sheet presentation

`MTagEditor` and `MImport` are clearly modal sheets (presented
on top of the underlying screen, `Cancel` / `Done` in the top
bar dismisses). UIKit has half-sheet / page-sheet
presentations.

- [ ] **`.present_sheet(view)`** — push a view modally via
  `UIViewController::present(...)`. iOS 15+ supports
  detents (`UISheetPresentationController`) for half-sheet
  styling.
- [ ] **`Sheet` builder** with `.detents([Medium, Large])`.

We can fake this with a vstack overlay + Taffy absolute
positioning, but losing the iOS-native swipe-down dismissal +
backdrop fade is noticeable.

### Search field

`MTagEditor` has `🔍 Find or add a tag…`. UIKit's
`UISearchBar` / `UISearchController` is the iOS-native idiom.

- [ ] **`<search_bar>`** builder around `UISearchBar`.

A `<text_field>` with a leading icon prefix gets ~80% there.

### Alerts & action sheets

`MPhotoDetail`'s `•••` trailing button typically opens an action
sheet (`UIAlertController.actionSheet`). Confirm-delete dialogs
similarly. We have nothing.

- [ ] **`alert(title, message, actions)`** helper around
  `UIAlertController`. Returns a future / fires a signal when
  the user picks an option.
- [ ] **`action_sheet(title, actions)`** same shape.

### Long-press / context menus

Long-pressing a photo cell in `MLibrary` should reveal a
context menu (Star, Share, Delete, Tag…). UIKit has
`UIContextMenuConfiguration` since iOS 13. Long-press is also
the standard "enter Select mode" gesture.

- [ ] **`on:long_press` event** — UILongPressGestureRecognizer
  fallback similar to the recently-added tap one.
- [ ] **`<context_menu>`** builder — wraps a view so that
  long-pressing it reveals the menu items.

### Pinch-to-zoom & swipe-paged photo viewer

`MPhotoDetail` is the kind of screen where you'd expect to
swipe between adjacent photos and pinch-to-zoom on each.

- [ ] **`UIScrollView` zoom support** — set `minimumZoomScale`,
  `maximumZoomScale`, `zoomingView` delegate. Builder attribute
  on `<scroll_view>`.
- [ ] **Paged scrolling** — `UIScrollView.isPagingEnabled = true`
  for swipe-between-photos; or a proper
  `UIPageViewController`-backed builder.
- [ ] **`on:pinch` event** — UIPinchGestureRecognizer.

### Swipe-to-delete row actions

`MPendingDelete` would benefit from swipe-to-restore /
swipe-to-purge on each pending row. iOS's standard table-view
swipe actions need a proper UITableView/UICollectionView
builder.

- [ ] **`<list>`** builder around `UITableView` /
  `UICollectionView` with row-actions / leading / trailing
  swipe configurations. Big effort. Could fake with manual
  swipe-gesture handling + animated row dismissal in
  the meantime.

### Custom switch / checkbox styling

`MStorage` uses a *checkbox* (square with checkmark, custom
drawn) rather than `<switch>`. `MImport` uses a custom toggle
that's drawn the iOS way but we'd ship UISwitch for that.

- [ ] **`<checkbox>`** builder — square check-box, distinct from
  `<switch>`. UIKit doesn't ship one — typical UIKit apps draw
  it themselves (`UIButton` with two state images, or a custom
  `UIControl`). Add a small `Checkbox` builder under
  `tachys::ios::element` that draws via SF Symbols
  ("checkmark.square" / "square").
- [ ] **`<radio>`** if PhotoSite needs single-select-from-list
  anywhere — currently it doesn't.

### Status bar styling

`IOSStatusBar` defaults to dark glyphs over light background
but the mockup suggests we control it (`dark` prop). UIKit
controls status bar via `UIViewController.preferredStatusBarStyle`
or scene-level `UIStatusBarManager`.

- [ ] **`.status_bar_style(.lightContent | .darkContent)`** on
  the root window or per-screen via a builder hook.

### Custom font / SF Symbols

The mockups in production iOS would use SF Symbols (the system
icon font) for the back chevron, ellipsis, search magnifier,
star, etc. Currently we have no icon support — every glyph
would need to be a Unicode character (the mockups' cheat) or a
bundled image.

- [ ] **`<icon name="chevron.left">`** builder around UIImage's
  `systemSymbol(named:)` (iOS 13+). Returns a UIImageView
  configured with a symbol image. Tintable, scales with text
  via `UIImage.SymbolConfiguration`.

---

## Tier 3 — polish for the "iOS 26 liquid glass" finish

The frame primitives in `ios-frame.jsx` lean on fancy effects.
None are required for a working app but they're what gives the
mockup its distinctive look.

### UIVisualEffectView (blur + saturation)

The `IOSGlassPill`, `IOSKeyboard`, and modern iOS toolbar /
nav-bar backgrounds all use blurred-translucent backgrounds
(`backdropFilter: blur(12px) saturate(180%)` in CSS).

- [ ] **`<blur_view>`** builder around `UIVisualEffectView` with
  one of the system blur styles
  (`.systemUltraThinMaterial`, `.systemMaterial`, etc.). Useful
  for any iOS app that wants stock chrome.

### Shadows

`IOSGlassPill` has `boxShadow: '0 1px 3px rgba(0,0,0,0.07), 0 3px 10px rgba(0,0,0,0.06)'`.
The `IOSDevice` itself has `'0 40px 80px rgba(0,0,0,0.18), 0 0 0 1px rgba(0,0,0,0.12)'`.

- [ ] **Shadow on a UIView** — `layer.shadowColor`,
  `.shadowOpacity`, `.shadowOffset`, `.shadowRadius`, with
  `.layer.masksToBounds = false` and ideally a `.shadowPath`
  for performance. Builder attr `.shadow(...)`.

### Animations

UIKit animates implicitly (e.g. switch toggles, tab transitions).
We don't currently expose `UIView.animate(withDuration:)` — but
since builders set frames via Taffy and aren't wrapped in an
animation block, layout changes happen immediately. For simple
tab/sheet transitions UIKit handles it; for our own (like the
tag-editor sheet sliding up), we'd need explicit animation.

- [ ] **`.transition(...)`** / `.animated(true)` on signal
  changes — wrap the layout-mutation closure in
  `UIView.animate(withDuration: 0.25) { compute_layout(...) }`.

### Haptic feedback

iOS apps tend to fire light haptics on toggle, medium on
delete, etc. `UIImpactFeedbackGenerator`.

- [ ] **`haptic(.light | .medium | .heavy | .selection | .success)`**
  helper called from event handlers.

### Toast / snackbar

`MStorage` quota slider could benefit from a toast confirming
the change. Not in the mockups; nice-to-have.

---

## Tier 4 — backend / out-of-scope (PhotoSite app concerns)

These aren't iOS-port responsibilities but PhotoSite-the-app
will need them:

- PhotoKit access (camera roll → PHAsset).
- Metadata extraction (EXIF, RAW headers).
- Background sync (URLSessionConfiguration backgroundConfig).
- File transfers to NAS (NetService discovery, AFP/SMB).
- Persistence beyond NSUserDefaults (CoreData / SQLite).
- Push notifications.
- Camera capture (AVFoundation).

These belong in a separate PhotoSite app crate, not the leptos
iOS port.

---

## Suggested order to tackle the iOS-port pieces

1. ~~**Background colour + border-radius + border**~~ ✅ DONE
   for `<view>` / `<vstack>` / `<hstack>` (all routes through the
   `View<Children, At>` builder). Reactive `background_color=…`,
   `corner_radius=…`, `border_width=…`, `border_color=…`.
   `Element::set_*` setters in `ios_dom/src/node.rs`. Pulls
   `objc2-quartz-core`'s `CALayer` for `cornerRadius` /
   `borderWidth` / `borderColor`. Verified visually with a
   "sync pellet" card in `examples_ios/controls`. Chrome attrs
   on Button / Label / etc. deferred — `<vstack>` wrapping a
   `<label>` covers the immediate use cases (chips, pellets,
   list cards). The `impl_chrome_attrs!` macro is already in
   place for when those builders are extended.

   Gotcha: `<view>` is in leptos_macro's SVG-tag list, so attrs
   on `<view>` route through `.attr(name, value)` (not
   `.method(value)`). Use `<vstack>` / `<hstack>` instead — they
   aren't in the SVG list.
2. ~~**Position::Absolute + aspect_ratio**~~ ✅ DONE on the
   `<vstack>` / `<hstack>` / view builder.
   - `.aspect_ratio(r: f32)` — Taffy `aspect_ratio`. `1.0` for
     square photo cells.
   - `.position_absolute(true|false)` — Taffy `Position::Absolute`,
     takes the node out of flex flow.
   - `.inset_top(v)` / `.inset_right(v)` / `.inset_bottom(v)` /
     `.inset_left(v)` — anchor offsets for absolute children.
   - `Element::set_aspect_ratio` / `set_position` / `set_inset` in
     `ios_dom/src/layout.rs`.
   - More named adaptive colour constants:
     `Color::SYSTEM_YELLOW` / `SYSTEM_RED` / `SYSTEM_GREEN`.

   Note: positioning is on `View<Children, At>` only — for now,
   wrap a `<label>` in a `<vstack position_absolute=true ...>` to
   anchor it. Adding the same surface to Label / ImageView is
   trivial when needed.

   Layout caveat: `aspect_ratio` on a flex item with no explicit
   width and `align-items: stretch` (vstack default) makes the
   item take full cross-axis size, which can swallow other
   children in the parent. Set explicit `width` + `height`
   instead, OR wait for grid layout (item 6) which makes
   `aspect_ratio` cells natural. Documented in
   `examples_ios/controls`.

   Z-stack pattern: a vstack/hstack whose children include one
   plain child (the background) plus one or more
   `position_absolute=true` children (the overlays) gives you
   layered rendering. UIView renders later subviews on top of
   earlier ones, so source order = z-order. No separate `<z_stack>`
   builder needed.
3. **Multi-line labels + font weight + line-break mode**.
   Unlocks every long copy block.
4. **Tab bar wrapper** (UITabBarController). Unlocks the bottom
   nav across all screens.
5. **Navigation controller wrapper + push/pop API + large
   title**. Unlocks screen-to-screen flow.
6. **Taffy grid layout + grid_template_columns**. Unlocks the
   photo grid layout (small library — under a few hundred
   photos — works fine without cell recycling).
7. **Pull-to-refresh** — small win, unlocks the sync gesture.
8. **on:long_press + context menu**. Unlocks "select mode" entry.
9. **Action sheet / alert helpers**. Unlocks the `•••` button.
10. **Modal sheet builder**. Unlocks `MTagEditor` / `MImport`.
11. **Async image loading + cache**. Unlocks photos beyond the
    bundle.
12. **`<list>` builder around UICollectionView**. Unlocks
    swipe-to-delete + scales the photo grid past a few
    thousand items.
13. **SF Symbols / icon builder**. Unlocks proper iOS glyphs.
14. **Pinch-to-zoom + paged scroll for photo detail**.
15. **Visual effect (blur), shadows, haptics, animations** —
    polish layer.

That's the realistic build order. Items 1–6 alone get you a
functional but plain version of every screen; 7–10 get the iOS
feel; 11–13 make it production-shaped; 14–15 are the iOS 26
chrome.
