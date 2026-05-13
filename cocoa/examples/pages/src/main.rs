//! Apple Pages document-editor UI — a non-functional mockup that
//! mirrors the toolbar / canvas / inspector-sidebar shape of the
//! macOS Pages app. Toggles between the **Document** and **Format**
//! inspector via the toolbar's Format/Document segmented buttons on
//! the right.
//!
//! All data is hardcoded; clicks toggle inspector modes but don't
//! edit anything. Stress-tests the cocoa port's per-edge padding,
//! `<Switch>` control flow, label truncation, and dark-on-light
//! theming.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;
    use cocoa_dom::{Color, LineBreak, TextAlignment};

    // ---- palette ----------------------------------------------------

    // Toolbar / sidebar are dark; canvas is white.
    const TOOLBAR_BG:   Color = Color { r: 0.165, g: 0.165, b: 0.165, a: 1.0 };
    const SIDEBAR_BG:   Color = Color { r: 0.176, g: 0.176, b: 0.176, a: 1.0 };
    const CANVAS_BG:    Color = Color::WHITE;
    const APP_BG:       Color = Color { r: 0.082, g: 0.082, b: 0.082, a: 1.0 };

    const TXT_PRIMARY:   Color = Color { r: 0.95,  g: 0.95,  b: 0.95,  a: 1.0 };
    const TXT_SECONDARY: Color = Color { r: 0.62,  g: 0.62,  b: 0.62,  a: 1.0 };
    const TXT_DARK:      Color = Color { r: 0.05,  g: 0.05,  b: 0.05,  a: 1.0 };
    const TXT_BODY:      Color = Color { r: 0.20,  g: 0.20,  b: 0.20,  a: 1.0 };

    const ACCENT_ORANGE: Color = Color { r: 1.00,  g: 0.58,  b: 0.0,   a: 1.0 };
    const ACCENT_BLUE:   Color = Color { r: 0.31,  g: 0.58,  b: 1.00,  a: 1.0 };
    const FIELD_BG:      Color = Color { r: 0.231, g: 0.231, b: 0.231, a: 1.0 };
    const FIELD_BORDER:  Color = Color { r: 0.30,  g: 0.30,  b: 0.30,  a: 1.0 };
    const HAIRLINE:      Color = Color { r: 0.30,  g: 0.30,  b: 0.30,  a: 1.0 };

    // Image placeholder color (the bedroom photo stand-in).
    const PHOTO_BG: Color = Color { r: 0.45, g: 0.43, b: 0.40, a: 1.0 };

    // Orange highlight tint (the "T" drop cap).
    const DROPCAP_BG: Color = Color { r: 0.95, g: 0.79, b: 0.65, a: 1.0 };

    // ---- model ------------------------------------------------------

    /// Which inspector pane is open in the sidebar. `Hidden` means
    /// the sidebar is collapsed (animated out via the
    /// NSSplitViewItem inspector behavior).
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    enum Inspector { Hidden, Document, Format }

    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    enum DocTab { Document, Section, Bookmarks }

    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    enum FmtTab { Style, Layout, More }

    // ---- helpers ----------------------------------------------------

    /// Square clickable icon-button used across the toolbar — small
    /// icon glyph above a tiny label. The label and glyph share
    /// styling so this captures the visual repetition once.
    fn tool_button(
        glyph: &'static str,
        label: &'static str,
        on_click: impl FnMut() + Send + 'static,
    ) -> impl IntoView {
        let mut cb = on_click;
        view! {
            <button
                bordered=false
                background_color=Color::rgba(1.0, 1.0, 1.0, 0.0)
                padding=Edges::axis(8.0, 4.0)
                on:click=move |_| cb()
                text_color=TXT_PRIMARY
                font_size=11.0
            >
                {format!("{}\n{}", glyph, label)}
            </button>
        }
    }

    /// A "pill" toggle for the toolbar's Format/Document segmented
    /// pair — active gets an orange icon tint.
    fn inspector_pill(
        glyph: &'static str,
        label: &'static str,
        active: impl Fn() -> bool + Send + Sync + 'static + Copy,
        on_click: impl FnMut() + Send + 'static,
    ) -> impl IntoView {
        let mut cb = on_click;
        view! {
            <button
                bordered=false
                corner_radius=4.0
                padding=Edges::axis(10.0, 4.0)
                background_color=move || if active() {
                    Color::rgba(1.0, 1.0, 1.0, 0.12)
                } else {
                    Color::rgba(1.0, 1.0, 1.0, 0.0)
                }
                text_color=move || if active() { ACCENT_ORANGE } else { TXT_PRIMARY }
                font_size=11.0
                on:click=move |_| cb()
            >
                {format!("{}\n{}", glyph, label)}
            </button>
        }
    }

    /// 3-segment selector — the row of pills at the top of each
    /// inspector pane (Document/Section/Bookmarks, Style/Layout/More).
    fn segmented<T, F>(items: Vec<(T, &'static str)>, current: F, set: impl Fn(T) + Send + Sync + 'static + Copy)
        -> impl IntoView
    where
        T: PartialEq + Copy + Send + Sync + 'static,
        F: Fn() -> T + Send + Sync + 'static + Copy,
    {
        view! {
            <hstack
                gap=0.0
                background_color=FIELD_BG
                corner_radius=6.0
                padding=2.0
                clip=true
            >
                {items.into_iter().map(|(value, label)| view! {
                    <button
                        bordered=false
                        flex_grow=1.0
                        padding=Edges::axis(0.0, 4.0)
                        corner_radius=4.0
                        background_color=move || if current() == value {
                            ACCENT_ORANGE
                        } else {
                            Color::rgba(1.0, 1.0, 1.0, 0.0)
                        }
                        text_color=move || if current() == value { TXT_DARK } else { TXT_PRIMARY }
                        font_size=12.0
                        bold=move || current() == value
                        on:click=move |_| set(value)
                    >{label}</button>
                }).collect::<Vec<_>>()}
            </hstack>
        }
    }

    /// Label + dropdown row — `<label>Caption</label> <dropdown ▾>`.
    /// Caption only — actual native popup state lives elsewhere.
    fn dropdown_row(value: &'static str, accent: bool) -> impl IntoView {
        view! {
            <hstack
                gap=0.0
                background_color=FIELD_BG
                corner_radius=4.0
                padding=Edges::axis(8.0, 5.0)
                align=AlignItems::Center
                border_width=0.5
                border_color=FIELD_BORDER
            >
                <label
                    flex_grow=1.0
                    font_size=12.0
                    text_color=if accent { ACCENT_ORANGE } else { TXT_PRIMARY }
                    bold=accent
                    line_break=LineBreak::TRUNCATE_TAIL
                >{value}</label>
                <label
                    font_size=10.0
                    text_color=TXT_SECONDARY
                    padding=Edges::ZERO.left(8.0)
                >"▾"</label>
            </hstack>
        }
    }

    /// "section header" — a small caption row above a group of fields.
    fn section_header(text: &'static str) -> impl IntoView {
        view! {
            <label
                font_size=11.0
                bold=true
                text_color=TXT_PRIMARY
                padding=Edges::ZERO.top(8.0).bottom(4.0)
            >{text}</label>
        }
    }

    /// Numeric field (with optional unit suffix) used in margin /
    /// header inputs. Not editable — visual stand-in for a stepper.
    fn measure_field(value: &'static str) -> impl IntoView {
        view! {
            <hstack
                background_color=FIELD_BG
                corner_radius=4.0
                padding=Edges::axis(6.0, 4.0)
                border_width=0.5
                border_color=FIELD_BORDER
                gap=4.0
                align=AlignItems::Center
            >
                <label
                    flex_grow=1.0
                    font_size=11.0
                    text_color=TXT_PRIMARY
                >{value}</label>
                <vstack gap=0.0>
                    <label font_size=8.0 text_color=TXT_SECONDARY>"▴"</label>
                    <label font_size=8.0 text_color=TXT_SECONDARY>"▾"</label>
                </vstack>
            </hstack>
        }
    }

    /// Square page-orientation thumbnail. Looks like a tiny sheet of
    /// paper; clicking would set portrait/landscape.
    fn orientation_thumb(
        landscape: bool,
        selected: impl Fn() -> bool + Send + Sync + 'static + Copy,
        on_click: impl FnMut() + Send + 'static,
    ) -> impl IntoView {
        let mut cb = on_click;
        let (w, h) = if landscape { (72.0_f32, 50.0_f32) } else { (50.0_f32, 72.0_f32) };
        view! {
            <vstack
                width=w
                height=h
                background_color=Color::WHITE
                corner_radius=2.0
                border_width=move || if selected() { 2.0 } else { 0.5 }
                border_color=move || if selected() { ACCENT_BLUE } else { HAIRLINE }
                align=AlignItems::FlexEnd
                justify_content=JustifyContent::FlexEnd
                padding=4.0
            >
                <label
                    font_size=14.0
                    text_color=move || if selected() { ACCENT_BLUE } else { Color::rgba(0.0,0.0,0.0,0.0) }
                    bold=true
                    on:click=move |_| cb()
                >"✓"</label>
            </vstack>
        }
    }

    /// Checkbox row — orange-tinted checkbox + label, common pattern
    /// across the inspector sidebar.
    fn check_row(label: &'static str, checked: impl Fn() -> bool + Send + Sync + 'static + Copy, set: impl Fn() + Send + Sync + 'static + Copy)
        -> impl IntoView
    {
        view! {
            <hstack gap=8.0 align=AlignItems::Center>
                <button
                    bordered=false
                    size=14.0
                    corner_radius=2.0
                    background_color=move || if checked() { ACCENT_ORANGE } else { FIELD_BG }
                    text_color=move || if checked() { TXT_DARK } else { Color::rgba(0.0,0.0,0.0,0.0) }
                    font_size=10.0
                    bold=true
                    border_width=0.5
                    border_color=FIELD_BORDER
                    on:click=move |_| set()
                >"✓"</button>
                <label font_size=12.0 text_color=TXT_PRIMARY>{label}</label>
            </hstack>
        }
    }

    // ---- root -------------------------------------------------------

    /// Main pane content: toolbar at top, canvas below. Lives
    /// inside an NSSplitViewItem; layout inside the pane is Taffy
    /// as everywhere else.
    #[component]
    fn MainPane() -> impl IntoView {
        view! {
            <vstack flex_grow=1.0 background_color=APP_BG>
                <Toolbar />
                <Canvas />
            </vstack>
        }
    }

    /// Inspector pane content. Picks between Document and Format
    /// inspectors. The pane itself (flyout, animation, vibrancy) is
    /// owned by NSSplitView; this is just the body that lives
    /// inside.
    #[component]
    fn InspectorPane() -> impl IntoView {
        let inspector = use_context::<RwSignal<Inspector>>()
            .expect("inspector ctx");
        view! {
            <vstack
                flex_grow=1.0
                background_color=SIDEBAR_BG
            >
                <Switch>
                    <Match when=move || inspector.get() == Inspector::Document>
                        <DocumentInspector />
                    </Match>
                    <Match when=move || inspector.get() == Inspector::Format>
                        <FormatInspector />
                    </Match>
                </Switch>
            </vstack>
        }
    }

    // ---- toolbar ----------------------------------------------------

    #[component]
    fn Toolbar() -> impl IntoView {
        let inspector = use_context::<RwSignal<Inspector>>().expect("inspector");
        view! {
            <hstack
                height=64.0
                background_color=TOOLBAR_BG
                padding=Edges::axis(12.0, 6.0)
                gap=6.0
                align=AlignItems::Center
            >
                // Left group
                <hstack gap=2.0 align=AlignItems::Center>
                    {tool_button("◫", "View", || {})}
                    {tool_button("125%", "Zoom", || {})}
                    {tool_button("⊕", "Add Page", || {})}
                </hstack>
                <Divider />
                // Insert section
                <hstack gap=2.0 align=AlignItems::Center>
                    {tool_button("⌶", "Insert", || {})}
                    {tool_button("⊞", "Table", || {})}
                    {tool_button("▦", "Graph", || {})}
                    {tool_button("T", "Text", || {})}
                    {tool_button("◆", "Shape", || {})}
                    {tool_button("◳", "Media", || {})}
                    {tool_button("💬", "Comment", || {})}
                </hstack>
                <vstack flex_grow=1.0 />
                // Right group
                {tool_button("↑", "Share", || {})}
                <Divider />
                // Clicking the active pill collapses the sidebar
                // (NSSplitViewController animates it). Clicking
                // either pill switches modes and ensures the
                // sidebar is open.
                <hstack gap=2.0 align=AlignItems::Center>
                    {inspector_pill("✎", "Format",
                        move || inspector.get() == Inspector::Format,
                        move || inspector.update(|i| *i = match *i {
                            Inspector::Format => Inspector::Hidden,
                            _ => Inspector::Format,
                        }))}
                    {inspector_pill("☰", "Document",
                        move || inspector.get() == Inspector::Document,
                        move || inspector.update(|i| *i = match *i {
                            Inspector::Document => Inspector::Hidden,
                            _ => Inspector::Document,
                        }))}
                </hstack>
            </hstack>
        }
    }

    #[component]
    fn Divider() -> impl IntoView {
        view! {
            <vstack
                width=1.0
                height=32.0
                background_color=HAIRLINE
                margin=Edges::axis(6.0, 0.0)
            />
        }
    }

    // ---- canvas -----------------------------------------------------

    #[component]
    fn Canvas() -> impl IntoView {
        view! {
            <scroll_view flex_grow=1.0 autohides_scrollers=true>
                <vstack
                    padding=Edges::axis(60.0, 40.0)
                    align=AlignItems::Center
                >
                    <vstack
                        width=540.0
                        background_color=CANVAS_BG
                        padding=Edges::trbl(48.0, 60.0, 60.0, 60.0)
                        gap=18.0
                    >
                        <label
                            font_size=11.0
                            text_color=TXT_BODY
                        >"Simple Home Styling"</label>

                        <vstack gap=4.0>
                            <label
                                font_size=20.0
                                text_color=TXT_BODY
                            >"Simple Home Styling"</label>
                            <label
                                font_size=44.0
                                bold=true
                                text_color=TXT_DARK
                            >"Easy Decorating"</label>
                        </vstack>

                        // Body paragraph + drop cap. The drop cap is a
                        // separate orange-tinted "T" pinned to the left
                        // of a multiline body label.
                        <hstack gap=8.0 align=AlignItems::FlexStart>
                            <vstack
                                size=48.0
                                background_color=DROPCAP_BG
                                align=AlignItems::Center
                                justify_content=JustifyContent::Center
                            >
                                <label
                                    font_size=42.0
                                    bold=true
                                    text_color=TXT_DARK
                                >"T"</label>
                            </vstack>
                            <label
                                flex_grow=1.0
                                font_size=14.0
                                text_color=TXT_BODY
                                multiline=true
                            >
                                "o get started, just tap or click this placeholder text and begin typing. You can view and edit this document on your Mac, iPad, iPhone, and on iCloud.com."
                            </label>
                        </hstack>

                        <label
                            font_size=14.0
                            text_color=TXT_BODY
                            multiline=true
                        >
                            "It's easy to edit text, change fonts and add beautiful graphics. Use paragraph styles to get a consistent look throughout your document. For example, this paragraph uses Body style. You can change it in the Text tab of the Format controls."
                        </label>

                        // Image placeholder
                        <vstack
                            height=240.0
                            background_color=PHOTO_BG
                            corner_radius=2.0
                            align=AlignItems::Center
                            justify_content=JustifyContent::Center
                        >
                            <label
                                font_size=14.0
                                text_color=Color::WHITE
                                bold=true
                            >"🛋"</label>
                        </vstack>

                        <vstack gap=2.0>
                            <label
                                font_size=11.0
                                text_color=TXT_SECONDARY
                                multiline=true
                            >
                                "Drag your own photo to the image placeholder above, then crop or resize it if you wish."
                            </label>
                            <label
                                font_size=11.0
                                text_color=TXT_SECONDARY
                                multiline=true
                            >
                                "Tap or click this placeholder and start typing to replace the caption, or turn it off in the Format controls."
                            </label>
                        </vstack>
                    </vstack>
                </vstack>
            </scroll_view>
        }
    }

    // ---- document inspector -----------------------------------------

    #[component]
    fn DocumentInspector() -> impl IntoView {
        let tab = RwSignal::new(DocTab::Document);
        let portrait = RwSignal::new(true);
        let v_text = RwSignal::new(false);
        let body = RwSignal::new(true);
        let header = RwSignal::new(true);
        let footer = RwSignal::new(false);
        let facing = RwSignal::new(false);
        let hyphenation = RwSignal::new(false);
        let ligatures = RwSignal::new(true);

        view! {
            <scroll_view flex_grow=1.0 autohides_scrollers=true>
                <vstack padding=Edges::all(16.0) gap=12.0>
                    {segmented(
                        vec![
                            (DocTab::Document,  "Document"),
                            (DocTab::Section,   "Section"),
                            (DocTab::Bookmarks, "Bookmarks"),
                        ],
                        move || tab.get(),
                        move |t| tab.set(t),
                    )}

                    {section_header("Printer and Paper Size")}
                    {dropdown_row("Canon TR160 series", true)}
                    {dropdown_row("A4", true)}

                    {section_header("Page Orientation")}
                    <hstack gap=12.0 align=AlignItems::Center>
                        {orientation_thumb(false,
                            move || portrait.get(),
                            move || portrait.set(true))}
                        {orientation_thumb(true,
                            move || !portrait.get(),
                            move || portrait.set(false))}
                    </hstack>
                    <label
                        font_size=11.0
                        text_color=TXT_SECONDARY
                    >"20.99 × 29.70 cm"</label>

                    <Hairline />

                    {check_row("Vertical Text",
                        move || v_text.get(),
                        move || v_text.update(|b| *b = !*b))}

                    {section_header("Header & Footer")}
                    <hstack gap=12.0 align=AlignItems::FlexStart>
                        <vstack gap=4.0 flex_grow=1.0>
                            <hstack gap=8.0 align=AlignItems::Center>
                                {check_row("Header",
                                    move || header.get(),
                                    move || header.update(|b| *b = !*b))}
                            </hstack>
                            {measure_field("1.06 cm")}
                            <label
                                font_size=10.0
                                text_color=TXT_SECONDARY
                            >"Top"</label>
                        </vstack>
                        <vstack gap=4.0 flex_grow=1.0>
                            <hstack gap=8.0 align=AlignItems::Center>
                                {check_row("Footer",
                                    move || footer.get(),
                                    move || footer.update(|b| *b = !*b))}
                            </hstack>
                            {measure_field("0.71 cm")}
                            <label
                                font_size=10.0
                                text_color=TXT_SECONDARY
                            >"Bottom"</label>
                        </vstack>
                    </hstack>

                    {check_row("Document Body",
                        move || body.get(),
                        move || body.update(|b| *b = !*b))}

                    {section_header("Document Margins")}
                    <hstack gap=12.0>
                        <vstack gap=4.0 flex_grow=1.0>
                            {measure_field("2.82 cm")}
                            <label
                                font_size=10.0
                                text_color=TXT_SECONDARY
                            >"Top"</label>
                        </vstack>
                        <vstack gap=4.0 flex_grow=1.0>
                            {measure_field("2.54 cm")}
                            <label
                                font_size=10.0
                                text_color=TXT_SECONDARY
                            >"Bottom"</label>
                        </vstack>
                    </hstack>
                    <hstack gap=12.0>
                        <vstack gap=4.0 flex_grow=1.0>
                            {measure_field("2.54 cm")}
                            <label
                                font_size=10.0
                                text_color=TXT_SECONDARY
                            >"Left"</label>
                        </vstack>
                        <vstack gap=4.0 flex_grow=1.0>
                            {measure_field("2.54 cm")}
                            <label
                                font_size=10.0
                                text_color=TXT_SECONDARY
                            >"Right"</label>
                        </vstack>
                    </hstack>

                    {check_row("Facing Pages",
                        move || facing.get(),
                        move || facing.update(|b| *b = !*b))}
                    {check_row("Hyphenation",
                        move || hyphenation.get(),
                        move || hyphenation.update(|b| *b = !*b))}
                    {check_row("Ligatures",
                        move || ligatures.get(),
                        move || ligatures.update(|b| *b = !*b))}

                    <button
                        bordered=false
                        corner_radius=6.0
                        padding=Edges::axis(0.0, 8.0)
                        background_color=FIELD_BG
                        text_color=TXT_PRIMARY
                        font_size=12.0
                        bold=true
                        margin=Edges::ZERO.top(8.0)
                    >"Mail Merge"</button>
                </vstack>
            </scroll_view>
        }
    }

    // ---- format inspector -------------------------------------------

    #[component]
    fn FormatInspector() -> impl IntoView {
        let tab = RwSignal::new(FmtTab::Style);
        let bold = RwSignal::new(false);
        let italic = RwSignal::new(false);
        let under = RwSignal::new(false);
        let strike = RwSignal::new(false);
        let drop_cap = RwSignal::new(true);

        view! {
            <scroll_view flex_grow=1.0 autohides_scrollers=true>
                <vstack padding=Edges::all(16.0) gap=12.0>
                    <label
                        font_size=11.0
                        bold=true
                        text_color=TXT_PRIMARY
                        alignment=TextAlignment::CENTER
                    >"Text"</label>

                    {dropdown_row("Body", false)}

                    {segmented(
                        vec![
                            (FmtTab::Style,  "Style"),
                            (FmtTab::Layout, "Layout"),
                            (FmtTab::More,   "More"),
                        ],
                        move || tab.get(),
                        move |t| tab.set(t),
                    )}

                    {section_header("Font")}
                    {dropdown_row("Helvetica Neue", false)}

                    <hstack gap=8.0>
                        <vstack flex_grow=1.0>
                            {dropdown_row("Multiple", false)}
                        </vstack>
                        <vstack width=72.0>
                            {measure_field("12 pt")}
                        </vstack>
                    </hstack>

                    // BIUS row
                    <hstack
                        gap=0.0
                        background_color=FIELD_BG
                        corner_radius=4.0
                        padding=2.0
                    >
                        {style_btn("B", move || bold.get(),   move || bold.update(|b| *b = !*b))}
                        {style_btn("I", move || italic.get(), move || italic.update(|b| *b = !*b))}
                        {style_btn("U", move || under.get(),  move || under.update(|b| *b = !*b))}
                        {style_btn("S", move || strike.get(), move || strike.update(|b| *b = !*b))}
                        <vstack
                            width=1.0
                            margin=Edges::axis(2.0, 4.0)
                            background_color=HAIRLINE
                        />
                        {style_btn("⚙", || false, || {})}
                    </hstack>

                    {section_header("Character Styles")}
                    {dropdown_row("None", false)}

                    {section_header("Text Colour")}
                    <hstack gap=8.0 align=AlignItems::Center>
                        <hstack
                            size=22.0
                            background_color=Color::WHITE
                            corner_radius=11.0
                            border_width=2.0
                            border_color=Color::rgb(0.7, 0.7, 0.7)
                        />
                        <label
                            flex_grow=1.0
                            font_size=10.0
                            text_color=TXT_SECONDARY
                        >"◇"</label>
                    </hstack>

                    // Alignment row
                    <hstack
                        gap=0.0
                        background_color=FIELD_BG
                        corner_radius=4.0
                        padding=2.0
                    >
                        {align_btn("☱", true)}
                        {align_btn("☰", false)}
                        {align_btn("☴", false)}
                        {align_btn("☷", false)}
                        <vstack
                            width=1.0
                            margin=Edges::axis(2.0, 4.0)
                            background_color=HAIRLINE
                        />
                        {align_btn("↥", false)}
                        {align_btn("↧", false)}
                    </hstack>

                    // Bullets and indent
                    <hstack gap=4.0>
                        <vstack flex_grow=1.0>
                            {dropdown_row("0", false)}
                        </vstack>
                        <vstack flex_grow=1.0>
                            {dropdown_row("0", false)}
                        </vstack>
                    </hstack>

                    {section_header("Spacing")}
                    {dropdown_row("1.1", false)}

                    {section_header("Bullets & Lists")}
                    {dropdown_row("None", false)}

                    // Drop Cap section — checkbox + lines/chars steppers
                    {check_row("Drop Cap",
                        move || drop_cap.get(),
                        move || drop_cap.update(|b| *b = !*b))}
                    <hstack gap=8.0>
                        <vstack gap=4.0 flex_grow=1.0>
                            {measure_field("3")}
                            <label
                                font_size=10.0
                                text_color=TXT_SECONDARY
                            >"Lines"</label>
                        </vstack>
                        <vstack gap=4.0 flex_grow=1.0>
                            {measure_field("1")}
                            <label
                                font_size=10.0
                                text_color=TXT_SECONDARY
                            >"Characters"</label>
                        </vstack>
                        <vstack gap=4.0 width=40.0>
                            <hstack
                                size=28.0
                                background_color=FIELD_BG
                                corner_radius=4.0
                                border_width=0.5
                                border_color=FIELD_BORDER
                                align=AlignItems::Center
                                justify_content=JustifyContent::Center
                            >
                                <label
                                    font_size=14.0
                                    text_color=TXT_PRIMARY
                                    bold=true
                                >"A"</label>
                            </hstack>
                            <label
                                font_size=10.0
                                text_color=TXT_SECONDARY
                            >"Options"</label>
                        </vstack>
                    </hstack>
                </vstack>
            </scroll_view>
        }
    }

    /// Toggleable B/I/U/S style button inside the format-bar group.
    fn style_btn(
        glyph: &'static str,
        active: impl Fn() -> bool + Send + Sync + 'static + Copy,
        set: impl Fn() + Send + Sync + 'static + Copy,
    ) -> impl IntoView {
        view! {
            <button
                bordered=false
                flex_grow=1.0
                padding=Edges::axis(0.0, 4.0)
                corner_radius=3.0
                background_color=move || if active() {
                    Color::rgba(1.0, 1.0, 1.0, 0.18)
                } else {
                    Color::rgba(1.0, 1.0, 1.0, 0.0)
                }
                text_color=TXT_PRIMARY
                font_size=12.0
                bold=true
                on:click=move |_| set()
            >{glyph}</button>
        }
    }

    fn align_btn(glyph: &'static str, active: bool) -> impl IntoView {
        view! {
            <button
                bordered=false
                flex_grow=1.0
                padding=Edges::axis(0.0, 4.0)
                corner_radius=3.0
                background_color=if active { Color::rgba(1.0, 1.0, 1.0, 0.18) } else { Color::rgba(1.0, 1.0, 1.0, 0.0) }
                text_color=TXT_PRIMARY
                font_size=12.0
            >{glyph}</button>
        }
    }

    #[component]
    fn Hairline() -> impl IntoView {
        view! {
            <vstack
                height=1.0
                background_color=HAIRLINE
                margin=Edges::axis(0.0, 4.0)
            />
        }
    }

    pub fn main() {
        mount_to_split_window("Untitled", (1100.0, 760.0), || {
            // Inspector signal lives at this closure's Owner scope
            // (which is the mount_to_split_window-installed Owner)
            // so it survives both panes' reactive lifetimes.
            let inspector = RwSignal::new(Inspector::Document);
            provide_context(inspector);

            view! {
                <split_view vertical=true>
                    // Main pane: takes flex space, holds size last
                    // (low holding priority) so the inspector stays
                    // at its preferred width on window resize.
                    <split_pane holding_priority=199.0>
                        <MainPane />
                    </split_pane>
                    // Inspector flyout: NSSplitView animates the
                    // collapse/expand via the system's standard
                    // curve when the `collapsed` signal flips.
                    <split_pane
                        behavior=PaneBehavior::Inspector
                        preferred_thickness=290.0
                        minimum_thickness=240.0
                        maximum_thickness=420.0
                        can_collapse=true
                        collapsed=move || inspector.get() == Inspector::Hidden
                    >
                        <InspectorPane />
                    </split_pane>
                </split_view>
            }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
