# Cocoa API review

A review of the public API surface that downstream apps target when they
write `leptos = { package = "leptos_cocoa" }` and `use leptos::prelude::*;`.

Scope: macOS only. The iOS and GTK ports mirror the same shape and most
of the recommendations below apply equally to them, but the audit was
done against the cocoa port because it is the most mature.

This is split into three sections:

1. **API surface** — what an app gets when it imports the prelude
2. **Critique** — what works, what doesn't
3. **Recommendations** — prioritized list of changes to consider before
   stabilizing

---

## 1. API surface

### 1.1 Entry points (mounting)

From `leptos::prelude::*`:

```rust
pub fn run<F, V>(f: F)                              // multi-window or custom
pub fn mount_to_window<F, V>(title: &str, size: (f64, f64), f: F)
pub fn mount_to_split_window<F, V, P>(title: &str, size: (f64, f64), f: F)
```

Plus the bare-builder form via `leptos::tachys::cocoa::window::window()`
for the `run(|| (window().title(..).child(..), window()..))` multi-window
pattern.

All three block until the app terminates. `Owner` and view state are
deliberately leaked (process lifetime == owner lifetime).

### 1.2 Element builders (the `tachys::html::element::*` and prelude set)

Containers (`Render<Dom>` impls that take children, no built-in event hooks):

| Builder           | Tag                  | Backing            |
|-------------------|----------------------|--------------------|
| `vstack()`        | `<vstack>`           | FlippedView, flex column |
| `hstack()`        | `<hstack>`           | FlippedView, flex row    |
| `view()`          | `<view>`             | FlippedView, flex column (no direction preset; alias of `stack()`) |
| `stack()`         | `<stack>`            | FlippedView, flex column |
| `stack_view()`    | `<stack_view>`       | Legacy alias of `vstack()` |
| `grid()`          | `<grid>`             | FlippedView, Taffy CSS grid |
| `scroll_view()`   | `<scroll_view>`      | NSScrollView + FlippedView documentView |
| `split_view()`    | `<split_view>`       | NSSplitViewController (*via* `mount_to_split_window` only) |
| `split_pane()`    | `<split_pane>`       | NSSplitViewItem + FlippedView |
| `window()`        | n/a (bare builder)   | NSWindow            |

Leaves (`Render<Dom>` impls that don't take children):

| Builder                 | Tag                    | Backing            |
|-------------------------|------------------------|--------------------|
| `button()`              | `<button>`             | NSButton           |
| `checkbox()`            | `<checkbox>`           | NSButton (switch-style) |
| `label()`               | `<label>`              | NSTextField (non-editable) |
| `text_field()`          | `<text_field>`         | NSTextField        |
| `secure_text_field()`   | `<secure_text_field>`  | NSSecureTextField  |
| `text_view()`           | `<text_view>`          | NSScrollView wrapping NSTextView |
| `slider()`              | `<slider>`             | NSSlider           |
| `stepper()`             | `<stepper>`            | NSStepper          |
| `pop_up_button()`       | `<pop_up_button>`      | NSPopUpButton      |
| `segmented_control()`   | `<segmented_control>`  | NSSegmentedControl |
| `date_picker()`         | `<date_picker>`        | NSDatePicker       |
| `color_well()`          | `<color_well>`         | NSColorWell        |
| `progress_indicator()`  | `<progress_indicator>` | NSProgressIndicator (bar or spinner) |
| `image_view()`          | `<image_view>`         | NSImageView        |

### 1.3 Attribute traits (chainable setters)

All builders implement `WithLayout` and `WithUniversal`. Most leaves
also implement `WithText`. The trait methods supply chainable setters:

**`WithLayout`** — `padding`, `margin`, `width`, `height`, `min_width`,
`min_height`, `max_width`, `max_height`, `flex_grow`, `flex_shrink`,
`flex_basis`, `align_self`, `size(n)` (forces n×n + non-shrinkable),
`grid_column_start`, `grid_column_end`, `grid_row_start`,
`grid_row_end`, `grid_column((s, e))`, `grid_row((s, e))`,
`grid_column_at(n)`, `grid_row_at(n)`, `grid_column_span(n)`,
`grid_row_span(n)`.

**`WithUniversal`** — `alpha`, `tool_tip`.

**`WithText`** (cocoa-local, generic over `Color` / `TextAlignment`) —
`text_color`, `alignment`, `font_size`.

### 1.4 Per-builder inherent methods

These appear on individual builders, **not** through a shared trait:

- **`Stack`** — `direction`, `gap`, `justify_content`, `align`,
  `align_content`, `justify_items`, `wrap`, `background_color`,
  `corner_radius`, `border_width`, `border_color`, `clip`, `hidden`,
  `child(c)`.
- **`Grid`** — same `_content`/`_items`/`background_color`/`clip` group,
  plus `columns`, `rows`, `auto_columns`, `auto_rows`, `auto_flow`,
  `gap`, `column_gap`, `row_gap`, `child(c)`.
- **`ScrollView`** — `autohides_scrollers`, `has_horizontal_scroller`,
  `has_vertical_scroller`, `child(c)`.
- **`Button`** — `title`, `enabled`, `on_click`, `bordered`,
  `key_equivalent`, `background_color`, `corner_radius`, `border_width`,
  `border_color`, `text_color`, `bold`, `child(text)`, `on(event, fn)`,
  `node_ref`, `directive`.
- **`Checkbox`** — `title`, `checked`, `child(text)`, `on`, `node_ref`,
  `directive`. (`bind:checked=signal` via `BindAttribute`.)
- **`Label`** — `text`, `child(text)`, `bold`, `line_break`, `multiline`,
  `selectable`, `on`, `node_ref`, `directive`.
- **`TextField`** (and `secure_text_field`) — `value`, `placeholder`,
  `enabled`, `bordered`, `bezeled`, `on`, `node_ref`, `directive`.
- **`TextView`** — `value`, `enabled`, `node_ref`, `directive`.
- **`Slider`** — `value`, `min_value`, `max_value`, `enabled`, `vertical`,
  `num_tick_marks`, `snaps_to_ticks`, `on`, `node_ref`, `directive`.
- **`Stepper`** — `value`, `min_value`, `max_value`, `increment`,
  `enabled`, `on`, `node_ref`, `directive`.
- **`PopUpButton`** — `items`, `selection`, `enabled`, `pulls_down`, `on`,
  `node_ref`, `directive`.
- **`SegmentedControl`** — `items`, `selection`, `enabled`,
  `segment_style`, `on`, `node_ref`, `directive`.
- **`DatePicker`** — `value`, `enabled`, `style`, `min_date`, `max_date`,
  `on`, `node_ref`, `directive`.
- **`ColorWell`** — `value`, `enabled`, `on`, `node_ref`, `directive`.
- **`ProgressIndicator`** — `value`, `max_value`, `indeterminate`,
  `displayed_when_stopped`, `node_ref`, `directive`.
- **`ImageView`** — `source` (file-path string), `node_ref`, `directive`.
- **`SplitPane`** — `behavior` (`PaneBehavior::{Default,Sidebar,Inspector}`),
  `preferred_thickness`, `minimum_thickness`, `maximum_thickness`,
  `holding_priority`, `can_collapse`, `collapsed`, `child(c)`.
- **`SplitView`** — `vertical`, `child(c)`.
- **`Window`** — `title`, `size`, `child(c)`.

### 1.5 Events (`on:event=handler`)

| Event       | Payload                | Supported on |
|-------------|------------------------|--------------|
| `click`     | `()`                   | Button, Checkbox, Label, Slider, Stepper, PopUpButton, SegmentedControl, DatePicker, ColorWell |
| `input`     | `String`               | TextField, SecureTextField |
| `change`    | `String`               | TextField, SecureTextField |
| `focus`     | `()`                   | TextField, SecureTextField |
| `blur`      | `()`                   | TextField, SecureTextField |
| `keydown`   | `cocoa_dom::KeyEvent`  | TextField, SecureTextField |
| `keyup`     | `cocoa_dom::KeyEvent`  | TextField, SecureTextField |

`SupportsEvent<E>` is the compile-time gating trait. Two `on:click=`
handlers on the same NSControl is a runtime panic at build time
(NSControl has one target/action slot).

### 1.6 Two-way binding (`bind:foo=signal`)

`BindAttribute<Key, Sig>` impls + `IntoSignal<T>` trait. Provided shapes
on the signal side: `RwSignal<T>` and `(getter_fn, setter_fn)` tuples.

| Bind key      | Value type         | Controls                                       |
|---------------|--------------------|------------------------------------------------|
| `bind:value`  | `String`           | TextField, SecureTextField, TextView, Label (read-only sink) |
| `bind:value`  | `f64`              | Slider, Stepper                                |
| `bind:value`  | `cocoa_dom::Date`  | DatePicker                                     |
| `bind:value`  | `cocoa_dom::Color` | ColorWell                                      |
| `bind:checked`| `bool`             | Checkbox                                       |
| `bind:selection` | `usize`         | PopUpButton, SegmentedControl                  |

### 1.7 Style / layout enums (re-exported by the prelude)

`AlignItems`, `AlignContent`, `JustifyContent`, `JustifyItems`,
`FlexDirection`, `FlexWrap`, `GridAutoFlow`, `GridTemplateComponent`,
`TrackSizingFunction` — re-exported from Taffy via the renderer crate.

`AlignSelf`, `Dim` (`Px` / `Pct` / `Auto`), `Edges`, `GridLine` —
renderer-agnostic wrappers, also re-exported.

Grid track helpers: `fr`, `length`, `auto`, `percent`, `min_content`,
`max_content`, `minmax`, `fit_content`, `repeat`. Plus `span(n)` and
`auto_line()`.

### 1.8 Cocoa value types

`Color` (sRGB rgba 0..=1, helpers `rgb`, `rgba`, constants `WHITE`,
`BLACK`, `RED`, `GREEN`, `BLUE`), `Date` (`now()`, `seconds_since_epoch`
field), `KeyEvent` (`key: String`, mapped to web names like
`"Enter"`/`"Escape"`/`"ArrowUp"`), `TextAlignment` (LEFT, RIGHT, CENTER,
JUSTIFIED, NATURAL), `LineBreak` (WORD_WRAP, TRUNCATE_HEAD/TAIL/MIDDLE,
CLIP), `SegmentStyle`, `DatePickerStyle`.

### 1.9 Utilities

- `NodeRef::new()` → pass via `node_ref=ref`, then `ref.get()` /
  `ref.get_untracked()` / `ref.on_load(|el| …)`. Returns
  `Option<cocoa_dom::Element>`. Element exposes imperative methods like
  `el.focus()`.
- `local_storage()` → `Result<Option<Storage>, _>` over `NSUserDefaults`.
- `set_interval(f, Duration)` and `set_interval_with_handle(f, Duration)`.
- `use:directive=fn` (or `use:directive=(fn, param)`) — fires once at
  `Render::build` with the underlying `cocoa_dom::Element`.

### 1.10 Reactive core (re-exported from `leptos`)

`RwSignal`, `signal()` (split read/write), `Signal::derive`, `Memo`,
`Effect`, `RenderEffect`, `provide_context` / `use_context`, the
`<Provider>` component, `Resource` / `LocalResource` (untyped — works
because the user spawns their own runtime, see `fetch` example),
`#[component]`, `#[slot]`, `view!{}`, `<Show>`, `<ShowLet>`, `<For>`,
`<Switch>` / `<Match>`, `<ErrorBoundary>`, `<Transition>`, `Suspend`.
`IntoView` is pre-specialized to `Dom`.

---

## 2. Critique

### 2.1 What works well

- **The view! macro feels right.** Examples are remarkably readable —
  `vstack { hstack { button on:click=… } }` reads almost like SwiftUI
  with rust syntax, while preserving the upstream Leptos macro and the
  reactive-graph machinery wholesale. The decision to invent
  `<vstack>` / `<hstack>` / `<view>` rather than reuse HTML tags pays
  off in the showcase / spotify / pages examples.
- **`MaybeReactive<T>` + `IntoMaybeReactive<T>`.** Letting attribute
  setters accept either a bare value or a `Fn() -> T` closure makes the
  static/reactive distinction invisible at the call site. `padding=8.0`
  and `padding=move || s.get()` cost the same syntactically; the
  underlying type machinery hides the divergence.
- **`WithLayout` / `WithUniversal` traits.** Adding `padding`, `margin`,
  `flex_grow`, `grid_column` etc. to every builder in one trait edit
  (vs N builder edits) is the right factoring. Same shape for
  `WithText`.
- **`bind:`.** The `RwSignal<T>` ↔ control wiring "just works" for
  every control whose state has an obvious type. The fact that the
  same `bind:value=signal` works on text field, slider, color well,
  text view, date picker is a clear win.
- **`Color`, `Edges`, `Dim`, `GridLine`** are well-designed value
  types — typed enough to catch usage errors, with sensible `From`
  impls (`f32 → Edges::all`, `i32 → GridLine::Line`) that keep the
  call sites compact.
- **Compile-time event/control gating.** `<text_field on:click=…>`
  doesn't compile (no `SupportsEvent<ClickEvent> for TextField`).
  Same for two `on:click` handlers — runtime panic at build time.
  Failure-mode hierarchy from `CLAUDE.md` is followed consistently.
- **Multi-window via `run(|| (window, window))`.** The fact that this
  composes cleanly from tuples is elegant — no special-case API.
- **Examples are exhaustive.** 23+ examples covering every major
  pattern (forms, dynamic lists, stores, persistence, fetch,
  transitions, slots, split-view, multi-window, scroll, grid). They
  serve as both regression tests and de facto API documentation.

### 2.2 Inconsistencies and rough edges

#### 2.2.1 Stack vs Grid have different inherent methods

`Stack` has `direction`, `gap`, `justify_content`, `align` (singular),
`align_content`, `justify_items`, `wrap`, `background_color`,
`corner_radius`, `border_width`, `border_color`, `clip`, `hidden`.

`Grid` has `gap`, `column_gap`, `row_gap`, `auto_flow`,
`justify_items`, `align_items` (note: not `align`), `justify_content`,
`align_content`, `background_color`, `clip`, plus track helpers — but
**no `corner_radius`, `border_width`, `border_color`, or `hidden`**.

This means `<vstack corner_radius=4.0 ...>` works, but
`<grid corner_radius=4.0 ...>` is a method-not-found error. There's no
principled reason; `Grid` was just added later and got the subset that
existed at the time.

Stack also calls cross-axis alignment `align` while Grid calls it
`align_items`. Same Taffy property, different name.

#### 2.2.2 `Button` carries a lot of non-trait-shared style attrs

Look at Button: `background_color`, `corner_radius`, `border_width`,
`border_color`, `text_color`, `bold` all live as inherent methods,
duplicating Stack's set. So does ScrollView (missing them entirely),
Label (has `bold`), and Grid (has `background_color` and `clip` only).

`background_color`, `corner_radius`, `border_width`, `border_color`,
and `clip` are universal "rectangle styling" attributes — they should
live on a shared `WithDecoration` (or `WithBackground`) trait alongside
`WithLayout` and `WithUniversal`, and every builder should get them
for free. Today, layering rounded backgrounds onto unsupported
builders (text fields, sliders, segmented controls) needs wrapping
in a `<vstack background_color=…>`, which is a real workaround in
example code.

`bold` and `text_color` already live on `WithText` (the trait); Button
duplicates them as inherent methods, which is fine but redundant.

#### 2.2.3 `hidden` is mostly broken; `<Show>` is the workaround

`Stack::hidden(true)` calls NSView's `isHidden`, but Taffy still
reserves space for the slot. The doc comment says so explicitly —
"pair with `width=0` (or rebuild via `<Switch>`) to actually remove
the slot". Most users want display-none semantics, not visibility-
hidden. `<Show>` / `<Switch>` is what they reach for.

This makes `hidden=` a footgun — looks right, behaves wrong. Either
remove it, or change its semantics so it sets `display: none` in Taffy
(then there's no need for `<Show>` for the simplest hide/show case).

#### 2.2.4 Naming/aliasing soup for "make a flex container"

There are five spellings for "vertical stack":
- `vstack()` (defaults direction=Column)
- `stack()` (no preset; defaults to Column via `Render::build`)
- `view()` (no preset; same as `stack()`)
- `stack_view()` ("Legacy alias of `vstack()`")
- `<stack>` (the tag name)

Plus `<view>`, the SwiftUI-flavored generic-container tag. The
internal Stack struct backs all six. This is mostly historical
accumulation — keeping `stack_view()` for source compat with old
code, keeping `<stack>` because of the macro routing through
`tachys::html::element::*`. From a clean-slate API perspective,
three names is two too many.

#### 2.2.5 `child(...)` overloads are inconsistent

- `Button::child(value)` and `Label::child(value)` and
  `Checkbox::child(value)` all take `IntoMaybeReactive<String>` and
  set the title/text. They explicitly **cannot** take other Renders.
- `Stack::child(c)` / `Grid::child(c)` / `ScrollView::child(c)` /
  `SplitView::child(c)` / `SplitPane::child(c)` / `Window::child(c)`
  take any `Render<Dom>`.

That's fine in practice for the macro, but it means writing
`<label>{move || ...some_complex_view}</label>` is a type error.
Users have to remember "for `Result<T, E>` use `<view>`, not
`<label>`" (called out explicitly in `CLAUDE.md`'s gotchas).

#### 2.2.6 `selection` vs `value` for binding

PopUpButton and SegmentedControl use `bind:selection=`, while every
other bind uses `bind:value=`. Looking at the keys module, `Selection`
is a separate marker type that only lives in cocoa (`Value` and
`Checked` are shared via `apple_shared::attr_keys`).

The distinction is arguably useful — selection is a different concept
from value — but downstream, "what do I bind to a popup?" is one more
thing users have to look up. Standardizing on `bind:value=` for
indices (i.e. typing the popup's value as `usize`) would shrink the
mental model.

#### 2.2.7 Static-only attrs

A few attrs deliberately don't accept reactive forms, with comments
explaining why:
- `TextField::placeholder` takes `impl Into<String>` only (no
  `IntoMaybeReactive`).
- `Slider::min_value` / `max_value` are bare `f64` (also Stepper's
  `min_value` / `max_value` / `increment`).
- `Grid::columns` / `rows` / `auto_columns` / `auto_rows` take static
  Vecs.
- `WithLayout::size(n)` is static-only (it sets five sub-attrs and
  reactive would need closure cloning).
- `Window::title` is one-shot at build; rebuild is documented as no-op.

These are individually reasonable but the resulting "is this attr
reactive?" rules are inconsistent across the builder surface. From a
user perspective: "everything is reactive, except occasionally not"
is harder to remember than "everything is reactive" with an explicit
escape hatch.

#### 2.2.8 No image input besides file path

`ImageView::source(path)` only accepts a filesystem path string. The
doc note is honest about this — "Network URLs aren't supported here
— fetch them yourself … and write to a temp file". For a port that
wants to compete with SwiftUI ergonomics, this is a notable gap.
There's no `Image::from_data(&[u8])`, no
`Image::from_url("https://...")`, no SF Symbol support, no system-
icon convenience.

#### 2.2.9 `mount_to_split_window` API is heavy

It's the only entry point that returns a non-`Render` view — the
closure must yield a `<split_view>` builder. The `IntoSplitView`
trait adapts the `view!{}`-wrapped case, but the user-facing
constraint "you have to use *this* mount function and not the
regular one" is a wart. Three mount functions for three
fundamentally similar tasks (open a window, run the loop, mount
content) is overkill.

A cleaner shape: `<split_view>` and `<split_pane>` are just regular
elements that can sit anywhere inside a `<window>`. The fact that
NSSplitViewController needs to be the window's `contentViewController`
is an implementation detail the framework should hide.

#### 2.2.10 Color-well + bind:value mixes types

`bind:value=color_signal` works on ColorWell with
`Signal<cocoa_dom::Color>`. Two issues:
1. The `Color` type lives in `cocoa_dom`, leaking the implementation
   crate's name into user code (`use cocoa_dom::Color` shows up in
   spotify/pages examples). Should be re-exported from
   `leptos::prelude::Color` only — actually it is, but examples
   habitually import from `cocoa_dom` directly because it shows up
   in autocomplete first.
2. Same for `cocoa_dom::Date`, `cocoa_dom::TextAlignment`,
   `cocoa_dom::LineBreak`, `cocoa_dom::KeyEvent`. All re-exported in
   prelude, but example code reaches for the underlying path. The
   imports leak the layering.

#### 2.2.11 Window lifecycle is incomplete

- `Window::rebuild` is a no-op — changing title/size reactively is
  not supported.
- `Owner` is intentionally leaked at `run` time, which means
  per-window owner scoping isn't really a thing. Closing a window
  cleans up its mount tree (via the windowWillClose handler) but
  doesn't drop the reactive context that the window opened in.
- There's no programmatic close API surfaced on `Window` builders;
  you'd need a NodeRef on the window's content_root and an
  imperative call (which isn't on the public Element API anyway).
- "Cmd-Q closes the whole app" is the only termination model — no
  `App::terminate()`, no `Window::close()`.

#### 2.2.12 No menu / menubar API

Examples that need real macOS app polish (Cmd-N for new window,
File / Edit menus, Quit handler) have no way to do it. `init_app`
sets up a default menu bar internally but doesn't expose a builder.
For a "production macOS app" story this is a significant gap.

#### 2.2.13 No drag & drop, clipboard, accessibility, or printing APIs

These are real macOS-app features. None are exposed.

#### 2.2.14 No animation primitives

CoreAnimation is right there, but Taffy + the manual relayout
pipeline means style changes snap rather than animate. `<Transition>`
exists for *content* swaps via reactive lifetimes, but there's no
"animate this `corner_radius` over 200ms" API. SwiftUI's `.animation()`
modifier is the obvious target.

#### 2.2.15 `directive(...)` and `use:directive` are not discoverable

The directive escape hatch is documented per-builder but there's no
example of writing a directive from scratch. The `IntoDirective` trait
is re-exported from `apple_shared` but the prelude doesn't even pull
it in; users have to find `leptos::tachys::html::directive::*` or
`leptos_cocoa::directive::IntoDirective`.

#### 2.2.16 The `bind:value=` two-way binding silently accepts label

`Label` has a `BindAttribute<Value, _>` impl that's documented as
"read-only sink". It works (binds the text to a signal getter) but
the name `bind:` implies two-way. A user might expect editing the
label's text would write back to the signal — but labels aren't
editable at all. Either remove this impl (force users to write
`<label>{move || sig.get()}</label>`) or rename the concept.

#### 2.2.17 No way to set window position / persist window state

Windows always open at AppKit's default position. There's no
`x, y` argument, no "remember last position" helper, no
NSWindowRestoration integration. For a port that wants to compete
on macOS-native feel, this is missing.

#### 2.2.18 `tachys::cocoa::*` and `tachys::html::element::*` are both reachable

The macro emits paths through `tachys::html::element::*`, but the
prelude also exposes a `tachys::cocoa::element::*` re-export, and
the `counter_without_macros` example uses both:

```rust
use leptos::tachys::{
    cocoa::element::{button, label, vstack},
    html::event::click,
};
```

Two import paths for the same set of functions is dual-API surface
that doesn't need to exist.

### 2.3 Hidden complexity that user code has to know about

These are things that the docs / CLAUDE.md call out, but a fresh user
has no way to know:

- **`<scroll_view>` needs `flex_grow=1.0` on its parent.** This is the
  classic CLAUDE.md gotcha — if you wrap your content in
  `<scroll_view>` without a bounded parent, scroll bars never appear.
- **`<view>` not `<label>` for `Result<T, E>` children.** Because Label
  has the `IntoMaybeReactive<String>` child setter, you can't put a
  `Result` in it for ErrorBoundary to catch.
- **`<grid>` placement uses `grid_column=(1, -1)` on children.** The
  child needs to be a `WithLayout` builder (every builder is, but the
  user has to know they configure placement on the child, not the
  parent).
- **`Taffy's Grid type isn't Send.**" Documented inline in the spotify
  example (the comment about why two `<hstack>`s instead of a `<grid>`
  inside a `#[component]`). This is a real limitation that has no
  surfaced workaround.
- **Stack defaults to Column direction**, but the bare `stack()` /
  `view()` builders set it via `Render::build` rather than at
  constructor time, so passing through Stack to a different code path
  could see `direction=None`. Subtle.

---

## 3. Recommendations

Ordered from highest leverage / lowest cost to lowest. The library is
explicitly pre-1.0; breaking changes are on the table.

### Priority 1 — Cheap consolidations and fixes

These are the easy wins; small, mechanical changes that smooth real
sharp edges.

1. **Add a `WithDecoration` trait** with `background_color`,
   `corner_radius`, `border_width`, `border_color`, `clip` setters,
   and impl it on every builder. Today these methods exist on Stack,
   Grid (partial), and Button, and have to be reinvented or wrapped
   for everything else. After: every control can be styled identically.
2. **Unify Stack and Grid's flex-style methods.** Pick one name —
   `align` or `align_items` — and use it everywhere. Add `wrap`,
   `corner_radius`, `border_*`, `hidden` to `Grid`. Add `gap`-style
   per-axis methods to Stack if useful (today Stack has only `gap`).
3. **Pick a name for "vertical stack".** Keep `vstack()` / `hstack()`
   as the canonical entry points; deprecate `stack_view()`. Decide
   between `stack()` and `view()` and pick one (probably `view()` —
   it's the SwiftUI-flavored generic-container tag, matches iOS/GTK
   port vocab, and reads naturally in `view!{}`). Remove the other.
4. **Fix `hidden=`** to set Taffy `display: none` (collapsing the
   slot), or remove it entirely in favor of `<Show>`. Document the
   semantics either way. Current "looks right, behaves wrong" is the
   worst of both worlds.
5. **Drop `Label::bind:value`.** Two-way binding to a read-only sink
   is a category error. Users can already write
   `<label>{move || sig.get()}</label>` (one character longer).
6. **Stop leaking `cocoa_dom::` into example code.** Re-export
   `Color`, `Date`, `KeyEvent`, `TextAlignment`, `LineBreak`,
   `SegmentStyle`, `DatePickerStyle` from a single `leptos::prelude`
   path *only* (or `leptos::types::*`). Mark `cocoa_dom` as
   `#[doc(hidden)]` for downstream — users should never need it.
7. **Single import path for builders.** Pick `tachys::html::element::*`
   (matches macro) OR `tachys::cocoa::*` (matches port). Probably
   `tachys::html::element::*` since the macro emits those paths. Remove
   the other.
8. **Promote `bind:value=` to cover indices.** Drop `bind:selection=`,
   accept `Signal<usize>` via `bind:value=` on PopUpButton and
   SegmentedControl. One key, less to remember.

### Priority 2 — Architectural cleanups

These are bigger changes that meaningfully reshape the surface, but
the library is small enough that the cost is contained.

9. **Make `<split_view>` a regular element, not a special mount
   path.** *Deferred — see notes.* Move the NSSplitViewController
   setup into `Window`'s build when it detects a top-level
   `<split_view>` child. Delete `mount_to_split_window` and
   `IntoSplitView`. After: one mount function (`mount_to_window`)
   handles everything.

   *Why deferred:* the natural design — a `WindowChild` trait with
   blanket impl `for R: Render<Dom>` and specific impl `for
   SplitView<P>` — hits Rust coherence rules without specialization.
   The alternatives (a wrapper newtype, or having `SplitView::mount`
   walk up to the NSWindow and swap its `contentViewController`)
   are tractable but require more invasive refactoring than fit
   into the P1+P2 sweep. Keep `mount_to_split_window` for now.
10. **Make every static attribute accept reactive forms.** Specifically:
   `placeholder`, `Slider::min_value/max_value`, `Stepper::min_value/
   max_value/increment`, `Grid::columns/rows/auto_*`. Where reactive
   doesn't make sense (e.g., `size(n)` setting five things), document
   why inline. Today users can't tell which attrs are reactive
   without trying.
11. **Unify `child()` semantics.** *Deferred to P3 — see notes
    below.* Two open questions: (a) Label probably shouldn't accept
    Render children at all; it's a leaf view of a string. (b) Button
    *does* want generic children — image + text, SF Symbol + text —
    but the right AppKit path needs research (NSButton's
    `attributedTitle` for inline images vs an `image:` + `title:`
    pair vs a custom-view subview).
12. **A real menu/menubar API.** `app::menu_bar()` builder taking a
    list of `Menu` / `MenuItem` children with `key_equivalent`,
    `on:click`, separators, sub-menus. Essential for "production
    macOS app" use cases.
13. **Stub-then-grow a `Window` lifecycle:**
    - Reactive `title=`, `size=` that work after build.
    - `Window::position(x, y)` (initial position, reactive).
    - `Window::on_close(fn)`, `Window::close()` via NodeRef-like
      handle.
    - Application-level `App::quit()` callable from anywhere.
14. **Animation primitive.** *Deferred to P3.* A
    `.animation(duration, curve)` modifier (or `<Animated value=...>`)
    that interpolates `background_color`, `corner_radius`,
    `width/height` etc. on signal change. Map to NSAnimationContext
    on cocoa, UIView.animate on uikit. Even a simple "fade between
    values" version would lift the polish
    ceiling enormously.

### Priority 3 — New features that the library will need

15. **Image API:** `ImageView::data(&[u8])`, `ImageView::url(...)`
    (async fetch), `ImageView::sf_symbol("plus.circle")`. The
    file-path-only version is a 0.1-shaped API.
16. **System dialog/picker API:** open/save panel, alert/sheet,
    confirm. These exist as NSAlert/NSOpenPanel but are not surfaced.
17. **Clipboard and drag-and-drop primitives.** Basic
    `Clipboard::get_string()` / `set_string(s)` would cover 80% of
    needs.
18. **Toolbar / `<toolbar>`.** Macos apps live or die by their
    toolbar; the spotify and pages mockups had to fake it in hstack.
    Native NSToolbar integration would be a real differentiator.
19. **More builtin controls** (matching CLAUDE.md's "not implemented"
    list and visible gaps from examples):
    - Native `<table>` / `<list>` (NSTableView / NSOutlineView) —
      todomvc currently uses `<For>` over `<vstack>`, which doesn't
      get row selection, column resizing, or accessibility.
    - `<tabs>` / `<tabbed_view>` (NSTabView).
    - `<box_view>` (NSBox) with title, border options.
    - `<level_indicator>` (NSLevelIndicator).
20. **Resource / network primitives.** Examples that do real HTTP
    (`fetch`, `transition`) all reinvent the
    `tokio::spawn + oneshot::channel + LocalResource` bridge. A
    helper — `use_fetch(url)` or `Resource::http(...)` — could
    standardize the pattern.
21. **Window restoration** via NSWindowRestoration so size/position
    survive app restarts. This is what makes a macOS app feel native.

### Priority 4 — Polish before 1.0

22. **Doc strategy.** Today CLAUDE.md is the canonical documentation;
    the public API has rustdoc but the *patterns* (when to use
    `<view>` vs `<label>`, why scroll_view needs flex_grow, etc.)
    are only in implementation logs and CLAUDE.md.
    Move the gotchas into module-level rustdoc on the relevant
    builders. `Label::child` should warn against `Result<T, E>` in
    its doc and point to `<view>`. `ScrollView` should warn about
    the bounded-parent requirement in its top doc.
23. **Naming cleanup pass.**
    - `pop_up_button` → just `popup` or `dropdown`. The cocoa name
      is a leak.
    - `text_view` vs `text_field` for single-vs-multi-line is OK,
      but `text_area` is the universal web term.
    - `progress_indicator` → `progress` (with the indeterminate flag
      switching to spinner).
    - `image_view` → `image`.
    - `color_well` → `color_picker`.
    - `segmented_control` → `segments` or `toggle_group`.

    Goal: drop the AppKit class names and pick conventional names
    that match GTK / iOS / web user expectations. AppKit's vocabulary
    is fine for `cocoa_dom`; the framework-level surface should
    abstract it.
24. **Make `mount_to_window` return an `App` handle.** Instead of
    `mount_to_window(...)` being terminal, `App::new(...).run()`
    style. Lets people add menu configuration, app delegate hooks,
    etc. before running. Equivalent of SwiftUI's `@main App` protocol.
25. **A `<spacer>` element.** Today `<vstack flex_grow=1.0 />`
    works but reads oddly; `<spacer />` would be clearer. Also
    matches SwiftUI.
26. **Decide on the directive surface.** Either commit (export
    `IntoDirective` from prelude, document writing custom
    directives, add example) or simplify (replace with a single
    `on_build(fn)` per builder). Today it's neither — supported
    but undiscoverable.

### Priority 5 — Strategic / long-term

27. **API parity matrix across cocoa / uikit / gtk.** The three
    ports have drifted (no PopUpButton on iOS, no `text_view` styling
    parity, etc.). For "write once, run native everywhere" to be a
    real claim, there needs to be a documented core (subset that's
    on all three) vs platform extensions. Today downstream code that
    uses `<pop_up_button>` silently won't compile on iOS.
28. **Hot reload.** Mentioned in TODO.md. For a UI library this is
    the killer feature. Hard problem (Rust + native) but a
    crate-level differentiator.
29. **Rename the project before 1.0** if "leptos-mac" / "leptos
    native fork" is going to stick. TODO.md mentions "Pachys".

---

## Summary

This API is in noticeably better shape than its complexity would
suggest — the SwiftUI-flavored builder syntax + the upstream Leptos
reactive machinery is a strong combination, and `MaybeReactive` +
`WithLayout` traits keep most of the surface coherent. The pre-1.0
breaking-change opportunities cluster around four themes:

1. **Consistency** — `Stack` vs `Grid`, `child()` overloads, attribute
   reactivity, `bind:selection` vs `bind:value`, name aliases.
2. **Missing primitives** — menus, animations, images-from-data,
   dialogs, window lifecycle, toolbars, native tables.
3. **Implementation layering leaking through** — `cocoa_dom::`
   imports in user code, `mount_to_split_window` as a separate
   entry point, AppKit class names in builder names.
4. **Patterns that have to be learned the hard way** — `<view>` vs
   `<label>`, `scroll_view` parent requirements, `hidden` semantics.
   Most of these are real, documentable design decisions; they just
   need to be in the doc surface rather than implementation logs.

Priority 1 alone (rough ~2 weeks of work for one person) would close
the most-noticeable rough edges. Priority 2 reshapes the surface into
something publishable as a 0.5 / 0.6 milestone. Priority 3 fills in
the "production macOS app" feature gap. Priorities 4-5 are
post-stabilization polish.

---

## Appendix A — Deferred to P3: design notes

### child() semantics on string-bearing leaves (Button, Label, Checkbox)

After the P1+P2 sweep, Label/Button/Checkbox still take only
`IntoMaybeReactive<String>` via their `child()` overload. Whether
this should change is a P3 question. The two leaves disagree:

- **Label.** A label is a leaf view of a string. Letting it host
  arbitrary children is probably a category error — if you want
  rich content, `<view>` is the right container. Keep label
  string-only. Document the workaround clearly.
- **Button.** This one matters. SwiftUI buttons happily host icons,
  text, and arbitrary subviews. macOS buttons (and toolbar items
  especially) routinely combine an SF Symbol or NSImage with a
  caption. NSButton supports this through three different paths:
    1. `image:` + `title:` (separate properties; AppKit positions
       them via `imagePosition`). Lowest-friction for icon-plus-text.
    2. `attributedTitle:` with embedded `NSTextAttachment` for
       inline icons. Composable but more setup.
    3. Custom-view subview hosted inside the button frame. Most
       flexible but loses native click/hover affordances.

  The right approach probably depends on what's being composed.
  Needs prototyping before committing to an API shape. Some
  candidate shapes to evaluate:
    - `<button image="plus.circle" title="Add"/>` — typed image
      attribute alongside title, NSButton's native path.
    - `<button>"Add"</button>` keeps working for plain-text; rich
      content via `<button>{move || view! { <hstack>...</hstack> }}</button>`
      lifts the title slot to accept any Render.
    - Drop the string-specialized `child()` entirely and require
      `<button title="Add"/>` everywhere. Most consistent but
      breaks existing code.

### Animation primitive

The shape question is whether animation is a *modifier*
(`.animation(duration, curve)` per builder, applying to subsequent
reactive attribute changes) or a *primitive* (`<Animated value=...
duration=...>` wrapper that interpolates).

CoreAnimation's natural unit is the CALayer property — `setOpacity`,
`setBackgroundColor`, `setBounds`, `setCornerRadius`. AppKit's
`NSAnimationContext` wraps these in implicit animations. Mapping to
our `MaybeReactive` setters means the install pipeline needs an
optional "wrap this setter in an animation context" hook.

Open design questions:
- Per-attribute opt-in vs per-builder default?
- How to express interpolation for non-animatable types (enum
  values like `TextAlignment` — fade through? Snap at midpoint?
  Forbid?)
- Stagger / chain semantics for groups (`<For>` insertions)?
- Layout animation needs a Taffy diff step (animate width changes
  between layout passes). Significant work.

For a 0.x release, the minimal viable feature is probably
`alpha` + `background_color` + `corner_radius` interpolation only —
the CALayer "free" set. That gives "fade in/out" and "color
breathing" patterns; everything else is post-1.0.
