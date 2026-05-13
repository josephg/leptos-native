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

    const BG_BODY: Color    = Color { r: 0.000, g: 0.000, b: 0.000, a: 1.0 };
    const BG_PANEL: Color   = Color { r: 0.071, g: 0.071, b: 0.071, a: 1.0 }; // #121212
    const BG_RAISED: Color  = Color { r: 0.094, g: 0.094, b: 0.094, a: 1.0 }; // #181818
    const BG_ROW_HOVER: Color = Color { r: 0.165, g: 0.165, b: 0.165, a: 1.0 }; // #2a2a2a
    const BG_CHIP: Color    = Color { r: 0.165, g: 0.165, b: 0.165, a: 1.0 };
    const BG_CHIP_ACTIVE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    const TXT_PRIMARY: Color   = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    const TXT_SECONDARY: Color = Color { r: 0.702, g: 0.702, b: 0.702, a: 1.0 }; // #B3B3B3
    const TXT_MUTED: Color     = Color { r: 0.4, g: 0.4, b: 0.4, a: 1.0 };
    const ACCENT_GREEN: Color  = Color { r: 0.114, g: 0.725, b: 0.329, a: 1.0 }; // #1DB954
    const AVATAR_ORANGE: Color = Color { r: 0.95, g: 0.55, b: 0.16, a: 1.0 };

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
            LibraryItem { id: 2,  title: "Discover Weekly",   subtitle: "Playlist · Made for sineltor",  cover_idx: 1, opens_to: Page::Home },
            LibraryItem { id: 3,  title: "SEA SHANTIES THAT DROP MY PANTIES", subtitle: "Playlist · Luna Terra", cover_idx: 2, opens_to: Page::Home },
            LibraryItem { id: 4,  title: "Chilled Jazz",      subtitle: "Album · Ramin Djawadi",         cover_idx: 3, opens_to: Page::Home },
            LibraryItem { id: 5,  title: "Westworld: Season 1 (Music from the HBO S…", subtitle: "Album · Ramin Djawadi", cover_idx: 4, opens_to: Page::Home },
            LibraryItem { id: 6,  title: "The Köln Concert",  subtitle: "Album · Keith Jarrett",         cover_idx: 5, opens_to: Page::Home },
            LibraryItem { id: 7,  title: "Between Wind And Water", subtitle: "Album · Steve Tibbetts",  cover_idx: 6, opens_to: Page::Home },
            LibraryItem { id: 8,  title: "Árstíðir – Árstíðir", subtitle: "Playlist · sineltor",        cover_idx: 7, opens_to: Page::Home },
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
        // `.size(n)` locks the cover to an n×n square that flex
        // layout can't squeeze (sets width/height/min_w/min_h + flex
        // shrink=0 in one line; otherwise a long sibling title would
        // compress fixed-width children).
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

    /// Filled circular icon button — used for the green play button
    /// and the various circular toolbar buttons in the top bar.
    fn circle_button(
        size: f32,
        bg: Color,
        glyph: &'static str,
        glyph_size: f64,
        on_click: impl FnMut() + Send + 'static,
    ) -> impl IntoView {
        let mut on_click = on_click;
        view! {
            <button
                width=size
                height=size
                corner_radius=size / 2.0
                background_color=bg
                bordered=false
                on:click=move |_| on_click()
                font_size=glyph_size
                bold=true
                alignment=TextAlignment::CENTER
            >
                {glyph}
            </button>
        }
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
            <vstack flex_grow=1.0 background_color=BG_BODY>
                <TopBar />
                <hstack flex_grow=1.0 gap=8.0 padding=8.0>
                    <Sidebar />
                    <vstack
                        flex_grow=1.0
                        background_color=BG_RAISED
                        corner_radius=8.0
                        clip=true
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
        }
    }

    // ---- top bar ----------------------------------------------------

    #[component]
    fn TopBar() -> impl IntoView {
        let page = use_context::<RwSignal<Page>>().expect("page ctx");
        view! {
            <hstack
                height=64.0
                background_color=BG_BODY
                padding=8.0
                gap=8.0
                align=AlignItems::Center
            >
                // Nav: back/forward + home
                <hstack gap=8.0 align=AlignItems::Center>
                    {circle_button(32.0, BG_CHIP, "‹", 18.0,
                        move || page.set(Page::Home))}
                    {circle_button(32.0, BG_CHIP, "›", 18.0, move || {})}
                </hstack>

                // Center: home pill + search field
                <hstack
                    flex_grow=1.0
                    gap=8.0
                    justify_content=JustifyContent::Center
                    align=AlignItems::Center
                >
                    {circle_button(40.0, BG_CHIP, "⌂", 18.0,
                        move || page.set(Page::Home))}
                    <hstack
                        width=Dim::px(440.0)
                        height=40.0
                        background_color=BG_CHIP
                        corner_radius=20.0
                        padding=2.0
                        gap=6.0
                        align=AlignItems::Center
                    >
                        <label
                            text_color=TXT_SECONDARY
                            font_size=14.0
                            padding=8.0
                        >"\u{1F50D}"</label>
                        <text_field
                            placeholder="What do you want to play?"
                            flex_grow=1.0
                            text_color=TXT_PRIMARY
                            font_size=14.0
                            bordered=false
                        />
                        <label
                            text_color=TXT_SECONDARY
                            font_size=14.0
                            padding=8.0
                        >"\u{1F4FB}"</label>
                    </hstack>
                </hstack>

                // Right: notifications + avatar
                <hstack gap=8.0 align=AlignItems::Center>
                    <label text_color=TXT_SECONDARY font_size=14.0>
                        "Explore Premium"
                    </label>
                    {circle_button(32.0, BG_CHIP, "\u{1F514}", 14.0, move || {})}
                    {circle_button(32.0, AVATAR_ORANGE, "S", 14.0, move || {})}
                </hstack>
            </hstack>
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
                clip=true
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

        // 5 daily mixes
        let daily_mixes = vec![
            ("Ludovico Einaudi, Hania Rani, Nat King Cole and …", 0, "1", "Daily Mix"),
            ("Damon Korb, Massamasta, Lena Raine and …",          1, "2", "Daily Mix"),
            ("Vikingur Olafsson, Olga Scheps, Alexandra…",        4, "3", "Daily Mix"),
            ("Module, Chipset, Shrobokon and more",               3, "4", "Daily Mix"),
            ("Vikingur Olafsson, Christopher Tin, Andrea…",       6, "5", "Daily Mix"),
        ];

        // Albums you might like
        let albums = vec![
            ("Wild Life",   2, "W"),
            ("Charm Bracelet", 5, "C"),
            ("Lover",       6, "L"),
            ("Radiohead",   3, "R"),
            ("Yellow",      7, "Y"),
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
                                    clip=true
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
                                    clip=true
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

                    // "Made For sineltor" section
                    <vstack gap=12.0>
                        <hstack
                            justify_content=JustifyContent::SpaceBetween
                            align=AlignItems::Center
                        >
                            <vstack gap=2.0>
                                <label text_color=TXT_SECONDARY font_size=11.0>"Made For"</label>
                                <label text_color=TXT_PRIMARY font_size=22.0 bold=true>"sineltor"</label>
                            </vstack>
                            <label text_color=TXT_SECONDARY font_size=11.0 bold=true>"Show all"</label>
                        </hstack>
                        <hstack gap=14.0>
                            {daily_mixes.into_iter().map(|(subtitle, cover, num, label_text)| {
                                view! {
                                    <vstack
                                        flex_grow=1.0
                                        gap=10.0
                                        padding=12.0
                                        background_color=BG_PANEL
                                        corner_radius=8.0
                                    >
                                        <vstack
                                            width=Dim::AUTO
                                            height=140.0
                                        >
                                            {cover_block(140.0, cover, num)}
                                        </vstack>
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
                    </vstack>

                    // "Albums featuring songs you like"
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
                        <hstack gap=14.0>
                            {albums.into_iter().map(|(name, cover, letter)| {
                                view! {
                                    <vstack
                                        flex_grow=1.0
                                        gap=10.0
                                        padding=12.0
                                        background_color=BG_PANEL
                                        corner_radius=8.0
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
                                <label text_color=TXT_PRIMARY font_size=12.0 bold=true>"sineltor"</label>
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
                    <label text_color=ACCENT_GREEN font_size=18.0 padding=8.0>
                        "\u{2713}"
                    </label>
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
                        <label text_color=TXT_SECONDARY font_size=14.0>"\u{1F500}"</label>
                        <label text_color=TXT_SECONDARY font_size=14.0>"\u{23EE}"</label>
                        <button
                            width=36.0
                            height=36.0
                            corner_radius=18.0
                            bordered=false
                            background_color=Color::WHITE
                            text_color=Color::BLACK
                            font_size=14.0
                            bold=true
                        >"\u{25B6}"</button>
                        <label text_color=TXT_SECONDARY font_size=14.0>"\u{23ED}"</label>
                        <label text_color=TXT_SECONDARY font_size=14.0>"\u{1F501}"</label>
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
                    <label text_color=TXT_SECONDARY font_size=14.0>"\u{1F3A4}"</label>
                    <label text_color=TXT_SECONDARY font_size=14.0>"\u{1F50A}"</label>
                    <slider
                        bind:value=volume
                        min_value=0.0
                        max_value=1.0
                        width=Dim::px(100.0)
                    />
                    <label text_color=TXT_SECONDARY font_size=14.0>"\u{2922}"</label>
                </hstack>
            </hstack>
        }
    }

    pub fn main() {
        mount_to_window("Spotify", (1100.0, 760.0), || {
            view! { <App /> }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
