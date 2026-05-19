//! Tests for `Text` and `Placeholder` constructors + setters.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{NodeKind, Placeholder, Text};
use objc2::runtime::AnyObject;
use objc2_app_kit::NSTextField;

fn text_create_basic() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let t = Text::create(&tree, "hello");
    assert_eq!(t.as_node().kind(), NodeKind::Text);

    // Backed by an NSTextField with the given content.
    let any: &AnyObject = t.as_node().ns_view().as_ref();
    let field = any
        .downcast_ref::<NSTextField>()
        .expect("Text should be backed by NSTextField");
    assert_eq!(field.stringValue().to_string(), "hello");
    // Label-style: not editable, not bordered.
    assert!(!field.isEditable());
}

fn text_create_empty() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let t = Text::create(&tree, "");
    let any: &AnyObject = t.as_node().ns_view().as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    assert_eq!(field.stringValue().to_string(), "");
}

fn text_create_multiline_preserves_newlines() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let t = Text::create(&tree, "line one\nline two\nline three");
    let any: &AnyObject = t.as_node().ns_view().as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    assert_eq!(
        field.stringValue().to_string(),
        "line one\nline two\nline three"
    );
}

fn text_set_text_updates_value() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let t = Text::create(&tree, "before");
    t.set_text("after");
    let any: &AnyObject = t.as_node().ns_view().as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    assert_eq!(field.stringValue().to_string(), "after");
}

fn placeholder_create_is_hidden_zero_size() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let p = Placeholder::create(&tree);
    assert_eq!(p.as_node().kind(), NodeKind::Placeholder);

    let view = p.as_node().ns_view();
    // Placeholders shouldn't be visible — they shouldn't intercept
    // hit-testing or take layout space. Implementation: hidden +
    // zero frame.
    assert!(view.isHidden());
    let frame = view.frame();
    assert_eq!(frame.size.width, 0.0);
    assert_eq!(frame.size.height, 0.0);
}

fn main() {
    common::run_tests(&[
        ("text_create_basic", text_create_basic),
        ("text_create_empty", text_create_empty),
        (
            "text_create_multiline_preserves_newlines",
            text_create_multiline_preserves_newlines,
        ),
        ("text_set_text_updates_value", text_set_text_updates_value),
        (
            "placeholder_create_is_hidden_zero_size",
            placeholder_create_is_hidden_zero_size,
        ),
    ]);
}
