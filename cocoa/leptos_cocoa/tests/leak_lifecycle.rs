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

extern crate leptos_cocoa as leptos_platform;

mod common;

use leptos_cocoa::dom::event::{
    handler_store_size_for_test, text_field_store_size_for_test,
    text_view_store_size_for_test,
};
use leptos_native::prelude::*;
use leptos_cocoa::cocoa::bind::BindAttribute;
use leptos_cocoa::cocoa::element::{button, hstack, label, text_field, text_view, vstack};
use leptos_cocoa::event_macos::{click, on};
use reactive_graph::owner::Owner;
use leptos_native::renderer::view::{AddAnyAttr, Mountable, Render};

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
    // `init` is process-global; the custom harness runs every test in one
    // process, so only the first call succeeds. Ignore the `AlreadySet` the
    // rest return — it just means the executor is already wired up.
    let _ = leptos_cocoa::dom::spawner::init();
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
    use leptos_native::control_flow::{For, ForProps};
    use leptos_native::prelude::*;
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
    use leptos_native::prelude::*;
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
    use leptos_native::control_flow::{For, ForProps};
    use leptos_native::prelude::*;
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

// ---------------------------------------------------------------------
// text_view variants — isolating the residual leak the fuzzer reports.
// ---------------------------------------------------------------------

/// text_view with NO bind, no event handler — bare creation/drop.
fn drop_bare_text_view_clears_delegate() {
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let state = text_view().build();
        drop(state);
        common::pump_run_loop(0.1);
    });
    let after = snapshot();
    assert_eq!(
        after, before,
        "bare text_view drop must not leak; before={:?} after={:?}",
        before, after,
    );
}

/// text_view with bind:value — explicit unmount path.
fn explicit_unmount_clears_text_view_delegate() {
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let value = RwSignal::new(String::new());
        let view = text_view().bind(leptos_cocoa::attr::Value, value);
        let mut state = view.build();
        let installed = snapshot();
        assert!(
            installed.2 > before.2,
            "expected TEXT_VIEW counter to grow after build; \
             before={:?} installed={:?}",
            before, installed,
        );
        state.unmount();
        common::pump_run_loop(0.1);
    });
    let after = snapshot();
    assert_eq!(
        after, before,
        "explicit text_view unmount must clear delegate; \
         before={:?} after={:?}",
        before, after,
    );
}

/// Reactive child returning a text_view that's never toggled off.
/// Builds the dynamic-child code path the fuzzer uses for Show
/// without exercising any flip.
fn drop_reactive_child_text_view_no_toggle() {
    use leptos_cocoa::CocoaBackend;
    use leptos_native::renderer::view::{AnyView, IntoAny};
    use leptos_cocoa::cocoa::element::vstack;
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let value = RwSignal::new(String::new());
        let view = vstack().child(move || -> AnyView<CocoaBackend> {
            text_view().bind(leptos_cocoa::attr::Value, value).into_any()
        });
        let mut state = view.build();
        state.unmount();
        common::pump_run_loop(0.1);
    });
    let after = snapshot();
    assert_eq!(
        after, before,
        "reactive-child text_view with no toggle must not leak; \
         before={:?} after={:?}",
        before, after,
    );
}

/// Isolation: text_view inside Either with NO bind, just plain
/// creation. If THIS leaks, it's nothing to do with bind.
fn show_off_bare_text_view_clears_delegate() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let flag = RwSignal::new(true);
        let view = vstack().child(move || match flag.get() {
            true => Either::Left(text_view()),
            false => Either::Right(label().text("off")),
        });
        let mut state = view.build();
        flag.set(false);
        common::pump_run_loop(0.1);
        let after_off = snapshot();
        assert_eq!(
            after_off.2, before.2,
            "toggling off bare text_view must drop delegate; \
             before={:?} after_off={:?}",
            before, after_off,
        );
        state.unmount();
    });
}

/// Mirrors the fuzzer's full-teardown leak check. Build a tree
/// with a text_view+bind:value, do NOTHING with it (no toggle),
/// then explicit unmount + drop the Owner + pump extensively.
/// If the bundle still leaks after that, this matches the fuzzer
/// repro and rules out timing.
fn full_teardown_text_view_persistent_leak() {
    use leptos_cocoa::cocoa::element::vstack;
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let value = RwSignal::new(String::new());
        let view = vstack().child(
            text_view().bind(leptos_cocoa::attr::Value, value),
        );
        let mut state = view.build();
        state.unmount();
        for _ in 0..20 {
            common::pump_run_loop(0.02);
        }
    });
    objc2::rc::autoreleasepool(|_| {
        for _ in 0..20 {
            common::pump_run_loop(0.02);
        }
    });
    let after = snapshot();
    assert_eq!(
        after, before,
        "full teardown (owner-drop + pump) must clear the bundle; \
         before={:?} after={:?}",
        before, after,
    );
}

/// Like show_off_text_view_clears_delegate but with no signal
/// in the bind — just on:input registered on the text_view to
/// create the delegate, then toggle off. If THIS leaks, the
/// problem is in the unmount/drop path of the text_view delegate
/// regardless of the RenderEffect cycle.
fn show_off_text_view_with_oninput_clears_delegate() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    use leptos_cocoa::event_macos::{input, on};
    use leptos_native::renderer::view::AddAnyAttr;
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let flag = RwSignal::new(true);
        let view = vstack().child(move || match flag.get() {
            true => Either::Left(
                text_view()
                    .add_any_attr((on(input, |_v: String| {}),)),
            ),
            false => Either::Right(label().text("off")),
        });
        let mut state = view.build();
        flag.set(false);
        common::pump_run_loop(0.1);
        let after_off = snapshot();
        assert_eq!(
            after_off.2, before.2,
            "toggling off text_view+on:input must drop delegate; \
             before={:?} after_off={:?}",
            before, after_off,
        );
        state.unmount();
    });
}

/// Control case: text_field bind:value inside Either, toggled off.
/// If text_field also leaks here, the bug is in Either/RenderEffect
/// disposal, not text_view-specific.
fn show_off_text_field_clears_delegate() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let flag = RwSignal::new(true);
        let value = RwSignal::new(String::new());
        let view = vstack().child(move || match flag.get() {
            true => Either::Left(
                text_field().bind(leptos_cocoa::attr::Value, value),
            ),
            false => Either::Right(label().text("off")),
        });
        let mut state = view.build();
        flag.set(false);
        common::pump_run_loop(0.1);
        let after_off = snapshot();
        assert_eq!(
            after_off.1, before.1,
            "toggling off must drop text_field delegate; \
             before={:?} after_off={:?}",
            before, after_off,
        );
        state.unmount();
    });
    let after = snapshot();
    assert_eq!(
        after, before,
        "show-off then unmount must match baseline; \
         before={:?} after={:?}",
        before, after,
    );
}

#[cfg(any())]
fn diagnose_text_view_bind_extra_clones() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    use leptos_cocoa::cocoa::NodeRef;
    let _mtm = common::test_mtm();
    with_scope(|| {
        let flag = RwSignal::new(true);
        let value = RwSignal::new(String::new());
        let nref = NodeRef::new();
        let view = vstack().child(move || match flag.get() {
            true => Either::Left(
                text_view()
                    .node_ref(nref)
                    .bind(leptos_cocoa::attr::Value, value),
            ),
            false => Either::Right(label().text("off")),
        });
        let mut state = view.build();

        let el_before = nref.get().expect("text_view registered");
        let count_before = el_before.as_node().handlers_rc_count_for_test();
        let tv_alive_before = el_before
            .as_node()
            .handlers()
            .borrow()
            .text_view_delegate
            .is_some();
        eprintln!("TV before flip: count={count_before} delegate_in_bundle={tv_alive_before}");
        drop(el_before);

        flag.set(false);
        common::pump_run_loop(0.1);

        let el_after = nref.get();
        match el_after {
            Some(el) => {
                let c = el.as_node().handlers_rc_count_for_test();
                let tv_alive = el
                    .as_node()
                    .handlers()
                    .borrow()
                    .text_view_delegate
                    .is_some();
                eprintln!("TV after flip:  count={c} delegate_in_bundle={tv_alive}");
            }
            None => eprintln!("TV after flip: NodeRef cleared"),
        }
        state.unmount();
    });
}

/// Wrap the INITIAL BUILD in its own autoreleasepool. If TV1's
/// delegate is being held by an autoreleased object created
/// during build, draining the build pool BEFORE toggling should
/// release it.
#[cfg(any())]
fn build_in_pool_then_show_off() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    let _mtm = common::test_mtm();
    let before = snapshot();
    // `init` is process-global; the custom harness runs every test in one
    // process, so only the first call succeeds. Ignore the `AlreadySet` the
    // rest return — it just means the executor is already wired up.
    let _ = cocoa_dom::spawner::init();
    objc2::rc::autoreleasepool(|_| {
        let owner = reactive_graph::owner::Owner::new();
        let _result: () = owner.with(|| {
            let flag = RwSignal::new(true);
            let value = RwSignal::new(String::new());
            let view = vstack().child(move || match flag.get() {
                true => Either::Left(
                    text_view().bind(leptos_cocoa::attr::Value, value),
                ),
                false => Either::Right(label().text("off")),
            });
            let mut state = view.build();
            // Drain the build pool by entering and exiting an inner one.
            // Anything autoreleased during build releases here.
            objc2::rc::autoreleasepool(|_| {
                common::pump_run_loop(0.05);
            });
            let mid = snapshot();
            eprintln!("after build+drain: tv={}", mid.2 - before.2);
            flag.set(false);
            common::pump_run_loop(0.2);
            let after_off = snapshot();
            eprintln!("after flip-off:    tv={}", after_off.2 - before.2);
            state.unmount();
        });
        drop(owner);
        common::pump_run_loop(0.05);
    });
}

/// Start with flag=false so initial build doesn't create the
/// text_view. Then toggle on (create), then toggle off (drop).
/// If the leak only fires for "first text_view at build time",
/// this shouldn't leak. If it fires for "first text_view created
/// inside this reactive child," it WILL leak.
#[cfg(any())]
fn start_off_then_on_off() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let flag = RwSignal::new(false);
        let value = RwSignal::new(String::new());
        let view = vstack().child(move || match flag.get() {
            true => Either::Left(
                text_view().bind(leptos_cocoa::attr::Value, value),
            ),
            false => Either::Right(label().text("off")),
        });
        let mut state = view.build();
        flag.set(true);
        common::pump_run_loop(0.1);
        let after_on = snapshot();
        eprintln!("start-off then on:  tv now = {}", after_on.2 - before.2);
        flag.set(false);
        common::pump_run_loop(0.1);
        let after_off = snapshot();
        eprintln!("start-off then off: tv leak = {}", after_off.2 - before.2);
        state.unmount();
    });
}

/// Warm up by creating + immediately dropping a text_view+bind
/// before the leak test. If the leak is "first text_view in
/// process initialises some shared lazy state that holds the
/// instance," warmup should consume that and the actual test
/// should pass.
#[cfg(any())]
fn warmup_then_show_off_text_view() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    let _mtm = common::test_mtm();
    // Warmup: a separate scope first.
    with_scope(|| {
        let v = RwSignal::new(String::new());
        let mut s = text_view()
            .bind(leptos_cocoa::attr::Value, v)
            .build();
        s.unmount();
        common::pump_run_loop(0.1);
    });
    let before = snapshot();
    with_scope(|| {
        let flag = RwSignal::new(true);
        let value = RwSignal::new(String::new());
        let view = vstack().child(move || match flag.get() {
            true => Either::Left(
                text_view().bind(leptos_cocoa::attr::Value, value),
            ),
            false => Either::Right(label().text("off")),
        });
        let mut state = view.build();
        flag.set(false);
        common::pump_run_loop(0.2);
        let after_off = snapshot();
        eprintln!("post-warmup show-off: tv leak = {}", after_off.2 - before.2);
        state.unmount();
    });
}

/// Wrap the toggle in an inner autoreleasepool. If the leaked
/// holder is an autoreleased AppKit object, draining the inner
/// pool should release it before our assertion. If the test
/// still leaks, the holder is Rust-side.
#[cfg(any())]
fn show_off_text_view_inner_autoreleasepool() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let flag = RwSignal::new(true);
        let value = RwSignal::new(String::new());
        let view = vstack().child(move || match flag.get() {
            true => Either::Left(
                text_view().bind(leptos_cocoa::attr::Value, value),
            ),
            false => Either::Right(label().text("off")),
        });
        let mut state = view.build();
        objc2::rc::autoreleasepool(|_| {
            flag.set(false);
            common::pump_run_loop(0.2);
        });
        common::pump_run_loop(0.1);
        let after_off = snapshot();
        eprintln!("inner-pool show-off: tv leak = {}", after_off.2 - before.2);
        state.unmount();
    });
}

/// Toggle Either on/off many times. If the leak is per-iteration,
/// count grows linearly. If it's "last one survives", count stays
/// at 1.
#[cfg(any())]
fn diagnose_text_view_repeated_toggles() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let flag = RwSignal::new(true);
        let value = RwSignal::new(String::new());
        let view = vstack().child(move || match flag.get() {
            true => Either::Left(
                text_view().bind(leptos_cocoa::attr::Value, value),
            ),
            false => Either::Right(label().text("off")),
        });
        let mut state = view.build();
        eprintln!("DIAG: just after build, tv_count={}", snapshot().2 - before.2);
        for i in 0..10 {
            flag.update(|v| *v = !*v);
            common::pump_run_loop(0.05);
            let s = snapshot();
            eprintln!("after toggle {i}: tv_count={}", s.2 - before.2);
        }
        state.unmount();
    });
}

/// Same diagnostic for text_field, as a control. If text_field
/// shows the same "+1 extra holder after flip" then the leak
/// shape is identical and the issue is just timing/order; if
/// text_field clears to 1 cleanly, the extra holder is specific
/// to the text_view path.
#[cfg(any())]
fn diagnose_text_field_bind_extra_clones() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    use leptos_cocoa::cocoa::NodeRef;
    let _mtm = common::test_mtm();
    with_scope(|| {
        let flag = RwSignal::new(true);
        let value = RwSignal::new(String::new());
        let nref = NodeRef::new();
        let view = vstack().child(move || match flag.get() {
            true => Either::Left(
                text_field()
                    .node_ref(nref)
                    .bind(leptos_cocoa::attr::Value, value),
            ),
            false => Either::Right(label().text("off")),
        });
        let mut state = view.build();

        let el_before = nref.get().expect("text_field registered");
        let count_before = el_before.as_node().handlers_rc_count_for_test();
        let tf_alive_before = el_before
            .as_node()
            .handlers()
            .borrow()
            .text_field_delegate
            .is_some();
        eprintln!("TF before flip: count={count_before} delegate_in_bundle={tf_alive_before}");
        drop(el_before);

        flag.set(false);
        common::pump_run_loop(0.1);

        match nref.get() {
            Some(el) => {
                let c = el.as_node().handlers_rc_count_for_test();
                let tf_alive = el
                    .as_node()
                    .handlers()
                    .borrow()
                    .text_field_delegate
                    .is_some();
                eprintln!("TF after flip:  count={c} delegate_in_bundle={tf_alive}");
            }
            None => eprintln!("TF after flip: NodeRef cleared"),
        }
        state.unmount();
    });
}

/// text_view inside an Either branch + bind:value. Toggle off,
/// the previous branch's state must drop fully (including the
/// TextViewDelegate). This was the original P1 repro and was
/// known-failing until the `NSText.setDelegate:`
/// autorelease-pool fix in `ensure_text_view_entry`.
fn show_off_text_view_clears_delegate() {
    use either_of::Either;
    use leptos_cocoa::cocoa::element::{label, vstack};
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let flag = RwSignal::new(true);
        let value = RwSignal::new(String::new());
        let view = vstack().child(move || match flag.get() {
            true => Either::Left(
                text_view().bind(leptos_cocoa::attr::Value, value),
            ),
            false => Either::Right(label().text("off")),
        });
        let mut state = view.build();
        flag.set(false);
        common::pump_run_loop(0.1);
        let after_off = snapshot();
        assert_eq!(
            after_off.2, before.2,
            "toggling off must drop text_view delegate; \
             before={:?} after_off={:?}",
            before, after_off,
        );
        state.unmount();
    });
    let after = snapshot();
    assert_eq!(
        after, before,
        "show-off then unmount must match baseline; \
         before={:?} after={:?}",
        before, after,
    );
}

/// text_view with bind:value — drop without explicit unmount.
fn drop_without_unmount_clears_text_view_delegate() {
    let _mtm = common::test_mtm();
    let before = snapshot();
    with_scope(|| {
        let value = RwSignal::new(String::new());
        let view = text_view().bind(leptos_cocoa::attr::Value, value);
        let state = view.build();
        drop(state);
        common::pump_run_loop(0.1);
    });
    let after = snapshot();
    assert_eq!(
        after, before,
        "dropping text_view with bind:value must not leak delegate; \
         before={:?} after={:?}",
        before, after,
    );
}

fn main() {
    common::run_tests(&[
        (
            "drop_bare_text_view_clears_delegate",
            drop_bare_text_view_clears_delegate,
        ),
        (
            "explicit_unmount_clears_text_view_delegate",
            explicit_unmount_clears_text_view_delegate,
        ),
        (
            "drop_without_unmount_clears_text_view_delegate",
            drop_without_unmount_clears_text_view_delegate,
        ),
        (
            "drop_reactive_child_text_view_no_toggle",
            drop_reactive_child_text_view_no_toggle,
        ),
        (
            "show_off_text_view_clears_delegate",
            show_off_text_view_clears_delegate,
        ),
        (
            "show_off_text_field_clears_delegate",
            show_off_text_field_clears_delegate,
        ),
        (
            "show_off_bare_text_view_clears_delegate",
            show_off_bare_text_view_clears_delegate,
        ),
        (
            "show_off_text_view_with_oninput_clears_delegate",
            show_off_text_view_with_oninput_clears_delegate,
        ),
        (
            "full_teardown_text_view_persistent_leak",
            full_teardown_text_view_persistent_leak,
        ),
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
