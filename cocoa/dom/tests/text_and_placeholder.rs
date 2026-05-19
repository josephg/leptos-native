//! Tests for the text-label (`Element::create_text`) and placeholder
//! (`Element::create_placeholder`) constructors on `Element`.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::Element;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSTextField;

fn text_create_basic() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let t = Element::create_text(&tree, "hello");

    // Backed by an NSTextField with the given content.
    let any: &AnyObject = t.as_node().ns_view().as_ref();
    let field = any
        .downcast_ref::<NSTextField>()
        .expect("text-label should be backed by NSTextField");
    assert_eq!(field.stringValue().to_string(), "hello");
    // Label-style: not editable, not bordered.
    assert!(!field.isEditable());
}

fn text_create_empty() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let t = Element::create_text(&tree, "");
    let any: &AnyObject = t.as_node().ns_view().as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    assert_eq!(field.stringValue().to_string(), "");
}

fn text_create_multiline_preserves_newlines() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let t = Element::create_text(&tree, "line one\nline two\nline three");
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
    let t = Element::create_text(&tree, "before");
    t.set_text("after");
    let any: &AnyObject = t.as_node().ns_view().as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    assert_eq!(field.stringValue().to_string(), "after");
}

fn placeholder_create_is_hidden_zero_size() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let p = Element::create_placeholder(&tree);

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
