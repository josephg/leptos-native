//! CoreAnimation Phase 1 — explicit property animations.
//!
//! Wrap a closure that mutates signals with [`with_animation`];
//! every layer-backed setter that runs as a consequence
//! (`set_background_color`, `set_corner_radius`, `set_border_*`,
//! `set_alpha`) will interpolate from its previous value to the
//! new value over the supplied duration / curve instead of
//! snapping.
//!
//! ```ignore
//! let expanded = RwSignal::new(false);
//! // ...inside an event handler:
//! with_animation(spring(), move || expanded.update(|e| *e = !*e));
//! ```
//!
//! Phase 1 scope: only the explicit layer-property setters listed
//! above. Frame/layout changes do **not** animate — they snap on
//! the next relayout tick as before. Layout animation is Phase 2.
//!
//! Available only when the `animation` Cargo feature is enabled.
//!
//! # How it works
//!
//! `with_animation` pushes an [`Animation`] onto a thread-local
//! "current animation" slot, opens a [`CATransaction`] so any
//! implicit-action animations inherit the supplied duration /
//! timing, runs the body synchronously, commits the transaction,
//! then schedules an async cleanup that resets the slot. The
//! cleanup is FIFO-ordered behind any [`RenderEffect`] re-runs
//! that the body queued, so effect-driven setter writes still
//! see the animation context when they fire.
//!
//! Each setter, when [`current_animation`] is [`Some`], captures
//! the layer property's current value, writes the new model
//! value, then adds a [`CABasicAnimation`] (or
//! [`CASpringAnimation`] for spring curves) keyed by the
//! property keypath. Same-key `addAnimation` calls replace any
//! prior animation, so our explicit animation cleanly overrides
//! AppKit's implicit action when both fire.
//!
//! # Caveats
//!
//! - **Effect-driven writes that `.await` past one runloop tick
//!   miss the animation.** The cleanup runs after the next
//!   `spawn_local` drain. If a RenderEffect awaits before
//!   writing, the cleanup may run first.
//! - **A panic inside `body` still triggers the cleanup** (via a
//!   Drop guard), so a panicking handler doesn't permanently
//!   "stick" the animation slot.
//! - **Must be called on the main thread.** Asserts via
//!   `MainThreadMarker::new()`.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2_foundation::{NSNumber, NSString};
use objc2_quartz_core::{
    kCAMediaTimingFunctionEaseIn, kCAMediaTimingFunctionEaseInEaseOut,
    kCAMediaTimingFunctionEaseOut, kCAMediaTimingFunctionLinear,
    CABasicAnimation, CALayer, CAMediaTiming, CAMediaTimingFunction,
    CASpringAnimation, CATransaction,
};
use std::cell::Cell;

/// Timing curve for an [`Animation`].
#[derive(Clone, Copy, Debug)]
pub enum Curve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Cubic bézier with endpoints implicitly at (0,0) and (1,1).
    CubicBezier(f32, f32, f32, f32),
    /// Physical spring. `response` ≈ perceived duration in seconds
    /// (smaller = snappier); `damping_ratio` ≈ how much it bounces
    /// (1.0 = critically damped, no overshoot; ~0.7 = a small
    /// overshoot; 0.5 = lively bounce). Mass is fixed at 1.0 in
    /// Phase 1.
    Spring {
        response: f64,
        damping_ratio: f64,
    },
}

/// A description of how a property change should animate.
#[derive(Clone, Copy, Debug)]
pub struct Animation {
    /// Duration in seconds. For spring curves this is ignored;
    /// the spring uses its own `settlingDuration`.
    pub duration: f64,
    pub curve: Curve,
}

impl Animation {
    pub fn linear(duration: f64) -> Self {
        Self { duration, curve: Curve::Linear }
    }
    pub fn ease_in_out(duration: f64) -> Self {
        Self { duration, curve: Curve::EaseInOut }
    }
    /// Reasonable default spring (response 0.5s, damping ratio
    /// 0.825 — barely overshoots).
    pub fn spring() -> Self {
        Self {
            duration: 0.5,
            curve: Curve::Spring { response: 0.5, damping_ratio: 0.825 },
        }
    }
}

thread_local! {
    // Single-slot "current animation" for the main thread. Falls
    // squarely under the MEMORY_POLICY app-scope carve-out: it's
    // a fixed-size value, only ever Some during a `with_animation`
    // burst, and unconditionally cleared by the spawn_local
    // cleanup queued by [`CleanupGuard::drop`].
    static CURRENT: Cell<Option<Animation>> = const { Cell::new(None) };

    // Snapshot of the animation active when a relayout was
    // scheduled, retrieved by the deferred `compute_layout` pass
    // (Phase 2 — layout animation). The dispatch-queue ordering
    // puts the relayout *after* the cleanup that clears
    // `CURRENT`, so we capture at schedule time and read at
    // compute time. Cleared by [`take_pending_layout_animation`]
    // at the start of each compute_layout pass.
    static PENDING_LAYOUT_ANIM: Cell<Option<Animation>> = const { Cell::new(None) };
}

/// Returns the in-flight animation context, if any. Setters use
/// this to decide whether to interpolate.
pub fn current_animation() -> Option<Animation> {
    CURRENT.with(|c| c.get())
}

/// Called from `schedule_relayout_for_tree` to stash the current
/// animation for the deferred relayout pass to find. No-op if
/// no animation is active. Last-write-wins within a burst, which
/// is correct since every write happens under the same
/// `with_animation`.
pub(crate) fn capture_for_layout() {
    if let Some(anim) = current_animation() {
        PENDING_LAYOUT_ANIM.with(|c| c.set(Some(anim)));
    }
}

/// Called from `apply_frames` at the start of a relayout pass.
/// Returns + clears the pending animation; subsequent frame
/// writes within the pass animate over the returned curve.
pub(crate) fn take_pending_layout_animation() -> Option<Animation> {
    PENDING_LAYOUT_ANIM.with(|c| c.take())
}

/// Read a property from the **presentation layer** (what the
/// user currently sees) if the layer is in flight, falling back
/// to the **model layer** otherwise. Use this for the `fromValue`
/// of a new explicit animation so that interrupting a running
/// animation continues smoothly from the current visual state
/// rather than snapping to the model (which is already at the
/// previous animation's *target* value).
pub(crate) fn presentation_or_model<T>(
    layer: &CALayer,
    read: impl Fn(&CALayer) -> T,
) -> T {
    // SAFETY: -presentationLayer is documented as safe to call
    // at any time; it returns nil when no animation is in
    // flight, in which case we fall back to the model layer.
    if let Some(p) = unsafe { layer.presentationLayer() } {
        read(&p)
    } else {
        read(layer)
    }
}

/// Build NSValue wrappers for CGPoint / CGSize / CGRect — the
/// types `position` and `bounds` need for KVC on
/// CABasicAnimation. CGPoint and friends are not NSObjects
/// themselves, so they have to be boxed.
pub(crate) fn animate_frame(
    layer: &CALayer,
    old_position: objc2_foundation::NSPoint,
    old_bounds: objc2_foundation::NSRect,
    new_position: objc2_foundation::NSPoint,
    new_bounds: objc2_foundation::NSRect,
    anim: Animation,
) {
    use objc2_foundation::NSValue;
    let from_pos =
        unsafe { NSValue::valueWithPoint(old_position) };
    let to_pos = unsafe { NSValue::valueWithPoint(new_position) };
    apply_property_animation(
        layer,
        "position",
        Some(from_pos.as_ref()),
        Some(to_pos.as_ref()),
        anim,
    );
    let from_bounds = unsafe { NSValue::valueWithRect(old_bounds) };
    let to_bounds = unsafe { NSValue::valueWithRect(new_bounds) };
    apply_property_animation(
        layer,
        "bounds",
        Some(from_bounds.as_ref()),
        Some(to_bounds.as_ref()),
        anim,
    );
}

/// RAII guard: on Drop, schedules an async restore of the
/// previous `CURRENT` value. Using Drop (not an explicit cleanup
/// at end of function) is what makes `with_animation`
/// panic-safe — if `body()` unwinds we still queue the restore.
struct CleanupGuard {
    previous: Option<Animation>,
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let previous = self.previous;
        // FIFO ordering on the main dispatch queue ensures this
        // runs after any RenderEffect re-runs that body() queued.
        any_spawner::Executor::spawn_local(async move {
            CURRENT.with(|c| c.set(previous));
        });
    }
}

/// Run `body` with `anim` installed as the current animation
/// context. Synchronous mutations inside `body` see the context;
/// effect re-runs queued by signal writes inside `body` also see
/// it via the FIFO cleanup ordering.
///
/// Nested calls stack; the outer animation is restored after the
/// inner cleanup runs.
///
/// # Panics
///
/// - If called off the main thread.
/// - If [`any_spawner::Executor`] is not initialised (i.e.
///   `with_animation` is invoked before `mount_to_window` /
///   `run()` has set up the spawner).
pub fn with_animation<R>(anim: Animation, body: impl FnOnce() -> R) -> R {
    let _mtm = MainThreadMarker::new()
        .expect("with_animation must be called on the main thread");

    // Stash the previous value via RAII guard so the cleanup
    // runs even if body() panics.
    let previous = CURRENT.with(|c| c.replace(Some(anim)));
    let _guard = CleanupGuard { previous };

    // Open a CATransaction so AppKit's implicit-action animations
    // (e.g. setAlphaValue → layer.opacity) inherit our duration
    // and timing, and so a future completion-block API can hang
    // off the burst. Only encompasses the *synchronous* body —
    // async effect-driven writes run in separate implicit
    // transactions but still see CURRENT via the FIFO cleanup.
    CATransaction::begin();
    CATransaction::setAnimationDuration(anim.duration);
    if let Some(tf) = timing_function_for(anim.curve) {
        CATransaction::setAnimationTimingFunction(Some(&tf));
    }
    let out = body();
    CATransaction::commit();
    out
}

// ---- Curve → CoreAnimation translation -------------------------

fn timing_function_for(curve: Curve) -> Option<Retained<CAMediaTimingFunction>> {
    unsafe {
        match curve {
            Curve::Linear => Some(CAMediaTimingFunction::functionWithName(
                kCAMediaTimingFunctionLinear,
            )),
            Curve::EaseIn => Some(CAMediaTimingFunction::functionWithName(
                kCAMediaTimingFunctionEaseIn,
            )),
            Curve::EaseOut => Some(CAMediaTimingFunction::functionWithName(
                kCAMediaTimingFunctionEaseOut,
            )),
            Curve::EaseInOut => Some(CAMediaTimingFunction::functionWithName(
                kCAMediaTimingFunctionEaseInEaseOut,
            )),
            Curve::CubicBezier(c1x, c1y, c2x, c2y) => {
                Some(CAMediaTimingFunction::functionWithControlPoints(
                    c1x, c1y, c2x, c2y,
                ))
            }
            Curve::Spring { .. } => None, // handled by CASpringAnimation
        }
    }
}

/// Build the animation, set explicit `from`/`to`, and attach it
/// to `layer` keyed by `key_path`. Same-key `addAnimation` calls
/// replace any prior animation (including AppKit's implicit
/// action), so this cleanly overrides default behaviour.
pub(crate) fn apply_property_animation(
    layer: &CALayer,
    key_path: &str,
    from_value: Option<&AnyObject>,
    to_value: Option<&AnyObject>,
    anim: Animation,
) {
    let key = NSString::from_str(key_path);
    match anim.curve {
        Curve::Spring { response, damping_ratio } => {
            // SwiftUI's spring math (with mass = 1):
            //   stiffness = (2π / response)²
            //   damping   = 4π · ζ / response
            // (critical damping = 4π / response; damping = ζ · critical).
            let response = response.max(1e-3);
            let omega = std::f64::consts::TAU / response;
            let stiffness = omega * omega;
            let damping = 4.0 * std::f64::consts::PI * damping_ratio / response;

            let spring =
                CASpringAnimation::animationWithKeyPath(Some(&key));
            spring.setMass(1.0);
            spring.setStiffness(stiffness);
            spring.setDamping(damping);
            spring.setInitialVelocity(0.0);
            if let Some(from) = from_value {
                unsafe { spring.setFromValue(Some(from)) };
            }
            if let Some(to) = to_value {
                unsafe { spring.setToValue(Some(to)) };
            }
            // Springs ignore Animation::duration — use the
            // physically-correct settling duration so the tail of
            // the bounce isn't clipped.
            spring.setDuration(spring.settlingDuration());
            layer.addAnimation_forKey(&spring, Some(&key));
        }
        _ => {
            let basic = CABasicAnimation::animationWithKeyPath(Some(&key));
            basic.setDuration(anim.duration);
            if let Some(tf) = timing_function_for(anim.curve) {
                basic.setTimingFunction(Some(&tf));
            }
            if let Some(from) = from_value {
                unsafe { basic.setFromValue(Some(from)) };
            }
            if let Some(to) = to_value {
                unsafe { basic.setToValue(Some(to)) };
            }
            layer.addAnimation_forKey(&basic, Some(&key));
        }
    }
}

// ---- Setter helpers -------------------------------------------

/// Animate a CGFloat-valued layer property (opacity, cornerRadius,
/// borderWidth). Caller passes the old value; this helper boxes
/// it as an NSNumber for KVC.
pub(crate) fn animate_float(
    layer: &CALayer,
    key_path: &str,
    from: f64,
    to: f64,
) {
    let Some(anim) = current_animation() else { return };
    let from_obj = NSNumber::new_f64(from);
    let to_obj = NSNumber::new_f64(to);
    apply_property_animation(
        layer,
        key_path,
        Some(from_obj.as_ref()),
        Some(to_obj.as_ref()),
        anim,
    );
}

/// CGColor is toll-free bridged with NSObject — its retain/release
/// pair is the same as `-retain`/`-release` at the runtime level,
/// so its pointer is a valid `AnyObject` pointer for KVC-style
/// `setValue:forKey:` (which is what `CABasicAnimation.fromValue`
/// uses under the hood).
fn cg_color_as_any(c: &objc2_core_graphics::CGColor) -> &AnyObject {
    // SAFETY: toll-free bridge between CGColorRef and NSObject *
    // (Apple guarantees CFRetain/CFRelease == -retain/-release).
    unsafe { &*(c as *const _ as *const AnyObject) }
}

/// Animate a CGColorRef-valued layer property (backgroundColor,
/// borderColor). `from` / `to` are CGColors; the toll-free
/// bridge lets us pass them as AnyObject to KVC.
pub(crate) fn animate_color(
    layer: &CALayer,
    key_path: &str,
    from: Option<&objc2_core_graphics::CGColor>,
    to: Option<&objc2_core_graphics::CGColor>,
) {
    let Some(anim) = current_animation() else { return };
    apply_property_animation(
        layer,
        key_path,
        from.map(cg_color_as_any),
        to.map(cg_color_as_any),
        anim,
    );
}
