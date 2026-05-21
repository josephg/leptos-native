//! Smoke tests for `cocoa_dom::animation`.
//!
//! Verifies (a) the TLS slot is `None` before/after a
//! `with_animation` burst, (b) `current_animation()` returns
//! `Some` synchronously inside the body, (c) FIFO ordering so a
//! `spawn_local`-queued task that runs *before* the cleanup still
//! sees the animation context, and (d) a panic in `body` doesn't
//! permanently stick the slot.

#![cfg(target_os = "macos")]
#![cfg(feature = "animation")]

mod common;

use leptos_cocoa::dom::animation::{current_animation, with_animation, Animation};
use leptos_cocoa::dom::spawner;
use std::{
    cell::Cell,
    rc::Rc,
};

fn slot_empty_before_and_after() {
    let _mtm = common::test_mtm();
    let _ = spawner::init().unwrap();
    assert!(current_animation().is_none(), "clean state");

    with_animation(Animation::ease_in_out(0.2), || {
        assert!(current_animation().is_some(), "set during body");
    });

    // Cleanup is async; drain the run loop so spawn_local fires.
    common::pump_run_loop(0.05);
    assert!(current_animation().is_none(), "cleared after drain");
}

fn body_sees_animation_synchronously() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();
    let observed = Rc::new(Cell::new(None));
    let obs = observed.clone();
    with_animation(Animation::linear(0.1), move || {
        obs.set(current_animation());
    });
    assert!(
        observed.get().is_some(),
        "body observed Some(_) inside with_animation"
    );
    common::pump_run_loop(0.05);
}

fn fifo_keeps_animation_for_queued_effect() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();
    let saw_animation_in_task = Rc::new(Cell::new(false));
    let flag = saw_animation_in_task.clone();

    with_animation(Animation::linear(0.1), move || {
        // Mimic what a RenderEffect re-run does: spawn_local a
        // task that touches the setter chain. Because the
        // CleanupGuard queues its restore via spawn_local AT END
        // OF body, our task here is queued first → must see the
        // animation.
        any_spawner::Executor::spawn_local(async move {
            flag.set(current_animation().is_some());
        });
    });

    common::pump_run_loop(0.1);
    assert!(
        saw_animation_in_task.get(),
        "queued effect saw animation context"
    );
    assert!(
        current_animation().is_none(),
        "slot cleared after both tasks drained"
    );
}

fn panic_in_body_still_clears_slot() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();

    let result = std::panic::catch_unwind(|| {
        with_animation(Animation::linear(0.1), || {
            panic!("simulated handler panic");
        })
    });
    assert!(result.is_err(), "panic propagated");

    // Drop guard should have queued the cleanup before unwind.
    common::pump_run_loop(0.05);
    assert!(
        current_animation().is_none(),
        "slot cleared despite panic"
    );
}

fn nested_restores_outer() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();
    let outer = Animation::linear(1.0);
    let inner = Animation::linear(0.1);
    let post_inner = Rc::new(Cell::new(None));
    let pi = post_inner.clone();

    with_animation(outer, move || {
        with_animation(inner, || {
            assert!(
                (current_animation().unwrap().duration - 0.1).abs() < 1e-6
            );
        });
        // Synchronously after inner returns, TLS is still inner's
        // value — the restore is async. That's expected; what
        // matters is what queued effects see.
        any_spawner::Executor::spawn_local(async move {
            pi.set(current_animation());
        });
    });

    common::pump_run_loop(0.1);
    // After inner's cleanup runs (queued first), the spawn_local
    // task above runs and should see outer (1.0s).
    let observed = post_inner.get().expect("task ran");
    assert!(
        (observed.duration - 1.0).abs() < 1e-6,
        "post-inner queued task saw outer (got {:?})",
        observed
    );
    assert!(current_animation().is_none(), "fully drained at end");
}

fn main() {
    common::run_tests(&[
        ("slot_empty_before_and_after", slot_empty_before_and_after),
        ("body_sees_animation_synchronously", body_sees_animation_synchronously),
        ("fifo_keeps_animation_for_queued_effect", fifo_keeps_animation_for_queued_effect),
        ("panic_in_body_still_clears_slot", panic_in_body_still_clears_slot),
        ("nested_restores_outer", nested_restores_outer),
    ]);
}
