# Design discussion: `IntoAttributeValue` and renderer extensibility

For sharing with upstream Leptos maintainers. This document describes
a tension in how the `view!{}` macro forwards attribute values to
typed builder methods, why it blocks third-party renderer crates from
extending Leptos with new (non-DOM) attribute value types, and a
shortlist of possible resolutions with their tradeoffs.

## Background

Leptos 0.7+ ships a typed-builder API for HTML attributes. Each
attribute is both a free function (returning an `Attr<Key, V>`) and a
method on the `GlobalAttributes`/element-specific traits. The methods
are bounded `V: AttributeValue`:

```rust
// tachys/src/html/attribute/global.rs (excerpt)
pub trait GlobalAttributes<V>
where
    Self: Sized + AddAnyAttr,
    V: AttributeValue,
{
    fn id(self, value: V) -> <Self as AddAnyAttr>::Output<Attr<Id, V>> {
        self.add_any_attr(id(value))
    }
    // … 100+ similarly shaped methods
}
```

The `view!{}` macro emits chained method calls on the element
builder. For non-literal attribute values it wraps the user's
expression through `IntoAttributeValue::into_attribute_value(...)`:

```rust
// leptos_macro/src/view/mod.rs (today, simplified)
quote! {
    .#key(::leptos::prelude::IntoAttributeValue::into_attribute_value(#value))
}
```

`IntoAttributeValue` has a blanket impl that's identity for any
`T: AttributeValue`, plus a few "real conversion" impls (notably
`TextProp` → `Arc<dyn Fn() -> Oco<'static, str>>`). Net effect: most
of the time the wrap is a no-op the optimizer deletes; for the
handful of types with non-identity conversions, it's the seam that
makes them pass-through-able as attribute values.

## What this supports today

`TextProp` is the load-bearing case: a component-prop type that's
not itself an `AttributeValue`, but has an `IntoAttributeValue` impl
whose `Output` *is* an `AttributeValue`:

```rust
use leptos::prelude::*;

#[component]
fn HeaderLink(
    /// Reactive label coming in as a `TextProp` prop.
    #[prop(into)] label: TextProp,
    href: &'static str,
) -> impl IntoView {
    // The user passes `label` (a TextProp) directly to `id=` and
    // it Just Works — the `view!` macro's `IntoAttributeValue` wrap
    // converts the TextProp to its inner `Arc<dyn Fn() -> Oco>`,
    // which is an AttributeValue.
    view! {
        <a id=label.clone() href=href>
            {label}
        </a>
    }
}
```

Without the wrap, that `id=label` would fail to compile because
`TextProp: AttributeValue` is unsatisfied. Users would have to write
`id={label.into_attribute_value()}` manually, which leaks an
implementation detail.

## What we want it to support

Native (non-DOM) renderer crates ship typed builders whose
attributes accept renderer-specific value types:

```rust
// In a hypothetical leptos_cocoa crate:
impl Label {
    pub fn text_color(self, color: impl IntoMaybeReactive<Color>) -> Self { … }
}
```

Where `Color` is `cocoa_dom::Color` (an enum: `SYSTEM_BLUE`, `LABEL`,
`Hex(u32)`, …). A user writing:

```rust
use leptos::prelude::*;
use leptos_cocoa::prelude::*;

#[component]
fn Heading(text: String) -> impl IntoView {
    view! {
        <label
            text_color=Color::SYSTEM_BLUE
            font_size=22.0
        >
            {text}
        </label>
    }
}
```

…wants the macro to forward `Color::SYSTEM_BLUE` to
`Label::text_color` unchanged. But the current macro emits

```rust
.text_color(IntoAttributeValue::into_attribute_value(Color::SYSTEM_BLUE))
```

…which requires `Color: IntoAttributeValue`. The blanket impl
`impl<T: AttributeValue>` doesn't apply (Color isn't an
`AttributeValue` — it's a renderer-specific enum). So someone has to
write a manual `impl IntoAttributeValue for Color { type Output =
Self; … }`.

## The constraint that makes this hard

`IntoAttributeValue` lives in tachys; `Color` lives in `cocoa_dom`.
Per Rust's orphan rule, neither `leptos_cocoa` (the renderer glue
crate) nor any third-party renderer extension can write that impl —
both the trait and the type are foreign to it.

The only crates that *can* write it are tachys and cocoa_dom. We've
been doing it in tachys, gated on `cfg(feature = "native-ui")`:

```rust
// tachys/src/html/attribute/value.rs (the status quo we want to leave behind)
#[cfg(all(target_os = "macos", feature = "native-ui"))]
impl IntoAttributeValue for cocoa_dom::Color {
    type Output = Self;
    fn into_attribute_value(self) -> Self::Output { self }
}
// … plus identity impls for FlexDirection, JustifyContent, AlignItems,
// FlexWrap, NSTextAlignment, ios_dom::Color, Vec<&'static str> for
// pop-up items, etc.
```

This is what we'd like to delete. The block makes tachys aware of
specific renderer crates (cocoa_dom, ios_dom, gtk_dom) — exactly the
coupling the `__leptos_view`-pluggable-renderer architecture is
trying to remove. As long as the macro's `into_attribute_value` wrap
is mandatory, tachys remains the only place those impls can live,
and we can't extract the per-OS code into independent renderer
crates.

## Solutions

### Solution 1 — drop the wrap unconditionally

Change the macro emission for typed-attribute calls from

```rust
.#key(IntoAttributeValue::into_attribute_value(#value))
```

to

```rust
.#key(#value)
```

The custom-attr `.attr(name, value)` and `attr:` spread emissions
keep the wrap (where the conversion is genuinely needed because the
target type is erased).

**Pros:** trivial macro change, smallest surface area; native
renderer crates work out of the box (their setters take whatever
type they want, the user's value flows through unchanged).

**Cons:** breaks `TextProp`-style "real conversion" cases. Users who
pass a `TextProp` (or any other type whose `IntoAttributeValue::Output
!= Self`) directly to a typed attribute now see a compile error
(`TextProp: AttributeValue` unsatisfied). They'd have to write
`id={label.into_attribute_value()}` manually, which leaks the
abstraction.

The leptos-mac fork is currently using this approach as a stopgap
(it's the smallest change that unblocks the tachys extraction). We
agree it's not acceptable upstream as-is.

### Solution 2 — push the wrap into the setter signature

Change every web typed setter from

```rust
fn id(self, value: V) -> _ where V: AttributeValue { … }
```

to

```rust
fn id(self, value: impl IntoAttributeValue<Output: AttributeValue>) -> _ {
    self.add_any_attr(id(value.into_attribute_value()))
}
```

The macro stops wrapping; the setter does the conversion in its body.
For native renderers the typed setters retain their own bounds
(`impl IntoMaybeReactive<Color>`, etc.) — they don't take
`IntoAttributeValue`, so renderer-specific values flow through
unchanged.

**Pros:** preserves `TextProp` ergonomics on web; no macro change;
clean per-renderer story (each renderer's typed setters declare their
own conversion bound).

**Cons:** touches every typed attribute setter on web — `id`,
`title`, `aria_*`, `data_*`, all of `GlobalAttributes`, all of the
element-specific trait methods. That's hundreds of method signatures
across `tachys/src/html/attribute/{global,aria,custom,…}.rs`.
Mechanical but voluminous. Requires careful audit because
`AddAnyAttr::Output` parametrization currently uses `V` directly (`<Self
as AddAnyAttr>::Output<Attr<Id, V>>`); turning `V` into `impl
IntoAttributeValue<Output: AttributeValue>` requires desugaring or
restructuring that signature.

### Solution 3 — autoref-specialization trick (a.k.a. the Yandros / `spez` pattern)

Use Rust's autoref behavior to make `into_attribute_value()` resolve
to one of two impls — a "real" one for types that impl
`IntoAttributeValue`, an identity one for any other type — without
needing real specialization.

**Pros:** truly transparent — the macro emission stays the same, web
keeps working as today, native types Just Work.

**Cons:** the trick is famously fragile and subtle. The error
messages it produces when something goes wrong are awful. It's
sensitive to small Rust-language changes (autoref behavior has been
tweaked a couple of times). Likely a hard sell upstream for the
attribute-value system that touches every user view tree.

### Solution 4 — renderer-pluggable wrap function in the macro extension namespace

Build on the recently-added `__view_namespace` extension point
(where the `view!{}` macro routes elements/events/attrs through a
per-renderer module).

Change the macro emission from

```rust
.#key(::leptos::prelude::IntoAttributeValue::into_attribute_value(#value))
```

to

```rust
.#key(__leptos_view::attrs::convert(#value))
```

Each renderer's `__view_namespace::attrs` module exports its own
`convert` function:

```rust
// In leptos's web __view_namespace::attrs:
pub fn convert<T: IntoAttributeValue>(value: T) -> T::Output {
    value.into_attribute_value()
}

// In leptos_cocoa's __view_namespace::attrs:
pub fn convert<T>(value: T) -> T { value }

// In leptos_ios's, leptos_gtk's: identity, like cocoa.
```

**Pros:** preserves `TextProp` ergonomics on web; trivial macro
change (one function path swap); no per-setter signature changes;
uses the renderer-pluggable extension point we already have.

**Cons:** the `convert` function is renderer-policy, defined in
each glue crate. Some bookkeeping: if a third-party renderer wants
its own conversion behavior (e.g. a renderer that *does* want some
form of normalization), it writes its own `convert`. But the
contract is "must accept any T and return something the typed setter
will accept" — looser than `IntoAttributeValue`'s well-typed
contract.

In practice, all renderers other than web would use the identity
form. There's no obvious reason a non-web renderer would want to do
anything else, since native typed setters declare their own
conversion bounds (`IntoMaybeReactive`, etc.).

### Solution 5 — leave the wrap, add a permissive blanket via specialization (hypothetical)

If/when min-spec stabilizes, add

```rust
default impl<T> IntoAttributeValue for T {
    type Output = T;
    fn into_attribute_value(self) -> Self::Output { self }
}
```

…with the existing `impl<T: AttributeValue>` overriding for
AttributeValue types. Native renderer types fall through to the
default identity impl.

**Pros:** zero macro changes; zero setter changes; transparent to
users.

**Cons:** depends on a Rust feature (specialization) that's been
"coming soon" for years. Not viable today.

## Tradeoff matrix

| | Macro changes | Per-setter changes | Web `TextProp` ergonomics | Native zero-config? | Upstream-shippable? |
|---|---|---|---|---|---|
| 1 — drop wrap | one line | none | **broken** (manual `.into_attribute_value()`) | yes | maybe (with deprecation/migration) |
| 2 — push to setter | none | **all setters** | preserved | yes | yes (mechanical but big) |
| 3 — autoref trick | one line | none | preserved | yes | risky (fragile) |
| 4 — per-renderer `convert` | one line | none | preserved | yes (1 fn per glue crate) | yes |
| 5 — specialization | none | none | preserved | yes | no (no min-spec) |

## Recommendation

**Solution 4** seems the best balance for upstream:

- Smallest macro change (swap one path).
- Zero web behavior change — `TextProp` users keep their seamless
  syntax.
- Native renderer crates get a single 1-line `pub fn convert<T>(v: T)
  -> T { v }` to ship in their `__view_namespace::attrs` and the
  orphan-rule pin in tachys disappears.
- The `__view_namespace` extension point already exists for elements,
  events, attrs, and bind keys. Adding `convert` is incremental.

If the per-renderer `convert` feels like architectural overreach,
**Solution 2** is the runner-up — invasive but locally
understandable, and the change is mechanical.

**Solution 1** (what leptos-mac currently does) we'd avoid as the
final upstream answer. It's a useful intermediate state for our fork
because it unblocks the rest of the tachys extraction work, but
shipping a `TextProp` regression upstream isn't on.

## Footnote: what leptos-mac is doing now

This fork is currently using Solution 1 for two reasons:

1. It's a one-line macro change, so it's easy to revert / replace
   later.
2. None of the leptos-mac examples (web or native) use `TextProp`
   directly inside a `view!{}` attribute. So the regression is
   invisible to our test surface.

When we land an upstream-acceptable answer (probably Solution 4),
the change set in leptos-mac is two edits: revert the
`attribute_value(node, false)` → `attribute_value(node, true)` line in
`leptos_macro/src/view/mod.rs`, and switch the macro emission from
`IntoAttributeValue::into_attribute_value(value)` to
`__leptos_view::attrs::convert(value)` (then add a `convert` function
to each `__view_namespace::attrs` module).
