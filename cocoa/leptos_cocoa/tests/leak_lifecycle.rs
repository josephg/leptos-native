//! Leak-detection tests for the cocoa port.
//!
//! These tests assert that the thread-local handler/store/tree
//! bookkeeping returns to baseline after a mount/unmount cycle, and
//! (the more aggressive case) that simply dropping a built View state
//! without explicit `unmount()` does not leave entries behind.
//!
//! The "drop without unmount" tests are the ones that catch the
//! documented `HANDLER_STORE`/`TEXT_FIELD_STORE` leaks. Until a Drop
//! safety net lands, those tests are expected to fail — that's the
//! point.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::event::{
    handler_store_size_for_test, text_field_store_size_for_test,
    text_view_store_size_for_test,
};
use leptos::prelude::*;
use leptos_cocoa::cocoa::bind::BindAttribute;
use leptos_cocoa::cocoa::element::{button, hstack, label, text_field, vstack};
use leptos_cocoa::event_macos::{click, on};
use reactive_graph::owner::Owner;
use renderer::view::{AddAnyAttr, Mountable, Render};

/// Set up a reactive Owner just for the test body, then drop it.
/// Effects created inside re-subscribe to whatever signals exist
/// during their lifetime; dropping the Owner disposes effects.
///
/// Wrapped in `autoreleasepool` so any NSView / NSControl /
/// NSToolbarItem that AppKit autoreleased during construction
/// actually deallocates by the time the closure returns —
/// associated objects (our handler stores) release in dealloc.
/// We also pump the run loop briefly so dispatch-queue async
/// tasks (e.g. `bind:value`'s spawned closure) complete and drop
/// their captured Elements before we snapshot.
fn with_scope<R>(f: impl FnOnce() -> R) -> R {
    let _ = cocoa_dom::spawner::init();
    objc2::rc::autoreleasepool(|_| {
        let owner = Owner::new();
        let result = owner.with(f);
        drop(owner);
        common::pump_run_loop(0.05);
        result
    })
}

fn snapshot() -> (usize, usize, usize) {
    (
        handler_store_size_for_test(),
        text_field_store_size_for_test(),
        text_view_store_size_for_test(),
    )
}

// ---------------------------------------------------------------------
// Baseline: explicit unmount works.
// ---------------------------------------------------------------------

fn explicit_unmount_clears_button_handler() {
    let _mtm = common::test_mtm();
    let before = snapshot();

    with_scope(|| {
        let view = button()
            .title("OK")
            .add_any_attr((on(click, |_: ()| {}),));
        let mut state = view.build();
        // No explicit mount in this test (we're not driving a window);
        // build alone installs the click handler via target/action.
        let installed = snapshot();
        assert!(
            installed.0 > before.0,
            "expected HANDLER_STORE to grow after build; before={:?} installed={:?}",
            before,
            installed,
        );
        state.unmount();
    });

    let after = snapshot();
    assert_eq!(
        after, before,
        "explicit unmount should return all stores to baseline; \
         before={:?} after={:?}",
        before, after,
    );
}

fn explicit_unmount_clears_text_field_delegate() {
    let _mtm = common::test_mtm();
    let before = snapshot();

    with_scope(|| {
        let value = RwSignal::new(String::new());
        let view = text_field().bind(leptos_cocoa::attr::Value, value);
        let mut state = view.build();
        let installed = snapshot();
        assert!(
            installed.1 > before.1,
            "expected TEXT_FIELD_STORE to grow after build; before={:?} installed={:?}",
            before,
            installed,
        );
        state.unmount();
    });

    let after = snapshot();
    assert_eq!(
        after, before,
        "explicit unmount should clear text-field delegate; \
         before={:?} after={:?}",
        before, after,
    );
}

// ---------------------------------------------------------------------
// Defense in depth: drop without unmount should also clean up.
// ---------------------------------------------------------------------
//
// Today this FAILS for the cocoa port — entries leak. Once the Drop
// safety net is added to Node/Element, these tests should pass.

fn drop_without_unmount_clears_button_handler() {
    let _mtm = common::test_mtm();
    let before = snapshot();

    with_scope(|| {
        let view = button()
            .title("OK")
            .add_any_attr((on(click, |_: ()| {}),));
        let state = view.build();
        // Deliberately drop without calling unmount.
        drop(state);
    });

    let after = snapshot();
    assert_eq!(
        after, before,
        "dropping a built state without unmount must not leak \
         handler-store entries; before={:?} after={:?}",
        before, after,
    );
}

fn drop_without_unmount_clears_text_field_delegate() {
    let _mtm = common::test_mtm();
    let before = snapshot();

    with_scope(|| {
        let value = RwSignal::new(String::new());
        let view = text_field().bind(leptos_cocoa::attr::Value, value);
        let state = view.build();
        drop(state);
        // bind:value installs a `RenderEffect` whose async runner
        // (driven by the dispatch-queue spawner) captures an
        // Element clone. Dropping the RenderEffect handle signals
        // its receiver but the task only drops its captured
        // closure after the runtime polls it once more — which
        // means the run loop must turn. Without this pump, the
        // captured Element keeps the TeardownGuard alive past
        // the snapshot.
        common::pump_run_loop(0.05);
    });

    let after = snapshot();
    assert_eq!(
        after, before,
        "dropping a built state without unmount must not leak \
         text-field delegate entries; before={:?} after={:?}",
        before, after,
    );
}

/// Isolation test: a text field with NO bind, just a raw on:change
/// handler. Used to determine whether the documented bind leak is
/// from the bind machinery (RenderEffect capturing Element) or from
/// the TextFieldDelegate retention itself.
fn drop_without_unmount_clears_text_field_no_bind() {
    use leptos_cocoa::event_macos::{input, on};
    let _mtm = common::test_mtm();
    let before = snapshot();

    with_scope(|| {
        let view = text_field()
            .add_any_attr((on(input, |_value: String| {}),));
        let state = view.build();
        drop(state);
    });

    let after = snapshot();
    assert_eq!(
        after, before,
        "text_field with raw on:change handler must clean up on drop; \
         before={:?} after={:?}",
        before, after,
    );
}

// ---------------------------------------------------------------------
// Composite tree — exercise the full vstack/hstack cascade.
// ---------------------------------------------------------------------

fn composite_tree_explicit_unmount_clears_all() {
    let _mtm = common::test_mtm();
    let before = snapshot();

    with_scope(|| {
        let view = vstack()
            .child(button().title("A").add_any_attr((on(click, |_: ()| {}),)))
            .child(
                hstack()
                    .child(label().text("Hello"))
                    .child(
                        button()
                            .title("B")
                            .add_any_attr((on(click, |_: ()| {}),)),
                    ),
            )
            .child({
                let v = RwSignal::new(String::new());
                text_field().bind(leptos_cocoa::attr::Value, v)
            });
        let mut state = view.build();
        let installed = snapshot();
        assert!(
            installed.0 >= before.0 + 2,
            "expected at least 2 HANDLER_STORE entries (two buttons); \
             before={:?} installed={:?}",
            before,
            installed,
        );
        assert!(
            installed.1 > before.1,
            "expected TEXT_FIELD_STORE entry from text_field; \
             before={:?} installed={:?}",
            before,
            installed,
        );
        state.unmount();
    });

    let after = snapshot();
    assert_eq!(
        after, before,
        "composite tree unmount should clear every store; \
         before={:?} after={:?}",
        before, after,
    );
}

fn composite_tree_drop_without_unmount_clears_all() {
    let _mtm = common::test_mtm();
    let before = snapshot();

    with_scope(|| {
        let view = vstack()
            .child(button().title("A").add_any_attr((on(click, |_: ()| {}),)))
            .child(
                hstack()
                    .child(label().text("Hello"))
                    .child(
                        button()
                            .title("B")
                            .add_any_attr((on(click, |_: ()| {}),)),
                    ),
            );
        let state = view.build();
        drop(state);
    });

    let after = snapshot();
    assert_eq!(
        after, before,
        "dropping a composite tree without unmount must not leak; \
         before={:?} after={:?}",
        before, after,
    );
}

/// `<For>` keyed-diff add/remove cycle, exercised via direct calls
/// to leptos's `For` component (the `view!{}` macro isn't in scope
/// in this crate's tests). Push 100 rows, clear them, drop. Stores
/// must return to baseline.
fn for_diff_add_then_clear_clears_handlers() {
    use leptos::control_flow::{For, ForProps};
    use leptos::prelude::*;
    use leptos_cocoa::cocoa::element::{hstack, label, vstack};
    use std::marker::PhantomData;
    let _mtm = common::test_mtm();

    let before = snapshot();

    with_scope(|| {
        let rows = RwSignal::new(Vec::<i32>::new());

        let for_view = For(ForProps::builder()
            .each(move || rows.get())
            .key(|i: &i32| *i)
            .children(move |i: i32| {
                hstack()
                    .child(
                        button()
                            .title("-1")
                            .add_any_attr((on(click, move |_: ()| {
                                let _ = i;
                            }),)),
                    )
                    .child(label().text(i.to_string()))
                    .child(
                        button()
                            .title("+1")
                            .add_any_attr((on(click, move |_: ()| {
                                let _ = i;
                            }),)),
                    )
            })
            ._marker(PhantomData)
            .build());
        let view = vstack().child(for_view);
        let mut state = view.build();

        // Push 100 rows.
        rows.update(|r| {
            for i in 0..100 {
                r.push(i);
            }
        });
        common::pump_run_loop(0.5);

        let after_add = snapshot();
        assert!(
            after_add.0 >= before.0 + 200,
            "expected at least 200 HANDLER_STORE entries after \
             adding 100 rows × 2 buttons; before={:?} after_add={:?}",
            before, after_add,
        );

        // Clear via For-diff.
        rows.update(|r| r.clear());
        common::pump_run_loop(0.5);

        let after_clear = snapshot();
        assert_eq!(
            after_clear.0, before.0,
            "For diff didn't reclaim per-row handlers; before={:?} \
             after_clear={:?}",
            before, after_clear,
        );

        state.unmount();
        common::pump_run_loop(0.1);
    });

    let after = snapshot();
    assert_eq!(
        after, before,
        "after full unmount + pump, all stores should match \
         baseline; before={:?} after={:?}",
        before, after,
    );
}

/// Reactive `Either<A, B>` branch toggle. Flipping the variant
/// must unmount the previous branch's resources every time.
/// Equivalent in shape to a `<Show>` flip; tested via a hand-
/// rolled signal-driven Either to avoid `ShowProps` builder
/// dance.
fn either_toggle_clears_handlers() {
    use either_of::Either;
    use leptos::prelude::*;
    use leptos_cocoa::cocoa::element::{label, vstack};
    let _mtm = common::test_mtm();

    let before = snapshot();

    with_scope(|| {
        let flag = RwSignal::new(false);

        let view = vstack().child(move || match flag.get() {
            false => Either::Left(label().text("off")),
            true => Either::Right(
                vstack()
                    .child(
                        button()
                            .title("A")
                            .add_any_attr((on(click, |_: ()| {}),)),
                    )
                    .child(
                        button()
                            .title("B")
                            .add_any_attr((on(click, |_: ()| {}),)),
                    ),
            ),
        });
        let mut state = view.build();

        for _ in 0..50 {
            flag.update(|v| *v = !*v);
            common::pump_run_loop(0.05);
        }

        flag.set(false);
        common::pump_run_loop(0.1);

        let after_toggling = snapshot();
        assert_eq!(
            after_toggling.0, before.0,
            "Either toggle accumulated handler entries; before={:?} \
             after={:?}",
            before, after_toggling,
        );

        state.unmount();
        common::pump_run_loop(0.1);
    });

    let after = snapshot();
    assert_eq!(
        after, before,
        "after unmount, stores must match baseline; before={:?} \
         after={:?}",
        before, after,
    );
}

/// Same as `for_diff_add_then_clear_clears_handlers` but with
/// random reordering between add and clear. The For keyed-diff
/// `move_cmds` path is what we're stressing here — see the audit
/// in plan A4 for the suspected overwrite-without-unmount case.
fn for_diff_shuffle_then_clear_clears_handlers() {
    use leptos::control_flow::{For, ForProps};
    use leptos::prelude::*;
    use leptos_cocoa::cocoa::element::{hstack, label, vstack};
    use std::marker::PhantomData;
    let _mtm = common::test_mtm();

    let before = snapshot();

    with_scope(|| {
        let rows = RwSignal::new(Vec::<i32>::new());

        let for_view = For(ForProps::builder()
            .each(move || rows.get())
            .key(|i: &i32| *i)
            .children(move |i: i32| {
                hstack()
                    .child(
                        button()
                            .title("-")
                            .add_any_attr((on(click, move |_: ()| {
                                let _ = i;
                            }),)),
                    )
                    .child(label().text(i.to_string()))
            })
            ._marker(PhantomData)
            .build());
        let view = vstack().child(for_view);
        let mut state = view.build();

        rows.update(|r| {
            for i in 0..50 {
                r.push(i);
            }
        });
        common::pump_run_loop(0.3);

        // Several shuffles (deterministic xorshift, no extra deps).
        for round in 0..5 {
            rows.update(|cs| {
                let len = cs.len();
                let mut seed: u64 = round as u64 + 1;
                for i in (1..len).rev() {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    let j = (seed as usize) % (i + 1);
                    cs.swap(i, j);
                }
            });
            common::pump_run_loop(0.2);
        }

        rows.update(|r| r.clear());
        common::pump_run_loop(0.3);

        let after_clear = snapshot();
        assert_eq!(
            after_clear.0, before.0,
            "shuffle+clear must leave HANDLER_STORE at baseline; \
             before={:?} after_clear={:?}",
            before, after_clear,
        );

        state.unmount();
        common::pump_run_loop(0.1);
    });

    let after = snapshot();
    assert_eq!(
        after, before,
        "after unmount + pump, stores must match baseline; \
         before={:?} after={:?}",
        before, after,
    );
}

fn main() {
    common::run_tests(&[
        (
            "explicit_unmount_clears_button_handler",
            explicit_unmount_clears_button_handler,
        ),
        (
            "explicit_unmount_clears_text_field_delegate",
            explicit_unmount_clears_text_field_delegate,
        ),
        (
            "composite_tree_explicit_unmount_clears_all",
            composite_tree_explicit_unmount_clears_all,
        ),
        (
            "drop_without_unmount_clears_button_handler",
            drop_without_unmount_clears_button_handler,
        ),
        (
            "drop_without_unmount_clears_text_field_delegate",
            drop_without_unmount_clears_text_field_delegate,
        ),
        (
            "composite_tree_drop_without_unmount_clears_all",
            composite_tree_drop_without_unmount_clears_all,
        ),
        (
            "drop_without_unmount_clears_text_field_no_bind",
            drop_without_unmount_clears_text_field_no_bind,
        ),
        (
            "for_diff_add_then_clear_clears_handlers",
            for_diff_add_then_clear_clears_handlers,
        ),
        (
            "for_diff_shuffle_then_clear_clears_handlers",
            for_diff_shuffle_then_clear_clears_handlers,
        ),
        (
            "either_toggle_clears_handlers",
            either_toggle_clears_handlers,
        ),
    ]);
}
