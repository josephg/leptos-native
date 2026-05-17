//! Walk two NSView trees rooted at separate `OpenedWindow`s and
//! diff their structure + key AppKit-readable attributes + computed
//! frames. Surfaces the first mismatch via `Err(String)`.
//!
//! "Same" is defined as:
//!   * Same ObjC class name at every node.
//!   * Same number of subviews at every level (recurse left-to-right).
//!   * Same value-bearing attributes: NSControl `stringValue`,
//!     NSButton/NSCheckbox `title` + `state`, NSTextField `placeholder`,
//!     `isHidden`, `isEnabled` (for NSControls).
//!   * Same `frame` (origin + size) within an integer-pixel tolerance.
//!
//! Anything beyond that (font metrics, layer-backed flags,
//! subview-internal cells) is intentionally not compared — adds
//! flakiness without catching regressions our spec actually drives.

use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSButton, NSControl, NSTextField, NSView};

pub fn compare_trees(
    a_root: &NSView,
    b_root: &NSView,
) -> Result<(), String> {
    compare_node(a_root, b_root, "root")
}

fn compare_node(a: &NSView, b: &NSView, path: &str) -> Result<(), String> {
    // Class name.
    let ac = class_name(a);
    let bc = class_name(b);
    if ac != bc {
        return Err(format!(
            "{path}: class mismatch ({ac} vs {bc})"
        ));
    }

    // Hidden + frame.
    let a_hidden = a.isHidden();
    let b_hidden = b.isHidden();
    if a_hidden != b_hidden {
        return Err(format!(
            "{path}: isHidden mismatch ({} vs {})",
            a_hidden, b_hidden
        ));
    }

    let af = a.frame();
    let bf = b.frame();
    if !approx_rect(af.origin.x, bf.origin.x)
        || !approx_rect(af.origin.y, bf.origin.y)
        || !approx_rect(af.size.width, bf.size.width)
        || !approx_rect(af.size.height, bf.size.height)
    {
        return Err(format!(
            "{path}: frame mismatch ({:?} vs {:?})",
            af, bf
        ));
    }

    // Class-specific attrs.
    let any_a: &AnyObject = a.as_ref();
    let any_b: &AnyObject = b.as_ref();

    if let (Some(ca), Some(cb)) = (
        any_a.downcast_ref::<NSControl>(),
        any_b.downcast_ref::<NSControl>(),
    ) {
        // `stringValue` means different things across NSControl
        // subclasses (NSTextField → user text; NSButton → "0"/"1"
        // or the title; NSImageView → the NSImage's debug
        // description including pointer; NSDatePicker → the
        // formatted date + locale; sliders/steppers → numeric).
        // Only compare it for the leaf NSTextField shape, which
        // is where we drive user-visible string state.
        let is_textfield = any_a.downcast_ref::<NSTextField>().is_some();
        if is_textfield {
            let av: String = ca.stringValue().to_string();
            let bv: String = cb.stringValue().to_string();
            if av != bv {
                return Err(format!(
                    "{path}: stringValue mismatch ({av:?} vs {bv:?})"
                ));
            }
        }
        let aen = ca.isEnabled();
        let ben = cb.isEnabled();
        if aen != ben {
            return Err(format!(
                "{path}: isEnabled mismatch ({aen} vs {ben})"
            ));
        }
    }

    if let (Some(btn_a), Some(btn_b)) = (
        any_a.downcast_ref::<NSButton>(),
        any_b.downcast_ref::<NSButton>(),
    ) {
        // NSButton state — relevant for checkboxes / switches.
        let asn = btn_a.state();
        let bsn = btn_b.state();
        if asn != bsn {
            return Err(format!(
                "{path}: NSButton state mismatch ({asn:?} vs {bsn:?})"
            ));
        }
        let at: String = btn_a.title().to_string();
        let bt: String = btn_b.title().to_string();
        if at != bt {
            return Err(format!(
                "{path}: NSButton title mismatch ({at:?} vs {bt:?})"
            ));
        }
    }

    if let (Some(tf_a), Some(tf_b)) = (
        any_a.downcast_ref::<NSTextField>(),
        any_b.downcast_ref::<NSTextField>(),
    ) {
        let ap: String = tf_a
            .placeholderString()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let bp: String = tf_b
            .placeholderString()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if ap != bp {
            return Err(format!(
                "{path}: NSTextField placeholder mismatch ({ap:?} vs {bp:?})"
            ));
        }
    }

    // NSControl subclasses (NSButton, NSTextField, NSSlider, ...)
    // own implementation-detail private subviews — NSWidgetView,
    // NSButtonTextField, cell-backing things — that materialise
    // lazily and inconsistently. We don't drive those from the
    // spec; comparing them is pure flake. Stop recursion at the
    // first NSControl boundary.
    if any_a.downcast_ref::<NSControl>().is_some() {
        return Ok(());
    }

    // Recurse into subviews.
    let a_subs: Retained<objc2_foundation::NSArray<NSView>> = a.subviews();
    let b_subs: Retained<objc2_foundation::NSArray<NSView>> = b.subviews();
    let a_n = a_subs.count();
    let b_n = b_subs.count();
    if a_n != b_n {
        return Err(format!(
            "{path}: subview count mismatch ({a_n} vs {b_n}); \
             a={} b={}",
            list_subview_classes(&a_subs),
            list_subview_classes(&b_subs),
        ));
    }
    for i in 0..a_n {
        let ai = a_subs.objectAtIndex(i);
        let bi = b_subs.objectAtIndex(i);
        let child_path = format!("{path}/{i}({})", class_name(&ai));
        compare_node(&ai, &bi, &child_path)?;
    }

    Ok(())
}

fn class_name(view: &NSView) -> String {
    // The runtime class can be a subclass (e.g. FlippedView).
    // Also strip the `NSKVONotifying_` prefix ObjC adds when KVO
    // is observing a property — that swizzling is non-deterministic
    // (one tree may have a property observed, another may not) and
    // both trees are semantically the same class.
    let cls = view.class();
    let raw = cls.name().to_string_lossy().into_owned();
    raw.strip_prefix("NSKVONotifying_")
        .map(str::to_owned)
        .unwrap_or(raw)
}

fn approx_rect(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.5
}

fn list_subview_classes(arr: &objc2_foundation::NSArray<NSView>) -> String {
    let mut out = String::new();
    out.push('[');
    for i in 0..arr.count() {
        if i > 0 {
            out.push_str(", ");
        }
        let v = arr.objectAtIndex(i);
        out.push_str(&class_name(&v));
    }
    out.push(']');
    out
}
