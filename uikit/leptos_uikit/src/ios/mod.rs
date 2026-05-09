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
pub mod node_ref;

pub use attr::{IntoMaybeReactive, MaybeReactive};
pub use bind::{BindAttribute, IntoSignal, Selection};
pub use element::{
    button, date_picker, hstack, image_view, label, progress_indicator,
    scroll_view, secure_text_field, segmented_control, slider, stepper,
    switch_, text_field, text_view, view, vstack,
};
pub use node_ref::NodeRef;

// Convenient passthrough of the most common style enums.
pub use ios_dom::layout::{FlexDirection, JustifyContent};
