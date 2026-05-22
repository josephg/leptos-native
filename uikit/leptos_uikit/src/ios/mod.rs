//! UIKit-flavoured element builders, the iOS analogue of
//! [`crate::html::element`]. Each tag here corresponds 1:1 to a
//! UIKit class; tag names drop the `UI` prefix and are snake_case
//! (`view` → UIView, `button` → UIButton, `switch` → UISwitch, etc.).
//!
//! The element types implement tachys' [`Render`](crate::view::Render)
//! trait, so they compose with the rest of the view-tree machinery.

#![cfg(target_os = "ios")]
#![allow(missing_docs)]

pub mod attr;
pub mod bind;
pub(crate) mod directives;
pub mod element;

pub use attr::{IntoMaybeReactive, MaybeReactive};
pub use bind::{BindAttribute, IntoSignal, Selection};
pub use element::{
    button, color_well, date_picker, grid, hstack, image_view, label,
    pop_up_button, progress_indicator, scroll_view, secure_text_field,
    segmented_control, slider, stack, stepper, switch_, text_field, text_view,
    toggle, vstack, IosText, WithText,
};

// Convenient passthrough of the most common style enums.
pub use crate::dom::layout::{
    AlignContent, AlignItems, FlexDirection, GridAutoFlow,
    GridTemplateComponent, JustifyContent, JustifyItems, TrackSizingFunction,
};
