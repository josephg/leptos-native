//! Exercises the CDP mapping against a hand-built layout tree, using a
//! trivial in-memory [`LayoutBackend`] (no platform views).

use leptos_native::renderer::{
    AvailableSpace, Dimension, LayoutBackend, LayoutState, LengthPercentage, Size, Style,
};
use std::cell::RefCell;

struct TestB;

thread_local! {
    static TREE: RefCell<LayoutState<TestB>> = RefCell::new(LayoutState::default());
}

impl LayoutBackend for TestB {
    type View = ();
    type NodeMeta = ();
    type Handlers = ();

    fn measure_leaf(
        _v: &(),
        _m: &(),
        known: Size<Option<f32>>,
        _avail: Size<AvailableSpace>,
    ) -> Size<f32> {
        Size {
            width: known.width.unwrap_or(0.0),
            height: known.height.unwrap_or(0.0),
        }
    }

    fn first_baseline(_v: &()) -> Option<f32> {
        None
    }

    fn with_tree<R>(f: impl FnOnce(&mut LayoutState<Self>) -> R) -> R {
        TREE.with(|t| f(&mut t.borrow_mut()))
    }
}

fn fixed(w: f32, h: f32) -> Style {
    let mut s = Style::default();
    s.size = Size {
        width: Dimension::length(w),
        height: Dimension::length(h),
    };
    s
}

#[test]
fn box_model_reflects_padding_and_position() {
    // Parent: 200x100, 10px padding all sides, column flex.
    let mut parent_style = fixed(200.0, 100.0);
    parent_style.padding = leptos_native::renderer::Rect {
        left: LengthPercentage::length(10.0),
        right: LengthPercentage::length(10.0),
        top: LengthPercentage::length(10.0),
        bottom: LengthPercentage::length(10.0),
    };
    let parent = TestB::new_leaf(parent_style, (), (), ());
    let child = TestB::new_leaf(fixed(50.0, 30.0), (), (), ());
    TestB::add_child(parent, child);

    TestB::run_layout_pass(
        parent,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(100.0),
        },
    );

    // Child border box should start at the parent's content origin (10,10).
    let model = leptos_devtools::box_model_json::<TestB>(child).expect("box model");
    let border = &model["border"];
    assert_eq!(border[0].as_f64().unwrap(), 10.0, "border x");
    assert_eq!(border[1].as_f64().unwrap(), 10.0, "border y");
    assert_eq!(model["width"].as_i64().unwrap(), 50);
    assert_eq!(model["height"].as_i64().unwrap(), 30);

    // Parent's content box is inset by its padding: origin (10,10), 180x80.
    let pm = leptos_devtools::box_model_json::<TestB>(parent).expect("parent box model");
    let content = &pm["content"];
    assert_eq!(content[0].as_f64().unwrap(), 10.0);
    assert_eq!(content[1].as_f64().unwrap(), 10.0);
    assert_eq!(content[2].as_f64().unwrap(), 190.0); // 10 + 180
}

#[test]
fn css_edit_round_trips_into_style() {
    let node = TestB::new_leaf(fixed(50.0, 30.0), (), (), ());

    // Sanity: the emitted declarations describe the starting style.
    let decls = leptos_devtools::css_decls(&TestB::style(node).unwrap());
    assert!(decls.contains(&("width".into(), "50px".into())));

    // Apply an edit the way CSS.setStyleTexts would.
    leptos_devtools::apply_css_text::<TestB>(node, "width: 80px; padding: 4px;", &|_| {});

    let after = leptos_devtools::css_decls(&TestB::style(node).unwrap());
    assert!(after.contains(&("width".into(), "80px".into())));
    assert!(after.contains(&("padding-left".into(), "4px".into())));
    assert!(after.contains(&("padding-top".into(), "4px".into())));
}
