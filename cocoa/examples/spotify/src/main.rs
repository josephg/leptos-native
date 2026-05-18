//! Spotify desktop UI — a non-functional mockup that stress-tests
//! the cocoa port's layout, styling, and reactivity. Three pages
//! (Home, Playlist, Artist) swap via the sidebar and the recent
//! search shortcuts at the top of the home grid.
//!
//! All data is hardcoded; clicks navigate but do nothing else.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;

    // ---- palette ----------------------------------------------------

    const BG_BODY: Color    = Color::Rgba { r: 0.000, g: 0.000, b: 0.000, a: 1.0 };
    #[allow(dead_code)] // Anchor for the resting card colour;
    // referenced by `card_hover_bg`'s lerp endpoints in spirit
    // (kept as a design-token constant rather than inlined).
    const BG_PANEL: Color   = Color::Rgba { r: 0.071, g: 0.071, b: 0.071, a: 1.0 }; // #121212
    const BG_RAISED: Color  = Color::Rgba { r: 0.094, g: 0.094, b: 0.094, a: 1.0 }; // #181818
    const BG_ROW_HOVER: Color = Color::Rgba { r: 0.165, g: 0.165, b: 0.165, a: 1.0 }; // #2a2a2a
    const BG_CHIP: Color    = Color::Rgba { r: 0.165, g: 0.165, b: 0.165, a: 1.0 };
    const BG_CHIP_ACTIVE: Color = Color::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    const TXT_PRIMARY: Color   = Color::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    const TXT_SECONDARY: Color = Color::Rgba { r: 0.702, g: 0.702, b: 0.702, a: 1.0 }; // #B3B3B3
    const TXT_MUTED: Color     = Color::Rgba { r: 0.4, g: 0.4, b: 0.4, a: 1.0 };
    const ACCENT_GREEN: Color  = Color::Rgba { r: 0.114, g: 0.725, b: 0.329, a: 1.0 }; // #1DB954

    // Album-art placeholder palettes — bright color squares with a
    // single-letter "cover" so we can fake the iconography without
    // real images.
    const COVER_COLORS: &[(f32, f32, f32)] = &[
        (0.49, 0.43, 0.92), // purple - liked
        (0.93, 0.71, 0.59), // peach - discover
        (0.21, 0.55, 0.78), // blue
        (0.74, 0.35, 0.20), // rust
        (0.32, 0.39, 0.33), // forest
        (0.65, 0.18, 0.18), // crimson
        (0.18, 0.43, 0.35), // teal
        (0.83, 0.55, 0.20), // amber
    ];

    // ---- model ------------------------------------------------------

    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    enum Page {
        Home,
        Playlist,
        Artist,
    }

    #[derive(Clone)]
    struct LibraryItem {
        id: usize,
        title: &'static str,
        subtitle: &'static str,
        cover_idx: usize,
        opens_to: Page,
    }

    fn library() -> Vec<LibraryItem> {
        vec![
            LibraryItem { id: 1,  title: "Liked Songs",       subtitle: "Playlist · 511 songs",          cover_idx: 0, opens_to: Page::Playlist },
            LibraryItem { id: 2,  title: "Discover Weekly",   subtitle: "Playlist · Made for you",  cover_idx: 1, opens_to: Page::Home },
            LibraryItem { id: 3,  title: "SEA SHANTIES THAT DROP MY PANTIES", subtitle: "Playlist · Luna Terra", cover_idx: 2, opens_to: Page::Home },
            LibraryItem { id: 4,  title: "Chilled Jazz",      subtitle: "Album · Ramin Djawadi",         cover_idx: 3, opens_to: Page::Home },
            LibraryItem { id: 5,  title: "Westworld: Season 1 (Music from the HBO S…", subtitle: "Album · Ramin Djawadi", cover_idx: 4, opens_to: Page::Home },
            LibraryItem { id: 6,  title: "The Köln Concert",  subtitle: "Album · Keith Jarrett",         cover_idx: 5, opens_to: Page::Home },
            LibraryItem { id: 7,  title: "Between Wind And Water", subtitle: "Album · Steve Tibbetts",  cover_idx: 6, opens_to: Page::Home },
            LibraryItem { id: 8,  title: "Árstíðir – Árstíðir", subtitle: "Playlist · Cool dude",        cover_idx: 7, opens_to: Page::Home },
            LibraryItem { id: 9,  title: "Flow",              subtitle: "Playlist · The Longest Johns", cover_idx: 0, opens_to: Page::Home },
            LibraryItem { id: 10, title: "sea shanties you could fight god to", subtitle: "Playlist · 5", cover_idx: 1, opens_to: Page::Home },
            LibraryItem { id: 11, title: "Black Holes and Revelations", subtitle: "Album · Muse",       cover_idx: 5, opens_to: Page::Home },
        ]
    }

    // ---- helpers ----------------------------------------------------

    /// Square "album art" placeholder. Uses a colored panel + the
    /// first letter of the title at large bold size, since the UI
    /// library has no easy image-from-data path here.
    fn cover_block(size: f32, cover_idx: usize, letter: &'static str) -> impl IntoView {
        let (r, g, b) = COVER_COLORS[cover_idx % COVER_COLORS.len()];
        let bg = Color::rgb(r, g, b);
        view! {
            <vstack
                size=size
                background_color=bg
                corner_radius=4.0
                align=AlignItems::Center
                justify_content=JustifyContent::Center
            >
                <label
                    text_color=TXT_PRIMARY
                    font_size=size as f64 * 0.42
                    bold=true
                    alignment=TextAlignment::CENTER
                >
                    {letter}
                </label>
            </vstack>
        }
    }

    /// Reactive background colour for the outer card panel — fades
    /// from `BG_PANEL` (resting) to `BG_RAISED` (hover) over a
    /// 0.3s ease-in-out. Returns the (raw_signal, color_closure)
    /// pair so the caller can bind both: `bind:mouse_hover=raw`
    /// + `background_color=bg`.
    fn card_hover_bg() -> (RwSignal<bool>, impl Fn() -> Color + Send + Sync + Copy + 'static) {
        use leptos::cocoa::animation::{ease_in_out, with_animation};
        let raw = RwSignal::new(false);
        let progress = RwSignal::new(0.0_f64);
        Effect::new(move |_| {
            let on = raw.get();
            with_animation(ease_in_out(0.3), move || {
                progress.set(if on { 1.0 } else { 0.0 });
            });
        });
        let bg = move || {
            // 0.071 → 0.165 (#121212 → #2a2a2a). Component-wise lerp.
            let t = progress.get() as f32;
            let lerp = |a: f32, b: f32| a + (b - a) * t;
            Color::rgb(lerp(0.071, 0.165), lerp(0.071, 0.165), lerp(0.071, 0.165))
        };
        (raw, bg)
    }

    /// "Pill" toggle chip — the filter buttons under "Your Library".
    /// Active chips are white-on-black; inactive are light-on-dark.
    fn chip<F>(label: &'static str, active: F) -> impl IntoView
    where
        F: Fn() -> bool + Send + Sync + 'static + Copy,
    {
        view! {
            <button
                corner_radius=14.0
                bordered=false
                background_color=move || if active() { BG_CHIP_ACTIVE } else { BG_CHIP }
                text_color=move || if active() { Color::BLACK } else { TXT_PRIMARY }
                font_size=12.0
                padding=2.0
            >
                {label}
            </button>
        }
    }

    // ---- root -------------------------------------------------------

    #[component]
    pub fn App() -> impl IntoView {
        let page = RwSignal::new(Page::Home);
        provide_context(page);

        view! {
            <window
                title="Spotify"
                size=(1100.0, 760.0)
                toolbar_style=WindowToolbarStyle::Unified
            >
                // Native NSToolbar across the title bar. Replaces the
                // previous custom `<TopBar/>` hstack. The search field
                // and the embedded "Explore Premium" label have moved
                // into a slim sub-bar below the toolbar, since
                // `<toolbar_item>` is a leaf-attribute element in v1
                // and can't host an `<text_field>` directly.
                //
                // The toolbar must be a child of `<window>` (not a
                // top-level sibling) so its mount can walk up the
                // parent NSView and call `setToolbar:` on the
                // containing NSWindow.
                <toolbar
                    identifier="spotify.main"
                    display_mode=ToolbarDisplayMode::IconOnly
                >
                    // Back / forward — navigational items, styled as
                    // back/forward by AppKit (macOS 12+).
                    <toolbar_item
                        identifier="back"
                        label="Back"
                        icon=Icon::sf_symbol("chevron.left")
                        tool_tip="Go back"
                        navigational=true
                        bordered=true
                        on:action=move |_| page.set(Page::Home)
                    />
                    <toolbar_item
                        identifier="forward"
                        label="Forward"
                        icon=Icon::sf_symbol("chevron.right")
                        tool_tip="Go forward"
                        navigational=true
                        bordered=true
                    />

                    <toolbar_space/>

                    // Home — single bordered button.
                    <toolbar_item
                        identifier="home"
                        label="Home"
                        icon=Icon::sf_symbol("house.fill")
                        tool_tip="Home"
                        bordered=true
                        on:action=move |_| page.set(Page::Home)
                    />

                    // Native NSSearchToolbarItem — gives the proper
                    // search-field chrome (magnifying-glass icon,
                    // clear ×, recent-searches support) and the
                    // correct toolbar expand/collapse behaviour.
                    <toolbar_search_item
                        identifier="search"
                        label="Search"
                        tool_tip="Search Spotify"
                        placeholder="What do you want to play?"
                        preferred_width=320.0
                    />

                    <toolbar_flexible_space/>

                    // Trailing controls.
                    <toolbar_item
                        identifier="premium"
                        label="Premium"
                        icon=Icon::sf_symbol("sparkles")
                        tool_tip="Explore Premium"
                        bordered=true
                    />
                    <toolbar_item
                        identifier="notifications"
                        label="Notifications"
                        icon=Icon::sf_symbol("bell")
                        tool_tip="Notifications"
                        bordered=true
                    />
                    <toolbar_item
                        identifier="account"
                        label="Account"
                        icon=Icon::sf_symbol("person.crop.circle.fill")
                        tool_tip="Your account"
                        bordered=true
                    />
                </toolbar>

                <vstack flex_grow=1.0 background_color=BG_BODY>
                    <hstack flex_grow=1.0 gap=8.0 padding=8.0>
                        <Sidebar />
                        <vstack
                            flex_grow=1.0
                            background_color=BG_RAISED
                            corner_radius=8.0
                            overflow=Overflow::Clip
                        >
                            <Switch>
                                <Match when=move || page.get() == Page::Home>
                                    <HomePage />
                                </Match>
                                <Match when=move || page.get() == Page::Playlist>
                                    <PlaylistPage />
                                </Match>
                                <Match when=move || page.get() == Page::Artist>
                                    <ArtistPage />
                                </Match>
                            </Switch>
                        </vstack>
                    </hstack>
                    <PlayerBar />
                </vstack>
            </window>
        }
    }

    // ---- sidebar ----------------------------------------------------

    #[component]
    fn Sidebar() -> impl IntoView {
        let page = use_context::<RwSignal<Page>>().expect("page ctx");
        let selected_filter = RwSignal::new(0_usize); // 0 = downloaded
        let items = library();

        view! {
            <vstack
                width=Dim::px(320.0)
                background_color=BG_RAISED
                corner_radius=8.0
                overflow=Overflow::Clip
            >
                // Header row
                <hstack
                    padding=16.0
                    justify_content=JustifyContent::SpaceBetween
                    align=AlignItems::Center
                >
                    <label
                        text_color=TXT_PRIMARY
                        font_size=15.0
                        bold=true
                    >"Your Library"</label>
                    <hstack gap=8.0 align=AlignItems::Center>
                        <button
                            bordered=false
                            background_color=Color::rgba(1.0, 1.0, 1.0, 0.0)
                            text_color=TXT_SECONDARY
                            font_size=12.0
                            bold=true
                        >"+ Create"</button>
                    </hstack>
                </hstack>

                // Filter chips
                <hstack
                    padding=8.0
                    gap=6.0
                    align=AlignItems::Center
                >
                    <button
                        corner_radius=14.0
                        bordered=false
                        background_color=BG_CHIP
                        text_color=TXT_PRIMARY
                        font_size=12.0
                        on:click=move |_| selected_filter.set(0)
                    >"\u{2715}"</button>
                    {chip("Downloaded", move || selected_filter.get() == 1)}
                    {chip("Playlists",  move || selected_filter.get() == 2)}
                    {chip("Albums",     move || selected_filter.get() == 3)}
                </hstack>

                // Search bar + "Recents"
                <hstack
                    padding=8.0
                    justify_content=JustifyContent::SpaceBetween
                    align=AlignItems::Center
                >
                    <label text_color=TXT_SECONDARY font_size=14.0 padding=4.0>
                        "\u{1F50D}"
                    </label>
                    <label text_color=TXT_SECONDARY font_size=12.0>
                        "Recents \u{2630}"
                    </label>
                </hstack>

                // List of library items
                <scroll_view flex_grow=1.0 autohides_scrollers=true>
                    <vstack padding=4.0 gap=2.0>
                        <For
                            each=move || items.clone()
                            key=|i| i.id
                            children=move |item| {
                                let opens = item.opens_to;
                                let cover_idx = item.cover_idx;
                                let title = item.title;
                                let subtitle = item.subtitle;
                                let first_letter: &'static str = match title.chars().next().unwrap_or('?') {
                                    'L' => "L", 'D' => "D", 'S' => "S", 'C' => "C",
                                    'W' => "W", 'T' => "T", 'B' => "B", 'Á' => "Á",
                                    'F' => "F", 'A' => "A",
                                    _ => "•",
                                };
                                view! {
                                    <hstack
                                        padding=8.0
                                        gap=12.0
                                        corner_radius=6.0
                                        align=AlignItems::Center
                                        background_color=move || if page.get() == opens && opens != Page::Home {
                                            BG_ROW_HOVER
                                        } else {
                                            Color::rgba(0.0, 0.0, 0.0, 0.0)
                                        }
                                    >
                                        {cover_block(48.0, cover_idx, first_letter)}
                                        <vstack gap=2.0 flex_grow=1.0>
                                            <label
                                                text_color=TXT_PRIMARY
                                                font_size=13.0
                                                bold=true
                                            >{title}</label>
                                            <hstack gap=4.0 align=AlignItems::Center>
                                                <label
                                                    text_color=ACCENT_GREEN
                                                    font_size=10.0
                                                >"\u{2B07}"</label>
                                                <label
                                                    text_color=TXT_SECONDARY
                                                    font_size=11.0
                                                >{subtitle}</label>
                                            </hstack>
                                        </vstack>
                                        <button
                                            bordered=false
                                            background_color=Color::rgba(0.0, 0.0, 0.0, 0.0)
                                            text_color=TXT_PRIMARY
                                            font_size=12.0
                                            on:click=move |_| page.set(opens)
                                        >"Open"</button>
                                    </hstack>
                                }
                            }
                        />
                    </vstack>
                </scroll_view>
            </vstack>
        }
    }

    // ---- home page --------------------------------------------------

    #[component]
    fn HomePage() -> impl IntoView {
        let page = use_context::<RwSignal<Page>>().expect("page ctx");
        let filter_idx = RwSignal::new(0_usize);

        // 8 colored "recent shortcut" tiles
        let recents = vec![
            ("Discover Weekly",         1, "D"),
            ("Pastel Blues",            2, "P"),
            ("Liked Songs",             0, "\u{2665}"),
            ("All Or Nothing At All",   3, "A"),
            ("triple j's Hottest 100 1997", 7, "t"),
            ("Kind Of Blue",            6, "K"),
            ("The Köln Concert",        5, "T"),
            ("Westworld: Season 1",     4, "W"),
        ];

        // Daily mixes — enough to always overflow horizontally
        // (≈3.8k px wide at 200/card, exceeds any laptop monitor).
        let daily_mixes = vec![
            ("Ludovico Einaudi, Hania Rani, Nat King Cole and …",  0, "1",  "Daily Mix"),
            ("Damon Korb, Massamasta, Lena Raine and …",           1, "2",  "Daily Mix"),
            ("Vikingur Olafsson, Olga Scheps, Alexandra…",         4, "3",  "Daily Mix"),
            ("Module, Chipset, Shrobokon and more",                3, "4",  "Daily Mix"),
            ("Vikingur Olafsson, Christopher Tin, Andrea…",        6, "5",  "Daily Mix"),
            ("Burial, Four Tet, Brian Eno and …",                  2, "6",  "Daily Mix"),
            ("Jon Hopkins, Nils Frahm, Olafur Arnalds …",          5, "7",  "Daily Mix"),
            ("Beach House, Slowdive, Cocteau Twins and …",         7, "8",  "Daily Mix"),
            ("Charli XCX, Caroline Polachek, Robyn …",             0, "9",  "Made For You"),
            ("Mac DeMarco, Whitney, Beach Fossils …",              3, "10", "Discover"),
            ("Tame Impala, Pond, Connan Mockasin …",               4, "11", "Daily Mix"),
            ("Sufjan Stevens, Bon Iver, Phoebe Bridgers …",        1, "12", "Daily Mix"),
            ("Aphex Twin, Boards of Canada, Autechre …",           6, "13", "Discover"),
            ("Jamie xx, Floating Points, Caribou …",               2, "14", "Daily Mix"),
            ("Solange, Frank Ocean, Blood Orange …",               5, "15", "Made For You"),
            ("Khruangbin, Mild High Club, Unknown Mortal Orch.",   7, "16", "Daily Mix"),
            ("Big Thief, Adrianne Lenker, Florist …",              3, "17", "Daily Mix"),
            ("Caroline Shaw, Nico Muhly, Anna Meredith …",         0, "18", "Discover"),
        ];

        // Albums — likewise. ≈5.3k px wide at 200/card.
        let albums = vec![
            ("Wild Life",         2, "W"),
            ("Charm Bracelet",    5, "C"),
            ("Lover",             6, "L"),
            ("Radiohead",         3, "R"),
            ("Yellow",            7, "Y"),
            ("Hounds of Love",    1, "H"),
            ("Endtroducing…",     0, "E"),
            ("Selected Ambient",  4, "S"),
            ("In Rainbows",       6, "I"),
            ("Punisher",          2, "P"),
            ("Blue",              3, "B"),
            ("Talkie Walkie",     5, "T"),
            ("Currents",          7, "C"),
            ("Untrue",            1, "U"),
            ("Kid A",             0, "K"),
            ("Pure Comedy",       4, "P"),
            ("Black Messiah",     6, "B"),
            ("Stankonia",         3, "S"),
            ("To Pimp A Butterfly", 2, "T"),
            ("Anniemal",          5, "A"),
            ("Plastic Beach",     7, "P"),
            ("Carrie & Lowell",   1, "C"),
            ("Ys",                0, "Y"),
            ("Music Has The Right", 4, "M"),
            ("OK Computer",       6, "O"),
        ];

        view! {
            <scroll_view flex_grow=1.0 autohides_scrollers=true>
                <vstack padding=24.0 gap=24.0>
                    // Top filter pills
                    <hstack gap=8.0 align=AlignItems::Center>
                        {["All", "Music", "Podcasts", "Audiobooks"]
                            .iter()
                            .enumerate()
                            .map(|(i, label)| view! {
                                <button
                                    corner_radius=16.0
                                    bordered=false
                                    padding=12.0
                                    background_color=move || if filter_idx.get() == i {
                                        BG_CHIP_ACTIVE
                                    } else {
                                        BG_CHIP
                                    }
                                    text_color=move || if filter_idx.get() == i {
                                        Color::BLACK
                                    } else {
                                        TXT_PRIMARY
                                    }
                                    font_size=12.0
                                    bold=true
                                    on:click=move |_| filter_idx.set(i)
                                >
                                    {*label}
                                </button>
                            })
                            .collect::<Vec<_>>()}
                    </hstack>

                    // Recent shortcut "grid" — two horizontal rows of
                    // four chips each. (Taffy's Grid type contains a
                    // raw pointer through CompactLength and isn't
                    // `Send`, so it can't go inside a `#[component]`
                    // body; two hstacks fill the same role.)
                    <vstack gap=12.0>
                        <hstack gap=12.0>
                            {recents.iter().take(4).cloned().map(|(title, cover, letter)| view! {
                                <hstack
                                    flex_grow=1.0
                                    height=56.0
                                    gap=10.0
                                    background_color=BG_ROW_HOVER
                                    corner_radius=6.0
                                    align=AlignItems::Center
                                    overflow=Overflow::Clip
                                >
                                    {cover_block(56.0, cover, letter)}
                                    <label
                                        text_color=TXT_PRIMARY
                                        font_size=12.0
                                        bold=true
                                        flex_grow=1.0
                                    >{title}</label>
                                </hstack>
                            }).collect::<Vec<_>>()}
                        </hstack>
                        <hstack gap=12.0>
                            {recents.iter().skip(4).cloned().map(|(title, cover, letter)| view! {
                                <hstack
                                    flex_grow=1.0
                                    height=56.0
                                    gap=10.0
                                    background_color=BG_ROW_HOVER
                                    corner_radius=6.0
                                    align=AlignItems::Center
                                    overflow=Overflow::Clip
                                >
                                    {cover_block(56.0, cover, letter)}
                                    <label
                                        text_color=TXT_PRIMARY
                                        font_size=12.0
                                        bold=true
                                        flex_grow=1.0
                                    >{title}</label>
                                </hstack>
                            }).collect::<Vec<_>>()}
                        </hstack>
                    </vstack>

                    // "Made For you" section — horizontally
                    // scrollable strip of daily-mix cards.
                    <vstack gap=12.0>
                        <hstack
                            justify_content=JustifyContent::SpaceBetween
                            align=AlignItems::Center
                        >
                            <vstack gap=2.0>
                                <label text_color=TXT_SECONDARY font_size=11.0>"Made For"</label>
                                <label text_color=TXT_PRIMARY font_size=22.0 bold=true>"you"</label>
                            </vstack>
                            <label text_color=TXT_SECONDARY font_size=11.0 bold=true>"Show all"</label>
                        </hstack>
                        <scroll_view
                            axis=ScrollAxis::Horizontal
                            min_height=260.0
                            autohides_scrollers=true
                        >
                            <hstack gap=14.0>
                                {daily_mixes.into_iter().map(|(subtitle, cover, num, label_text)| {
                                    let (hover_raw, hover_bg) = card_hover_bg();
                                    view! {
                                        <vstack
                                            width=200.0
                                            gap=10.0
                                            padding=12.0
                                            background_color=hover_bg
                                            corner_radius=8.0
                                            bind:mouse_hover=hover_raw
                                        >
                                            {cover_block(140.0, cover, num)}
                                            <label
                                                text_color=TXT_PRIMARY
                                                font_size=13.0
                                                bold=true
                                            >{label_text}</label>
                                            <label
                                                text_color=TXT_SECONDARY
                                                font_size=11.0
                                                multiline=true
                                            >{subtitle}</label>
                                        </vstack>
                                    }
                                }).collect::<Vec<_>>()}
                            </hstack>
                        </scroll_view>
                    </vstack>

                    // "Albums featuring songs you like" — horizontally
                    // scrollable strip. The cards inside have fixed
                    // widths (no flex_grow) so they keep their
                    // natural sizes inside the scroll_view's
                    // content-sized documentView wrapper.
                    <vstack gap=12.0>
                        <hstack
                            justify_content=JustifyContent::SpaceBetween
                            align=AlignItems::Center
                        >
                            <label text_color=TXT_PRIMARY font_size=22.0 bold=true>
                                "Albums featuring songs you like"
                            </label>
                            <label text_color=TXT_SECONDARY font_size=11.0 bold=true>"Show all"</label>
                        </hstack>
                        <scroll_view
                            axis=ScrollAxis::Horizontal
                            min_height=210.0
                            autohides_scrollers=true
                        >
                            <hstack gap=14.0>
                                {albums.into_iter().map(|(name, cover, letter)| {
                                    let (hover_raw, hover_bg) = card_hover_bg();
                                    view! {
                                        <vstack
                                            width=200.0
                                            gap=10.0
                                            padding=12.0
                                            background_color=hover_bg
                                            corner_radius=8.0
                                            bind:mouse_hover=hover_raw
                                        >
                                            {cover_block(140.0, cover, letter)}
                                            <label
                                                text_color=TXT_PRIMARY
                                                font_size=13.0
                                                bold=true
                                            >{name}</label>
                                        </vstack>
                                    }
                                }).collect::<Vec<_>>()}
                            </hstack>
                        </scroll_view>
                    </vstack>

                    // Pad a bit at the bottom so the player bar doesn't kiss the content
                    <vstack height=8.0 />

                    // Hidden "navigate to artist" affordance so the demo
                    // exercises the third page even without a clickable
                    // hero image.
                    <hstack gap=8.0>
                        <button
                            corner_radius=18.0
                            bordered=false
                            background_color=BG_CHIP
                            text_color=TXT_PRIMARY
                            font_size=12.0
                            bold=true
                            on:click=move |_| page.set(Page::Artist)
                        >"\u{2192} Visit artist page"</button>
                        <button
                            corner_radius=18.0
                            bordered=false
                            background_color=BG_CHIP
                            text_color=TXT_PRIMARY
                            font_size=12.0
                            bold=true
                            on:click=move |_| page.set(Page::Playlist)
                        >"\u{2192} Open playlist"</button>
                    </hstack>
                </vstack>
            </scroll_view>
        }
    }

    // ---- playlist page (Liked Songs) --------------------------------

    #[component]
    fn PlaylistPage() -> impl IntoView {
        let tracks = vec![
            ("1", "Mazurka No. 6 in A Minor, Op. 7 No. 2: Vivo", "Frédéric Chopin, Vladimir Ashkenazy", "Chopin: Mazurkas", "3 weeks ago", "3:14"),
            ("2", "Sinnerman", "Nina Simone", "Pastel Blues", "30 Sept 2025", "10:22"),
            ("3", "Claudio Constantini", "Nina Simone", "Flow", "23 May 2025", "5:11"),
            ("4", "Tinerno", "New Cool Collective", "Electric Monkey Sessions", "21 Apr 2025", "4:34"),
            ("5", "Köln, January 24, 1975, Part 1 - Live", "Keith Jarrett", "The Köln Concert", "18 Nov 2024", "26:02"),
            ("6", "The Heart Asks Pleasure First / The Promise", "Michael Nyman", "The Piano OST", "8 Aug 2024", "5:36"),
            ("7", "Spiegel im Spiegel", "Arvo Pärt", "Alina", "1 Jun 2024", "9:55"),
            ("8", "Une Barque sur l'Océan", "Maurice Ravel", "Miroirs", "4 Apr 2024", "8:01"),
        ];

        let chips = vec!["Soundtrack", "Electronic", "Rock", "Classy", "Folk", "Latin", "Jazz", "Rap", "Cabaret", "Dance", "Classical"];

        view! {
            <scroll_view flex_grow=1.0 autohides_scrollers=true>
                <vstack padding=0.0 gap=0.0>
                    // Hero block — purple gradient stand-in
                    <hstack
                        padding=24.0
                        gap=20.0
                        background_color=Color::rgb(0.42, 0.36, 0.85)
                        align=AlignItems::End
                        height=260.0
                    >
                        <vstack
                            width=Dim::px(200.0)
                            height=200.0
                            background_color=Color::rgb(0.36, 0.30, 0.85)
                            corner_radius=4.0
                            align=AlignItems::Center
                            justify_content=JustifyContent::Center
                        >
                            <label
                                text_color=TXT_PRIMARY
                                font_size=84.0
                                bold=true
                            >"\u{2665}"</label>
                        </vstack>
                        <vstack gap=8.0 flex_grow=1.0>
                            <label text_color=TXT_PRIMARY font_size=11.0 bold=true>"Playlist"</label>
                            <label
                                text_color=TXT_PRIMARY
                                font_size=72.0
                                bold=true
                                multiline=true
                            >"Liked Songs"</label>
                            <hstack gap=6.0 align=AlignItems::Center>
                                <label text_color=TXT_PRIMARY font_size=12.0 bold=true>"you"</label>
                                <label text_color=TXT_PRIMARY font_size=12.0>"\u{00B7} 511 songs, 31 hr 33 min"</label>
                            </hstack>
                        </vstack>
                    </hstack>

                    // Action row
                    <hstack
                        padding=24.0
                        gap=24.0
                        background_color=Color::rgb(0.12, 0.10, 0.16)
                        align=AlignItems::Center
                    >
                        <button
                            width=56.0
                            height=56.0
                            corner_radius=28.0
                            bordered=false
                            background_color=ACCENT_GREEN
                            text_color=Color::BLACK
                            font_size=22.0
                            bold=true
                        >"\u{25B6}"</button>
                        <label text_color=TXT_SECONDARY font_size=20.0>"\u{1F500}"</label>
                        <label text_color=TXT_SECONDARY font_size=20.0>"\u{2B07}"</label>
                        <vstack flex_grow=1.0 />
                        <label text_color=TXT_SECONDARY font_size=12.0>"\u{1F50D} Recently edited"</label>
                    </hstack>

                    // Genre chips row (horizontal scroll-like — capped to fit)
                    <hstack
                        padding=12.0
                        gap=8.0
                        background_color=Color::rgb(0.12, 0.10, 0.16)
                        wrap=FlexWrap::Wrap
                    >
                        {chips.into_iter().enumerate().map(|(i, c)| view!{
                            <button
                                corner_radius=14.0
                                bordered=false
                                padding=10.0
                                background_color=if i == 0 { BG_CHIP_ACTIVE } else { BG_CHIP }
                                text_color=if i == 0 { Color::BLACK } else { TXT_PRIMARY }
                                font_size=11.0
                                bold=true
                            >{c}</button>
                        }).collect::<Vec<_>>()}
                    </hstack>

                    // Table header
                    <hstack
                        padding=12.0
                        gap=12.0
                        background_color=BG_RAISED
                        align=AlignItems::Center
                    >
                        <label
                            text_color=TXT_SECONDARY
                            font_size=11.0
                            width=Dim::px(20.0)
                            alignment=TextAlignment::CENTER
                        >"#"</label>
                        <label text_color=TXT_SECONDARY font_size=11.0 flex_grow=1.0>"Title"</label>
                        <label
                            text_color=TXT_SECONDARY
                            font_size=11.0
                            width=Dim::px(200.0)
                        >"Album"</label>
                        <label
                            text_color=TXT_SECONDARY
                            font_size=11.0
                            width=Dim::px(110.0)
                        >"Date added"</label>
                        <label
                            text_color=TXT_SECONDARY
                            font_size=11.0
                            width=Dim::px(60.0)
                            alignment=TextAlignment::RIGHT
                        >"\u{1F551}"</label>
                    </hstack>

                    // Track rows
                    <vstack background_color=BG_RAISED padding=4.0 gap=0.0>
                        {tracks.into_iter().enumerate().map(|(i, (_n, title, artist, album, added, dur))| {
                            let cover = i % COVER_COLORS.len();
                            let n = i + 1;
                            view! {
                                <hstack
                                    padding=8.0
                                    gap=12.0
                                    corner_radius=4.0
                                    align=AlignItems::Center
                                >
                                    <label
                                        text_color=TXT_SECONDARY
                                        font_size=12.0
                                        width=Dim::px(20.0)
                                        alignment=TextAlignment::CENTER
                                    >{format!("{}", n)}</label>
                                    {cover_block(36.0, cover, "♪")}
                                    <vstack gap=2.0 flex_grow=1.0>
                                        <label
                                            text_color=TXT_PRIMARY
                                            font_size=13.0
                                            bold=true
                                        >{title}</label>
                                        <label
                                            text_color=TXT_SECONDARY
                                            font_size=11.0
                                        >{artist}</label>
                                    </vstack>
                                    <label
                                        text_color=TXT_SECONDARY
                                        font_size=12.0
                                        width=Dim::px(200.0)
                                    >{album}</label>
                                    <label
                                        text_color=TXT_SECONDARY
                                        font_size=12.0
                                        width=Dim::px(110.0)
                                    >{added}</label>
                                    <label
                                        text_color=TXT_SECONDARY
                                        font_size=12.0
                                        width=Dim::px(60.0)
                                        alignment=TextAlignment::RIGHT
                                    >{dur}</label>
                                </hstack>
                            }
                        }).collect::<Vec<_>>()}
                    </vstack>
                </vstack>
            </scroll_view>
        }
    }

    // ---- artist page (Nina Simone) ----------------------------------

    #[component]
    fn ArtistPage() -> impl IntoView {
        let songs = vec![
            ("1", "Feeling Good",            "615,011,784", "2:54", true),
            ("2", "My Baby Just Cares For Me","270,247,332", "3:35", false),
            ("3", "I Put A Spell On You",    "234,713,195", "2:32", false),
            ("4", "Sinnerman - Soft Tukker Remix", "57,187,490", "3:55", false),
            ("5", "Don't Let Me Be Misunderstood", "89,173,005", "2:51", false),
            ("6", "Ne Me Quitte Pas",         "65,440,221", "4:01", false),
            ("7", "I Wish I Knew How It Would Feel To Be Free", "44,012,950", "2:55", false),
        ];

        view! {
            <scroll_view flex_grow=1.0 autohides_scrollers=true>
                <vstack padding=0.0 gap=0.0>
                    // Hero: portrait stand-in
                    <hstack
                        padding=24.0
                        gap=20.0
                        height=300.0
                        background_color=Color::rgb(0.45, 0.20, 0.20)
                        align=AlignItems::End
                    >
                        // "Cover" — solid block representing the photo
                        <vstack
                            width=Dim::px(240.0)
                            height=240.0
                            background_color=Color::rgb(0.62, 0.27, 0.31)
                            corner_radius=4.0
                            align=AlignItems::Center
                            justify_content=JustifyContent::Center
                        >
                            <label
                                text_color=TXT_PRIMARY
                                font_size=80.0
                                bold=true
                            >"NS"</label>
                        </vstack>
                        <vstack gap=10.0 flex_grow=1.0>
                            <hstack
                                gap=4.0
                                align=AlignItems::Center
                            >
                                <label
                                    text_color=Color::rgb(0.28, 0.66, 0.96)
                                    font_size=11.0
                                    bold=true
                                >"\u{2713}"</label>
                                <label text_color=TXT_PRIMARY font_size=11.0 bold=true>"Verified Artist"</label>
                            </hstack>
                            <label
                                text_color=TXT_PRIMARY
                                font_size=84.0
                                bold=true
                            >"Nina Simone"</label>
                            <label text_color=TXT_PRIMARY font_size=12.0>
                                "9,337,097 monthly listeners"
                            </label>
                        </vstack>
                    </hstack>

                    // Action row
                    <hstack
                        padding=24.0
                        gap=20.0
                        background_color=Color::rgb(0.27, 0.12, 0.12)
                        align=AlignItems::Center
                    >
                        <button
                            width=56.0
                            height=56.0
                            corner_radius=28.0
                            bordered=false
                            background_color=ACCENT_GREEN
                            text_color=Color::BLACK
                            font_size=22.0
                            bold=true
                        >"\u{25B6}"</button>
                        <label text_color=TXT_SECONDARY font_size=20.0>"\u{1F500}"</label>
                        <button
                            corner_radius=18.0
                            bordered=false
                            background_color=Color::rgba(1.0,1.0,1.0,0.0)
                            text_color=TXT_PRIMARY
                            font_size=12.0
                            bold=true
                        >"Follow"</button>
                        <label text_color=TXT_SECONDARY font_size=22.0>"\u{2026}"</label>
                    </hstack>

                    // Popular header + list
                    <vstack
                        padding=24.0
                        gap=12.0
                        background_color=Color::rgb(0.13, 0.06, 0.06)
                        flex_grow=1.0
                    >
                        <label text_color=TXT_PRIMARY font_size=22.0 bold=true>"Popular"</label>
                        <vstack gap=4.0>
                            {songs.into_iter().enumerate().map(|(i, (_n, title, plays, dur, in_lib))| {
                                let cover = (i + 2) % COVER_COLORS.len();
                                let n = i + 1;
                                view! {
                                    <hstack
                                        padding=8.0
                                        gap=14.0
                                        corner_radius=4.0
                                        align=AlignItems::Center
                                    >
                                        <label
                                            text_color=TXT_SECONDARY
                                            font_size=14.0
                                            width=Dim::px(20.0)
                                            alignment=TextAlignment::CENTER
                                        >{format!("{}", n)}</label>
                                        {cover_block(40.0, cover, "M")}
                                        <vstack gap=2.0 flex_grow=1.0>
                                            <label
                                                text_color=TXT_PRIMARY
                                                font_size=14.0
                                                bold=true
                                            >{title}</label>
                                            <label
                                                text_color=TXT_SECONDARY
                                                font_size=11.0
                                            >"Music video"</label>
                                        </vstack>
                                        <label
                                            text_color=TXT_SECONDARY
                                            font_size=12.0
                                            width=Dim::px(120.0)
                                            alignment=TextAlignment::RIGHT
                                        >{plays}</label>
                                        <label
                                            text_color=if in_lib { ACCENT_GREEN } else { TXT_MUTED }
                                            font_size=14.0
                                            width=Dim::px(28.0)
                                            alignment=TextAlignment::CENTER
                                        >{if in_lib { "\u{2713}" } else { "+" }}</label>
                                        <label
                                            text_color=TXT_SECONDARY
                                            font_size=12.0
                                            width=Dim::px(48.0)
                                            alignment=TextAlignment::RIGHT
                                        >{dur}</label>
                                    </hstack>
                                }
                            }).collect::<Vec<_>>()}
                        </vstack>
                    </vstack>
                </vstack>
            </scroll_view>
        }
    }

    // ---- player bar -------------------------------------------------

    #[component]
    fn PlayerBar() -> impl IntoView {
        let progress = RwSignal::new(0.61_f64);
        let volume = RwSignal::new(0.7_f64);

        // Custom status bar at the bottom of the window. NSToolbar
        // doesn't natively support embedded sliders / multi-section
        // content like this; plain hstack with toolbar-ish styling.
        view! {
            <hstack
                height=80.0
                background_color=BG_BODY
                padding=12.0
                gap=12.0
                align=AlignItems::Center
            >
                // Now-playing left
                <hstack
                    gap=10.0
                    flex_grow=1.0
                    align=AlignItems::Center
                >
                    {cover_block(56.0, 4, "W")}
                    <vstack gap=2.0>
                        <label text_color=TXT_PRIMARY font_size=13.0 bold=true>
                            "Wild Is The Wind"
                        </label>
                        <label text_color=TXT_SECONDARY font_size=11.0>
                            "Nina Simone"
                        </label>
                    </vstack>
                    <image_view
                        sf_symbol="checkmark.circle.fill"
                        tint=ACCENT_GREEN
                        width=18.0
                        height=18.0
                        margin=8.0
                    />
                </hstack>

                // Center: playback controls + progress
                <vstack
                    flex_grow=2.0
                    gap=4.0
                    align=AlignItems::Center
                >
                    <hstack
                        gap=20.0
                        align=AlignItems::Center
                    >
                        <image_view sf_symbol="shuffle" tint=TXT_SECONDARY width=16.0 height=16.0 />
                        <image_view sf_symbol="backward.end.fill" tint=TXT_SECONDARY width=16.0 height=16.0 />
                        <button
                            width=36.0
                            height=36.0
                            corner_radius=18.0
                            bordered=false
                            background_color=Color::WHITE
                            text_color=Color::BLACK
                            sf_symbol="play.fill"
                        />
                        <image_view sf_symbol="forward.end.fill" tint=TXT_SECONDARY width=16.0 height=16.0 />
                        <image_view sf_symbol="repeat" tint=TXT_SECONDARY width=16.0 height=16.0 />
                    </hstack>
                    <hstack
                        gap=8.0
                        align=AlignItems::Center
                        width=Dim::px(420.0)
                    >
                        <label
                            text_color=TXT_SECONDARY
                            font_size=10.0
                            width=Dim::px(40.0)
                            alignment=TextAlignment::RIGHT
                        >"4:24"</label>
                        <slider
                            bind:value=progress
                            min_value=0.0
                            max_value=1.0
                            flex_grow=1.0
                        />
                        <label
                            text_color=TXT_SECONDARY
                            font_size=10.0
                            width=Dim::px(40.0)
                        >"6:57"</label>
                    </hstack>
                </vstack>

                // Right: aux controls + volume
                <hstack
                    flex_grow=1.0
                    gap=10.0
                    justify_content=JustifyContent::FlexEnd
                    align=AlignItems::Center
                >
                    <image_view sf_symbol="mic.fill" tint=TXT_SECONDARY width=16.0 height=16.0 />
                    <image_view sf_symbol="speaker.wave.2.fill" tint=TXT_SECONDARY width=16.0 height=16.0 />
                    <slider
                        bind:value=volume
                        min_value=0.0
                        max_value=1.0
                        width=Dim::px(100.0)
                    />
                    <image_view sf_symbol="arrow.up.left.and.arrow.down.right" tint=TXT_SECONDARY width=16.0 height=16.0 />
                </hstack>
            </hstack>
        }
    }

    pub fn main() {
        run(|| view! { <App /> }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
