//! Block-layout demo — a long-form article in `<block>` flow.
//!
//! `<block>` (gated behind the `block_layout` Cargo feature) is the
//! Taffy block-layout primitive. Children stack vertically and fill
//! container width, so labels word-wrap to the available width
//! without per-child sizing.
//!
//! The article body is wrapped in a `<scroll_view>` so the window
//! can be shorter than the content. Each figure is centred via an
//! `<hstack justify_content=Center>` and uses a `<vstack>` body
//! with a percentage `max_width` and a px `min_width`.
//!
//! ## On the figure's resize behaviour
//!
//! The figure uses `max_width=Dim::pct(0.85)` and
//! `min_width=Dim::px(280)`. As you narrow the window:
//!
//! * **Wide window** — the figure body is `0.85 × content_width`,
//!   centred, with breathing room on each side.
//! * **Narrowing** — the figure shrinks proportionally. There's no
//!   "fills edge-to-edge" phase because `pct` tracks the parent.
//! * **Window narrower than `min_width / 0.85`** — figure pegs at
//!   its `min_width` (280px) and overflows the centring container.
//!   This matches CSS behaviour: an element with `min-width: X`
//!   stays at least X wide and overflows its parent if the parent
//!   is narrower.

use cocoa_dom::{layout::JustifyContent, Color, NSTextAlignment};
use leptos::prelude::*;
use leptos::tachys::cocoa::attr::Dim;

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

/// A figure: a coloured rectangle ("image") with a centred caption
/// underneath. The figure body is centred horizontally inside the
/// article column, with a percentage-based `max_width` so it
/// always has visible side margins on a wide window, and a px
/// `min_width` so it stops shrinking past a readable size.
#[component]
fn Figure(
    #[prop(into)] caption: String,
    height: f32,
) -> impl IntoView {
    view! {
        <hstack justify_content=JustifyContent::Center>
            <vstack
                gap=10.0
                max_width=Dim::pct(0.85)
                min_width=Dim::px(280.0)
            >
                <stack background_color=Color::RED height=height />
                <label alignment=NSTextAlignment::Center>{caption}</label>
            </vstack>
        </hstack>
    }
}

#[component]
fn Article() -> impl IntoView {
    // The block holds a single `<vstack gap=...>` to space its
    // children apart. Block layout itself doesn't honour `gap`
    // (Taffy's block algorithm ignores it), so we use a flex
    // container to provide visible breathing room between
    // headings, paragraphs, and figures.
    view! {
        <scroll_view flex_grow=1.0>
            <block padding=32.0>
                <vstack gap=18.0>
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
                </vstack>
            </block>
        </scroll_view>
    }
}

fn main() {
    mount_to_window("Block layout demo", (640.0, 720.0), || {
        view! { <Article /> }
    });
}
