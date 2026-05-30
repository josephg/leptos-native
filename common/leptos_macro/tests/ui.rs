//! `trybuild` compile-fail tests for view!{} syntaxes we deliberately
//! reject (HTML/CSS-shaped attributes that have no native analog).

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/doctype.rs");
    t.compile_fail("tests/ui/class_attr.rs");
    t.compile_fail("tests/ui/style_attr.rs");
    t.compile_fail("tests/ui/prop_attr.rs");
    t.compile_fail("tests/ui/template_macro.rs");
}
