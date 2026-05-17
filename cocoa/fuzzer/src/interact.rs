//! XCUI-style interaction: walk the mounted NSView tree and drive
//! every interactable element via AppKit.
//!
//! "XCUI mode" here means *exercising the same code paths* that
//! Apple's XCUITest framework would (`performClick`, synthesised
//! `controlTextDidChange:` delegate calls) — without requiring an
//! actual XCUITest harness, a visible app, or accessibility
//! entitlements. Real OS-event-level automation can layer on top
//! later (cliclick / `xctrace`) by re-using the same walk.
//!
//! Behaviour per element:
//!   * `NSButton` (incl. checkboxes & switches) → `performClick:`
//!     fires its target/action AND, for two-state buttons,
//!     toggles state. `bind:checked` writes back to the signal.
//!   * `NSTextField` → set a fresh `stringValue` and synthesise
//!     `controlTextDidChange:` on its delegate. `bind:value`
//!     writes back to the signal.
//!
//! Anything else is ignored (we recurse into subviews regardless).

use objc2::{
    msg_send,
    rc::Retained,
    runtime::AnyObject,
};
use objc2_app_kit::{
    NSButton, NSControl, NSPopUpButton, NSSegmentedControl, NSSlider,
    NSStepper, NSTextField, NSView,
};
use objc2_foundation::{NSNotification, NSString};
use rand::{seq::SliceRandom, Rng};
use rand_chacha::ChaCha8Rng;

/// Walk `root` and trigger one interaction per interactable
/// descendant. Returns counts for diagnostics.
pub fn drive(root: &NSView, rng: &mut ChaCha8Rng) -> Stats {
    let mut s = Stats::default();
    walk(root, rng, &mut s);
    s
}

#[derive(Default, Debug)]
pub struct Stats {
    pub buttons_clicked: usize,
    pub text_fields_typed: usize,
}

fn walk(view: &NSView, rng: &mut ChaCha8Rng, stats: &mut Stats) {
    let any: &AnyObject = view.as_ref();

    // Skip NSPopUpButton / NSSegmentedControl: `performClick` on
    // a popup OPENS the menu without picking, which (a) leaves
    // the menu visibly open if `--show` is on, and (b) doesn't
    // change the selection signal, so it's pure noise. Driving
    // them properly needs `selectItemAtIndex:` + `sendAction:`
    // pairs — todo.
    if any.downcast_ref::<NSPopUpButton>().is_some()
        || any.downcast_ref::<NSSegmentedControl>().is_some()
    {
        return;
    }

    // NSSlider / NSStepper: `performClick` does nothing useful on
    // them (no menu, no toggle). To exercise them we'd need to
    // synthesise a value change. Skip until we wire that path.
    if any.downcast_ref::<NSSlider>().is_some()
        || any.downcast_ref::<NSStepper>().is_some()
    {
        return;
    }

    // NSButton covers push buttons, checkboxes, switches, radio
    // buttons. Skip if no target is set: (a) there'd be no
    // handler to fire, (b) for checkboxes `performClick` would
    // toggle state with no signal to roundtrip the change.
    if let Some(button) = any.downcast_ref::<NSButton>() {
        if button.target().is_some() {
            unsafe { button.performClick(None) };
            stats.buttons_clicked += 1;
        }
        return;
    }
    if let Some(field) = any.downcast_ref::<NSTextField>() {
        if field.delegate().is_some() {
            type_into(field, rng);
            stats.text_fields_typed += 1;
        }
        return;
    }
    // NSControl catch-all (date_picker, color_well, image_view,
    // progress_indicator, ...) — no generic interaction worth
    // driving from here.
    if any.downcast_ref::<NSControl>().is_some() {
        return;
    }
    let subs = view.subviews();
    for i in 0..subs.count() {
        let sv = subs.objectAtIndex(i);
        walk(&sv, rng, stats);
    }
}

fn type_into(field: &NSTextField, rng: &mut ChaCha8Rng) {
    let dict = ["typed", "abc", "xy", "input", "test", "hello"];
    let n = rng.gen_range(1..=2);
    let mut s = String::new();
    for i in 0..n {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(dict.choose(rng).unwrap());
    }
    let ns = NSString::from_str(&s);
    field.setStringValue(&ns);

    // Drive `controlTextDidChange:` directly on the delegate —
    // same mechanism `cocoa_dom`'s tests use to exercise text
    // input without a field editor / live focus. If no delegate
    // is wired (no `on:input` / `bind:value`), this is a no-op.
    let Some(delegate) = field.delegate() else {
        return;
    };
    let name = NSString::from_str("NSControlTextDidChangeNotification");
    let object: &AnyObject = field.as_ref();
    let notif: Retained<NSNotification> = unsafe {
        NSNotification::notificationWithName_object(&name, Some(object))
    };
    let delegate_any: &AnyObject = (*delegate).as_ref();
    let _: () = unsafe {
        msg_send![delegate_any, controlTextDidChange: &*notif]
    };
}
