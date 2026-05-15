# Element Reference

This section documents every native element this fork exposes —
its tag name, which platform widget it maps to, the
builder-method attributes it accepts, supported `on:` events, and
supported `bind:` keys.

| Element                                                         | Cocoa            | GTK4         | iOS              |
|-----------------------------------------------------------------|------------------|--------------|------------------|
| [`button`](./button.md)                                         | NSButton         | gtk::Button  | UIButton         |
| [`label`](./label.md)                                           | NSTextField      | gtk::Label   | UILabel          |
| [`text_field`](./text_field.md)                                 | NSTextField      | gtk::Entry   | UITextField      |
| [`secure_text_field`](./text_field.md)                          | NSSecureTextField| gtk::PasswordEntry | UITextField (masked) |
| [`text_view`](./text_view.md)                                   | NSTextView       | —            | UITextView       |
| [`checkbox`](./checkbox.md)                                     | NSButton (Switch style) | gtk::CheckButton | —          |
| [`switch`](./checkbox.md)                                       | —                | —            | UISwitch         |
| [`slider`](./slider.md)                                         | NSSlider         | gtk::Scale   | UISlider         |
| [`stepper`](./stepper.md)                                       | NSStepper        | —            | UIStepper        |
| [`segmented_control`](./segmented_control.md)                   | NSSegmentedControl | —          | UISegmentedControl |
| [`pop_up_button`](./pop_up_button.md)                           | NSPopUpButton    | gtk::DropDown | —               |
| [`date_picker`](./date_picker.md)                               | NSDatePicker     | —            | UIDatePicker     |
| [`color_well`](./color_well.md)                                 | NSColorWell      | —            | —                |
| [`progress_indicator`](./progress_indicator.md)                 | NSProgressIndicator | —         | UIProgressView   |
| [`image_view`](./image_view.md)                                 | NSImageView      | —            | UIImageView      |
| [`scroll_view`](./scroll_view.md)                               | NSScrollView     | —            | UIScrollView     |
| [`stack` / `vstack` / `hstack` / `view`](./stack.md)            | NSView (Taffy)   | gtk::Box     | UIView           |
| [`grid`](./grid.md)                                             | NSView (Taffy grid)| gtk::Box (Taffy) | UIView (Taffy) |

## Common conventions

- **Attributes are reactive.** Every attribute documented as
  `f32`, `bool`, `String`, etc. is also accepted as a closure
  returning that type. See [Reactivity and
  Functions](../reactivity/functions.md).
- **Events use `on:event_name=`**. Handlers are `FnMut`
  closures.
- **Two-way bindings use `bind:key=`** with an `RwSignal<T>` or
  `(Fn() -> T, FnMut(T))` tuple.
- **Shared layout attributes (`padding`, `width`, `flex_grow`,
  …) are accepted by every element.** They're documented once
  in [Shared Layout Attributes](../layout/attributes.md) and not
  repeated on individual element pages.
- **Children**: containers accept `<child>` syntax; leaf
  elements accept either a string literal (`<button>"OK"</button>`)
  or a `child=...` attribute.

## Tag naming

Element tag names are `snake_case`, including names that look
like SVG element names on the web (`<switch>` on iOS). Component
invocations are `PascalCase`. The `view!{}` macro uses the case
to decide whether a name is an element or a component.

`<switch>` is a Rust keyword, so the iOS port's macro emits
`r#switch`; you don't need to do anything special at the source
level.
