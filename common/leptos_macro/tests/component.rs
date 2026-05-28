//! `#[component]` macro tests. Adapted from upstream
//! `leptos-upstream/leptos_macro/tests/component.rs`.
//!
//! These tests don't call `view!{}` (no port available at this layer);
//! they only verify the props builder + the wrapper-fn the macro
//! generates.

// `#[component]` emits absolute paths against the `leptos_platform`
// sentinel. In this test crate the renderer-agnostic `leptos_native`
// stands in for a real port.
extern crate leptos_native as leptos_platform;

use core::num::NonZeroUsize;
use leptos_macro::component;

#[derive(PartialEq, Debug)]
struct UserInfo {
    user_id: String,
    email: String,
}

#[derive(PartialEq, Debug)]
struct Admin(bool);

#[component]
fn Component(
    #[prop(optional)] optional: bool,
    #[prop(optional, into)] optional_into: Option<String>,
    #[prop(optional_no_strip)] optional_no_strip: Option<String>,
    #[prop(strip_option)] strip_option: Option<u8>,
    #[prop(default = NonZeroUsize::new(10).unwrap())] default: NonZeroUsize,
    #[prop(into)] into: String,
    impl_trait: impl Fn() -> i32 + 'static,
    #[prop(name = "data")] UserInfo { email, user_id }: UserInfo,
    #[prop(name = "tuple")] (name, id): (String, i32),
    #[prop(name = "tuple_struct")] Admin(is_admin): Admin,
    #[prop(name = "outside_name")] inside_name: i32,
) {
    _ = optional;
    _ = optional_into;
    _ = optional_no_strip;
    _ = strip_option;
    _ = default;
    _ = into;
    _ = impl_trait;
    _ = email;
    _ = user_id;
    _ = id;
    _ = name;
    _ = is_admin;
    _ = inside_name;
}

#[test]
fn component() {
    let cp = ComponentProps::builder()
        .into("")
        .strip_option(9)
        .impl_trait(|| 42)
        .data(UserInfo {
            email: "em@il".into(),
            user_id: "1".into(),
        })
        .tuple(("Joe".into(), 12))
        .tuple_struct(Admin(true))
        .outside_name(1)
        .build();
    assert!(!cp.optional);
    assert_eq!(cp.optional_into, None);
    assert_eq!(cp.optional_no_strip, None);
    assert_eq!(cp.strip_option, Some(9));
    assert_eq!(cp.default, NonZeroUsize::new(10).unwrap());
    assert_eq!(cp.into, "");
    assert_eq!((cp.impl_trait)(), 42);
    assert_eq!(
        cp.data,
        UserInfo {
            email: "em@il".into(),
            user_id: "1".into(),
        }
    );
    assert_eq!(cp.tuple, ("Joe".into(), 12));
    assert_eq!(cp.tuple_struct, Admin(true));
    assert_eq!(cp.outside_name, 1);
}

#[component]
fn WithLifetime<'a>(data: &'a str) {
    _ = data;
}

#[test]
fn lifetime_component_builds() {
    let val = String::from("hello");
    WithLifetime(WithLifetimeProps::builder().data(&val).build());
}

#[component(transparent)]
fn Transparent(value: i32) -> i32 {
    value * 2
}

#[test]
fn transparent_component_returns_body() {
    assert_eq!(Transparent(TransparentProps::builder().value(7).build()), 14);
}
