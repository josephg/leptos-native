//! Shared test helpers — main-thread guard, action-firing, etc.
//!
//! AppKit demands the main thread for almost every API. Cargo's
//! default test harness spawns a worker thread per test, which
//! makes `MainThreadMarker::new()` return `None`. Each test file
//! uses `harness = false` (configured in `cocoa_dom/Cargo.toml`)
//! and calls [`run_main_thread_tests!`] from its `fn main()` to
//! run the test bodies sequentially on the actual main thread.
//!
//! Each "test" is a `fn() -> ()` whose name is registered with the
//! macro. On any panic the macro prints the test name and the panic
//! payload, then continues with the rest, mimicking the libtest
//! summary at the end.

#![cfg(target_os = "macos")]
#![allow(dead_code)] // helpers used by some test files but not all

use cocoa_dom::MainThreadMarker;
use objc2::{msg_send, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSControl, NSView};
use objc2_foundation::{NSNotification, NSString};

/// `MainThreadMarker` for the test runner's main thread. Panics if
/// the test isn't running on the main thread.
pub fn test_mtm() -> MainThreadMarker {
    MainThreadMarker::new().expect(
        "AppKit tests must run on the main thread. If this panics, \
         the test runner may have moved threads — try \
         `--test-threads=1` or check your test setup.",
    )
}

/// Read `target` and `action` off an NSControl and dispatch the
/// action via the ObjC runtime. The "synthetic click" — exercises
/// target/action handlers without an event loop or window.
///
/// Panics if the control has no action set (i.e. nothing was wired
/// up).
pub fn fire_action(control: &NSControl) {
    let _action = control
        .action()
        .expect("fire_action: control has no action set");
    let target = control
        .target()
        .expect("fire_action: control has nil target (first-responder dispatch — needs XCUIAutomation)");

    // Invoke `actionFired:` directly. We can't use
    // `performSelector:withObject:` because its declared return type
    // is `id` while our actionFired: returns void — objc2 type-
    // checks msg_send! and either rejects the wrong return type or
    // segfaults trying to retain a garbage return value.
    //
    // Hardcoding the selector keeps tests working with strict
    // type-checked msg_send. If the selector name in cocoa_dom's
    // ActionTarget ever changes, update here too.
    let target_any: &AnyObject = &*target;
    let control_any: &AnyObject = control.as_ref();
    let _: () = unsafe { msg_send![target_any, actionFired: control_any] };
}

/// Synthesise `textDidChange:` on an NSTextView. Use this AFTER
/// programmatically setting the text view's `string` so handlers
/// see the new value. Mirrors `fire_text_did_change` for
/// NSTextField — we invoke the delegate method directly rather
/// than posting through NSNotificationCenter.
pub fn fire_text_view_did_change(tv: &objc2_app_kit::NSTextView) {
    let delegate = tv
        .delegate()
        .expect("fire_text_view_did_change: tv has no delegate");
    let notif = synth_notification(
        "NSTextDidChangeNotification",
        tv.as_ref(),
    );
    let _: () = unsafe {
        msg_send![&*delegate, textDidChange: &*notif]
    };
}

/// Synthesise `controlTextDidChange:` on a text field. Use this
/// AFTER programmatically setting the field's stringValue so
/// handlers see the new value.
///
/// We invoke the delegate method DIRECTLY (via `msg_send!`) rather
/// than posting a notification. AppKit posts these notifications
/// from inside NSControl when the field editor changes; merely
/// posting the notification ourselves does not necessarily route
/// to the delegate (it depends on whether AppKit registered the
/// delegate as an observer, which is opaque). Direct invocation
/// matches what NSControl ultimately does and is independent of
/// AppKit's plumbing.
pub fn fire_text_did_change(field: &objc2_app_kit::NSTextField) {
    let delegate = field
        .delegate()
        .expect("fire_text_did_change: field has no delegate");
    let notif = synth_notification(
        "NSControlTextDidChangeNotification",
        field.as_ref(),
    );
    let _: () = unsafe {
        msg_send![&*delegate, controlTextDidChange: &*notif]
    };
}

/// Synthesise `controlTextDidBeginEditing:` (focus gained).
/// Drives `on:focus` handlers.
pub fn fire_text_did_begin_editing(
    field: &objc2_app_kit::NSTextField,
) {
    let delegate = field
        .delegate()
        .expect("fire_text_did_begin_editing: field has no delegate");
    let notif = synth_notification(
        "NSControlTextDidBeginEditingNotification",
        field.as_ref(),
    );
    let _: () = unsafe {
        msg_send![&*delegate, controlTextDidBeginEditing: &*notif]
    };
}

/// Synthesise `control:textView:doCommandBySelector:` for the
/// given selector. Drives `on:keydown` / `on:keyup` handlers.
/// We don't have a real NSTextView in unit tests, so we pass the
/// field cast to NSTextView's slot — our delegate handler
/// doesn't dereference the text_view argument, so this is safe
/// (and matches what AppKit does shape-wise: the field editor
/// IS-A NSTextView).
pub fn fire_text_did_command(
    field: &objc2_app_kit::NSTextField,
    command: objc2::runtime::Sel,
) -> bool {
    use objc2::runtime::Bool;
    let delegate = field
        .delegate()
        .expect("fire_text_did_command: field has no delegate");
    // Pass the field where a text_view is expected. The delegate
    // method we implemented ignores the text_view argument, and
    // the ObjC method dispatcher doesn't type-check arguments
    // beyond size/alignment.
    let field_any: &AnyObject = field.as_ref();
    let result: Bool = unsafe {
        msg_send![&*delegate,
            control: field_any,
            textView: field_any,
            doCommandBySelector: command
        ]
    };
    result.as_bool()
}

/// Synthesise `controlTextDidEndEditing:` (commit — return key /
/// focus loss). See [`fire_text_did_change`] for why we invoke
/// directly instead of posting a notification.
pub fn fire_text_did_end_editing(field: &objc2_app_kit::NSTextField) {
    let delegate = field
        .delegate()
        .expect("fire_text_did_end_editing: field has no delegate");
    let notif = synth_notification(
        "NSControlTextDidEndEditingNotification",
        field.as_ref(),
    );
    let _: () = unsafe {
        msg_send![&*delegate, controlTextDidEndEditing: &*notif]
    };
}

/// Build an `NSNotification` with the given name and object. Used
/// to feed our text-field delegate methods the same shape of
/// argument they'd receive from AppKit at runtime.
fn synth_notification(
    name: &str,
    object: &AnyObject,
) -> Retained<NSNotification> {
    let name = NSString::from_str(name);
    unsafe {
        NSNotification::notificationWithName_object(&name, Some(object))
    }
}

/// Suppress unused-warnings for items we expose for use across tests
/// even though some test files don't touch all of them.
#[allow(dead_code)]
fn _force_link() -> Option<Retained<NSView>> {
    None
}

/// Pump the main run loop briefly so any dispatched futures fire.
///
/// Our spawner uses `DispatchQueue::main().exec_async` to schedule
/// `RenderEffect` polls. Without a running loop, those don't fire.
/// In unit tests, calling this after a `signal.set(...)` lets the
/// Effect chain catch up before assertions.
pub fn pump_run_loop(timeout_secs: f64) {
    // Raw FFI to `CFRunLoopRunInMode` (CoreFoundation). Avoids
    // adding a dep just for this; the constants are stable.
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFRunLoopDefaultMode: *const std::ffi::c_void;
        fn CFRunLoopRunInMode(
            mode: *const std::ffi::c_void,
            seconds: f64,
            return_after_source_handled: bool,
        ) -> i32;
    }
    unsafe {
        // returnAfterSourceHandled=false → drain until timeout or
        // no work remaining.
        let _ =
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, timeout_secs, false);
    }
}

/// Custom test runner: runs each `fn()` on the current (main)
/// thread, catches panics, prints a libtest-style summary, and
/// exits with code 1 on any failure.
///
/// Usage from a `harness = false` test file:
///
/// ```ignore
/// fn main() {
///     run_tests(&[
///         ("button_is_nsbutton", button_is_nsbutton),
///         ("checkbox_is_nsbutton", checkbox_is_nsbutton),
///         // ...
///     ]);
/// }
/// ```
pub fn run_tests(tests: &[(&'static str, fn())]) {
    let total = tests.len();
    println!("\nrunning {} tests", total);

    let mut passed = 0usize;
    let mut failed: Vec<(String, String)> = Vec::new();

    for (name, body) in tests {
        // catch_unwind requires UnwindSafe; for fn pointers it's
        // satisfied. Our test bodies don't share mutable state
        // across the boundary so we ignore the warning.
        let result = std::panic::catch_unwind(
            std::panic::AssertUnwindSafe(*body),
        );
        match result {
            Ok(()) => {
                println!("test {} ... ok", name);
                passed += 1;
            }
            Err(payload) => {
                let msg = downcast_panic(&payload);
                println!("test {} ... FAILED", name);
                failed.push((name.to_string(), msg));
            }
        }
    }

    println!();
    if failed.is_empty() {
        println!(
            "test result: ok. {} passed; 0 failed; 0 ignored",
            passed
        );
    } else {
        println!("failures:");
        for (name, msg) in &failed {
            println!();
            println!("---- {} stdout ----", name);
            println!("{}", msg);
        }
        println!();
        println!(
            "test result: FAILED. {} passed; {} failed; 0 ignored",
            passed,
            failed.len()
        );
        std::process::exit(1);
    }
}

fn downcast_panic(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}
