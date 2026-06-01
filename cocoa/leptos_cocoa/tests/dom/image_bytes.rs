//! Tests for `CocoaNode::set_image_view_bytes` — the reactive
//! `<image_view bytes=…>` setter backed by `NSImage::initWithData:`.
//!
//! Covers: None / empty / garbage / valid PNG. NSImage's data
//! initializer is documented to return nil on undecodable input
//! rather than panicking; we lean on that contract and verify the
//! view ends up imageless rather than panicking the test.

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::dom::{CocoaElem, CocoaMakeView, CocoaNodeExt};
use objc2::runtime::AnyObject;
use objc2::Message;
use objc2_app_kit::NSImageView;

/// A 1×1 transparent PNG. Smallest valid PNG we can include
/// inline. NSImage can definitely decode this.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG sig
    0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR len + type
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1×1
    0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, // 8-bit RGBA
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, // IDAT len + type
    0x54, 0x78, 0x9c, 0x62, 0x00, 0x01, 0x00, 0x00, // zlib data
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00,
    0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, // IEND
    0x60, 0x82,
];

fn iv(el: &CocoaElem) -> objc2::rc::Retained<NSImageView> {
    let view = el.ns_view();
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<NSImageView>()
        .expect("`image_view` tag should produce an NSImageView")
        .retain()
}

// ---------------------------------------------------------------------
// Smoke: the tag actually creates an NSImageView (defensive against
// future tag-routing regressions; the rest of the file relies on it).
// ---------------------------------------------------------------------

fn image_view_tag_creates_nsimageview() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_image_view().0;
    let _iv = iv(&el);
}

// ---------------------------------------------------------------------
// Valid bytes → an image lands on the view.
// ---------------------------------------------------------------------

fn valid_png_bytes_set_image() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_image_view().0;
    assert!(
        iv(&el).image().is_none(),
        "fresh image_view should have no image"
    );

    el.set_image_view_bytes(Some(TINY_PNG));

    assert!(
        iv(&el).image().is_some(),
        "valid PNG bytes should populate NSImageView.image"
    );
}

// ---------------------------------------------------------------------
// None → image cleared.
// ---------------------------------------------------------------------

fn none_clears_image() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_image_view().0;
    el.set_image_view_bytes(Some(TINY_PNG));
    assert!(iv(&el).image().is_some());

    el.set_image_view_bytes(None);

    assert!(
        iv(&el).image().is_none(),
        "set_image_view_bytes(None) should clear NSImageView.image"
    );
}

// ---------------------------------------------------------------------
// Empty slice → image cleared (matches set_image_view_path("")
// no-image semantics).
// ---------------------------------------------------------------------

fn empty_slice_clears_image() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_image_view().0;
    el.set_image_view_bytes(Some(TINY_PNG));
    assert!(iv(&el).image().is_some());

    el.set_image_view_bytes(Some(&[]));

    assert!(
        iv(&el).image().is_none(),
        "set_image_view_bytes(Some(&[])) should clear NSImageView.image"
    );
}

// ---------------------------------------------------------------------
// Garbage bytes → image cleared, no panic.
//
// NSImage::initWithData: returns nil on undecodable data. Our setter
// forwards that nil to `setImage(None)`. The behaviour we care about:
// no panic, no abort, view ends imageless.
// ---------------------------------------------------------------------

fn garbage_bytes_dont_panic() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_image_view().0;
    let junk = b"not an image at all, just some bytes";

    el.set_image_view_bytes(Some(junk));

    assert!(
        iv(&el).image().is_none(),
        "undecodable bytes should leave NSImageView imageless"
    );
}

// ---------------------------------------------------------------------
// Replace: valid → different valid keeps an image set (different
// content, but the *presence* of an image is what we can cheaply
// assert without decoding pixels).
// ---------------------------------------------------------------------

fn replace_with_new_valid_bytes() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_image_view().0;
    el.set_image_view_bytes(Some(TINY_PNG));
    let first = iv(&el).image().expect("first image set");
    let first_ptr: *const objc2_app_kit::NSImage = &*first;

    el.set_image_view_bytes(Some(TINY_PNG));
    let second = iv(&el).image().expect("second image set");
    let second_ptr: *const objc2_app_kit::NSImage = &*second;

    // Pointer comparison is a bit indirect, but it's the cheapest
    // way to confirm the setter actually replaced the inner image
    // (initWithData: allocates a fresh NSImage each time).
    assert_ne!(
        first_ptr, second_ptr,
        "second set should produce a fresh NSImage object"
    );
}

fn main() {
    common::run_tests(&[
        (
            "image_view_tag_creates_nsimageview",
            image_view_tag_creates_nsimageview,
        ),
        ("valid_png_bytes_set_image", valid_png_bytes_set_image),
        ("none_clears_image", none_clears_image),
        ("empty_slice_clears_image", empty_slice_clears_image),
        ("garbage_bytes_dont_panic", garbage_bytes_dont_panic),
        ("replace_with_new_valid_bytes", replace_with_new_valid_bytes),
    ]);
}
