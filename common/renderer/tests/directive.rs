//! Tests for the generic `IntoDirective` trait + `pack` / `run_all`
//! helpers. Uses a fake `Element` type to avoid any platform deps.

use renderer::directive::{pack, run_all, IntoDirective};
use std::sync::{Arc, Mutex};

/// Stand-in for `cocoa_dom::Element` / `ios_dom::Element`. Cheap to
/// clone, gives observable identity for assertions.
#[derive(Copy, Clone, Debug, PartialEq)]
struct FakeElement(usize);

#[test]
fn zero_param_directive_runs_with_element() {
    let captured = Arc::new(Mutex::new(None::<FakeElement>));
    let captured_clone = captured.clone();

    let handler = move |el: FakeElement| {
        *captured_clone.lock().unwrap() = Some(el);
    };

    handler.run(FakeElement(7), ());
    assert_eq!(*captured.lock().unwrap(), Some(FakeElement(7)));
}

#[test]
fn one_param_directive_runs_with_element_and_param() {
    let captured = Arc::new(Mutex::new(None::<(FakeElement, i32)>));
    let captured_clone = captured.clone();

    let handler = move |el: FakeElement, p: i32| {
        *captured_clone.lock().unwrap() = Some((el, p));
    };

    handler.run(FakeElement(3), 42);
    assert_eq!(*captured.lock().unwrap(), Some((FakeElement(3), 42)));
}

#[test]
fn pack_boxes_zero_param_directive_into_fnonce_ref_element() {
    let captured = Arc::new(Mutex::new(None::<FakeElement>));
    let captured_clone = captured.clone();

    let handler = move |el: FakeElement| {
        *captured_clone.lock().unwrap() = Some(el);
    };
    let boxed = pack::<FakeElement, _, _, _>(handler, ());

    let el = FakeElement(11);
    boxed(el);
    assert_eq!(*captured.lock().unwrap(), Some(FakeElement(11)));
}

#[test]
fn pack_boxes_one_param_directive() {
    let captured = Arc::new(Mutex::new(None::<(FakeElement, String)>));
    let captured_clone = captured.clone();

    let handler = move |el: FakeElement, s: String| {
        *captured_clone.lock().unwrap() = Some((el, s));
    };
    let boxed = pack::<FakeElement, _, _, _>(handler, "hi".to_string());

    let el = FakeElement(5);
    boxed(el);
    assert_eq!(
        *captured.lock().unwrap(),
        Some((FakeElement(5), "hi".to_string()))
    );
}

#[test]
fn run_all_drains_in_order() {
    let order = Arc::new(Mutex::new(Vec::<usize>::new()));
    let mk = |n: usize| {
        let order = order.clone();
        let handler = move |_el: FakeElement| {
            order.lock().unwrap().push(n);
        };
        pack::<FakeElement, _, _, _>(handler, ())
    };

    let directives = vec![mk(1), mk(2), mk(3)];
    run_all(directives, FakeElement(0));

    assert_eq!(*order.lock().unwrap(), vec![1, 2, 3]);
}

#[test]
fn run_all_with_empty_vec_is_noop() {
    let directives: Vec<Box<dyn FnOnce(&FakeElement) + Send + 'static>> =
        Vec::new();
    run_all(directives, &FakeElement(0));
    // The point is just: no panic, no UB.
}

#[test]
fn pack_handler_consumes_param_by_move() {
    // Param can be a non-Clone owned value; pack must move it into
    // the closure rather than try to copy it.
    struct NotClone(i32);
    let captured = Arc::new(Mutex::new(None::<i32>));
    let captured_clone = captured.clone();

    let handler = move |_el: FakeElement, p: NotClone| {
        *captured_clone.lock().unwrap() = Some(p.0);
    };
    let boxed = pack::<FakeElement, _, _, _>(handler, NotClone(99));
    boxed(FakeElement(0));
    assert_eq!(*captured.lock().unwrap(), Some(99));
}
