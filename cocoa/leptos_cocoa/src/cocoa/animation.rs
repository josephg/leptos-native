//! Re-export of `cocoa_dom::animation` plus tiny convenience
//! constructors.
//!
//! Gated on the `animation` Cargo feature. See
//! `cocoa_dom::animation` for the full doc and the list of which
//! setters participate.

pub use cocoa_dom::animation::{
    current_animation, with_animation, Animation, Curve,
};

/// Linear timing curve, given duration in seconds.
pub fn linear(duration: f64) -> Animation {
    Animation::linear(duration)
}

/// Ease-in-out, given duration. Reasonable default for opacity
/// and color transitions.
pub fn ease_in_out(duration: f64) -> Animation {
    Animation::ease_in_out(duration)
}

/// Default spring (response 0.5s, damping ratio 0.825).
pub fn spring() -> Animation {
    Animation::spring()
}
