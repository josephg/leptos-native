# Safe Area and Keyboard

UIKit hands the framework two important inset sources:

- **Safe-area insets** — the rectangle of the screen that
  *isn't* obscured by the status bar, notch, dynamic island, or
  home indicator.
- **Keyboard layout guide** — the rectangle of the screen
  *covered* by the on-screen keyboard.

The `RootViewController` automatically combines these into
padding on the content root, so your layout stays clear of
device chrome without you doing anything.

## How it works

On every `viewDidLayoutSubviews`:

1. Read `view.safeAreaInsets` — top inset (notch / status bar),
   bottom (home indicator), and left/right (rotation on iPhones
   with notches).
2. Read `view.keyboardLayoutGuide.layoutFrame` — when the
   keyboard is up, derive a bottom inset.
3. Take the **max** of safe-area-bottom and keyboard-bottom — so
   when the keyboard rises past the home indicator, the keyboard
   wins.
4. Set the content root's `padding=` accordingly.

The insets are diffed before being pushed — if the values haven't
changed, no relayout is triggered.

## Effect on your layout

Your top-level `<vstack>` sits inside the content root, so:

- Content starts below the notch / status bar.
- Content ends above the home indicator.
- When the keyboard opens (because the user tapped into a
  `<text_field>`), content shifts upward to stay visible.

You don't need to write extra padding. Your `padding=16.0` on a
`<vstack>` is additive — it adds 16pt to whatever the system
insets already are.

## Edge-to-edge content

If you want a view that extends behind the notch — e.g. a
full-bleed image — you can put it outside the safe-area padded
content root by routing through a NodeRef and adding it directly
to the view controller's `view`. There's no high-level builder
for this yet.

A more common pattern is to use the safe-area-padded layout for
most of the screen and use `<image_view>` with negative margins
(or absolute positioning via Taffy's `position: Absolute`) for
the hero element.

## Keyboard avoidance in practice

```rust
#[component]
fn LoginForm() -> impl IntoView {
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());

    view! {
        <vstack padding=24.0 gap=16.0>
            <label font_size=24.0>"Sign in"</label>
            <text_field bind:value=email placeholder="Email" />
            <secure_text_field bind:value=password placeholder="Password" />
            <button>"Sign in"</button>
        </vstack>
    }
}
```

Tap the email field on an iPhone and the keyboard rises;
everything in the stack scrolls up to stay above the keyboard
without any extra code. The bottom button stays accessible.

## A note on long forms

The keyboard inset is applied to the **content root** — the
whole view tree shifts up. For very short forms (login,
two-field dialog) that's enough; the form is short enough that
the entire vstack fits in the visible area above the keyboard.

For longer forms, the shift can push the top of the form off
the screen. Wrap such forms in a `<scroll_view>`:

```rust
<scroll_view>
    <vstack padding=24.0 gap=16.0>
        // many fields ...
    </vstack>
</scroll_view>
```

The scroll view's bounds are recomputed when the keyboard
appears, so the user can scroll the form into view. Note: this
fork does not yet auto-scroll to the focused field — that's an
open enhancement. Until it lands, users manually scroll to the
field they're editing.
