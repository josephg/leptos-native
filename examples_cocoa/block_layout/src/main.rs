//! Block-layout demo — a long-form article in `<block>` flow.
//!
//! `<block>` is a Taffy block-layout primitive (gated behind the
//! `block_layout` Cargo feature). Children stack vertically and
//! fill container width, so labels word-wrap to the available
//! width without per-child sizing.
//!
//! The whole article is wrapped in a `<scroll_view>` so the
//! window can be shorter than the content and the user scrolls.
//! Figures are a small `<vstack>` with a coloured rectangle on top
//! and a caption label underneath — the rectangle fills the
//! block's width and naturally narrows as the window narrows.

use cocoa_dom::{layout::JustifyContent, Color, NSTextAlignment};
use leptos::prelude::*;

#[component]
fn Heading(#[prop(into)] text: String) -> impl IntoView {
    view! { <label font_size=22.0>{text}</label> }
}

#[component]
fn Subheading(#[prop(into)] text: String) -> impl IntoView {
    view! { <label font_size=16.0>{text}</label> }
}

#[component]
fn Paragraph(#[prop(into)] text: String) -> impl IntoView {
    view! { <label>{text}</label> }
}

/// A figure: a coloured rectangle ("image") with a caption
/// underneath, both centred horizontally inside the article. The
/// figure body has a `max_width` so it doesn't stretch all the way
/// to the block edges on a wide window, and a `min_width` so it
/// stops shrinking when the window gets narrow.
#[component]
fn Figure(
    #[prop(into)] caption: String,
    height: f32,
) -> impl IntoView {
    view! {
        <hstack justify_content=JustifyContent::Center>
            <vstack gap=8.0 max_width=520.0 min_width=300.0>
                <stack background_color=Color::RED height=height />
                <label alignment=NSTextAlignment::Center>{caption}</label>
            </vstack>
        </hstack>
    }
}

#[component]
fn Article() -> impl IntoView {
    view! {
        <scroll_view grow=1.0>
            <block padding=24.0>
                <Heading text="On the Inevitable Drift of Layout Engines"/>
                <Subheading text="A field report from the front of native UI"/>

                <Paragraph text="Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat."/>

                <Paragraph text="Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum."/>

                <Figure
                    caption="Figure 1 — A representative empty rectangle."
                    height=180.0
                />

                <Subheading text="The case for stack and block"/>

                <Paragraph text="Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo. Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit, sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt."/>

                <Paragraph text="Neque porro quisquam est, qui dolorem ipsum quia dolor sit amet, consectetur, adipisci velit, sed quia non numquam eius modi tempora incidunt ut labore et dolore magnam aliquam quaerat voluptatem."/>

                <Paragraph text="Ut enim ad minima veniam, quis nostrum exercitationem ullam corporis suscipit laboriosam, nisi ut aliquid ex ea commodi consequatur? Quis autem vel eum iure reprehenderit qui in ea voluptate velit esse quam nihil molestiae consequatur, vel illum qui dolorem eum fugiat quo voluptas nulla pariatur?"/>

                <Figure
                    caption="Figure 2 — Another representative empty rectangle. This caption is intentionally a little longer so that you can see how the text wraps when the window is narrowed."
                    height=220.0
                />

                <Subheading text="Implementation notes"/>

                <Paragraph text="At vero eos et accusamus et iusto odio dignissimos ducimus qui blanditiis praesentium voluptatum deleniti atque corrupti quos dolores et quas molestias excepturi sint occaecati cupiditate non provident, similique sunt in culpa qui officia deserunt mollitia animi, id est laborum et dolorum fuga."/>

                <Paragraph text="Et harum quidem rerum facilis est et expedita distinctio. Nam libero tempore, cum soluta nobis est eligendi optio cumque nihil impedit quo minus id quod maxime placeat facere possimus, omnis voluptas assumenda est, omnis dolor repellendus."/>

                <Paragraph text="Temporibus autem quibusdam et aut officiis debitis aut rerum necessitatibus saepe eveniet ut et voluptates repudiandae sint et molestiae non recusandae. Itaque earum rerum hic tenetur a sapiente delectus, ut aut reiciendis voluptatibus maiores alias consequatur aut perferendis doloribus asperiores repellat."/>

                <Figure
                    caption="Figure 3 — The third and final empty rectangle."
                    height=140.0
                />

                <Subheading text="Conclusion"/>

                <Paragraph text="On the whole, the experience of building applications around a typed Taffy-first layout language is much closer to writing platform-native UI than it is to writing a browser. The compiler rejects nonsense combinations, the runtime layout cost is bounded, and the resulting view tree reads exactly the way the designer drew it on paper."/>

                <Paragraph text="Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida."/>
            </block>
        </scroll_view>
    }
}

fn main() {
    mount_to_window("Block layout demo", (640.0, 720.0), || {
        view! { <Article /> }
    });
}
