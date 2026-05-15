//! `toolbar_demo` — exercise the native `<toolbar>` / `<toolbar_item>`
//! API surface. Walks through every supported feature in one window.
//!
//! Generic items:
//! - **Static items** (`identifier`, `label`, `icon`, `on:action`).
//! - **Reactive label & icon** — re-binding a signal updates the
//!   item in place.
//! - **Reactive `enabled`** — gate items behind app state.
//! - **`tool_tip`** — hover text.
//! - **`bordered` / `navigational`** — modern macOS chrome variants.
//!
//! Specialised items:
//! - **`<toolbar_search_item>`** — `NSSearchToolbarItem` +
//!   `NSSearchField`, with native magnifying-glass / clear chrome.
//!   Exercises `bind:value`, `preferred_width`, `on:input`, `on:action`.
//! - **`<toolbar_toggle_sidebar/>`** — system sidebar-toggle, fires
//!   `toggleSidebar:` against the `NSSplitViewController` we get from
//!   [`mount_to_split_window`].
//! - **`<toolbar_sidebar_tracking_separator/>`** — vertical separator
//!   that auto-aligns with the sidebar pane's divider.
//! - **`<toolbar_print/>`** — system print item; fires
//!   `printDocument:` up the responder chain.
//! - **`<toolbar_flexible_space/>` / `<toolbar_space/>`** — built-in
//!   spacers.
//!
//! Toolbar-level reactivity:
//! - **`display_mode`** — switch between `Default` / `IconAndLabel` /
//!   `IconOnly` / `LabelOnly` at runtime.
//! - **`visible`** — hide/show the entire toolbar band.
//!
//! Dynamic structure:
//! - **`ToolbarHandle`** — imperatively insert / remove items after
//!   build (the `ToolbarMountable` cascade is static-at-build, so
//!   `<Show>` / `<For>` over items don't work — `ToolbarHandle` is
//!   the supported escape hatch).

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;

    pub fn main() {
        mount_to_split_window("Toolbar Demo", (900.0, 560.0), || {
            // ---- App state driving toolbar reactivity ------------------
            let count = RwSignal::new(0);
            let starred = RwSignal::new(false);
            // Display-mode index drives a `<segmented_control>`; we
            // map it to the toolbar's `ToolbarDisplayMode` on read.
            let display_mode_idx = RwSignal::new(0_usize);
            let display_mode = move || match display_mode_idx.get() {
                0 => ToolbarDisplayMode::Default,
                1 => ToolbarDisplayMode::IconAndLabel,
                2 => ToolbarDisplayMode::IconOnly,
                _ => ToolbarDisplayMode::LabelOnly,
            };
            let toolbar_visible = RwSignal::new(true);

            // Two-way search binding — typing in the toolbar's search
            // field updates this signal; setting it from a button
            // pushes the new value back into the field.
            let query = RwSignal::new(String::new());
            let last_committed = RwSignal::new(String::new());

            // A handle for the dynamic-items demo at the bottom of
            // the toolbar. The handle is `Copy`, so closures and
            // `view!` bindings can freely capture it.
            let handle = ToolbarHandle::new();
            let dynamic_count = RwSignal::new(0u32);

            view! {
                <split_view vertical=true>
                    // ---- Sidebar pane ---------------------------------
                    // Collapsible via the toolbar's `<toolbar_toggle_sidebar/>`.
                    <split_pane
                        behavior=PaneBehavior::Sidebar
                        preferred_thickness=220.0
                        minimum_thickness=180.0
                        maximum_thickness=320.0
                        can_collapse=true
                        // Preview-style: keep the window fixed and
                        // let the main pane absorb the freed/needed
                        // space. AppKit's default for sidebar items
                        // is `PreferResizingSplitViewWithFixedSiblings`,
                        // which grows / shrinks the window on every
                        // toggle.
                        collapse_behavior=CollapseBehavior::PreferResizingSiblingsWithFixedSplitView
                    >
                        <vstack padding=16.0 gap=10.0>
                            <label font_size=13.0 bold=true>"Demo notes"</label>
                            <label font_size=11.0 multiline=true>
                                "Toggle this sidebar via the sidebar icon in the \
                                 toolbar. The separator next to it auto-aligns \
                                 with the divider — try resizing the sidebar."
                            </label>
                            <label font_size=11.0 multiline=true>
                                "Type in the toolbar search field to see the \
                                 query signal update live below. Press ⏎ to \
                                 commit (on:action)."
                            </label>
                            <label font_size=11.0 multiline=true>
                                "The Print item fires printDocument: up the \
                                 responder chain — try ⌘P or click it."
                            </label>
                        </vstack>
                    </split_pane>

                    // ---- Main pane: the toolbar + all controls --------
                    <split_pane holding_priority=199.0>
                        <toolbar
                            identifier="toolbar_demo.main"
                            display_mode=display_mode
                            visible=move || toolbar_visible.get()
                            handle=handle
                        >
                            // Sidebar toggle + tracking separator.
                            // Together they make the toolbar feel
                            // native in a split-view app.
                            <toolbar_toggle_sidebar/>
                            <toolbar_sidebar_tracking_separator/>

                            // Navigational pair (back/forward style).
                            // Note: identifiers are optional —
                            // auto-generated unless the item needs
                            // a stable id for `ToolbarHandle`
                            // operations (see the dynamic-items
                            // section below).
                            <toolbar_item
                                label="Back"
                                icon=Icon::sf_symbol("chevron.left")
                                tool_tip="Go back (decrement counter)"
                                navigational=true
                                bordered=true
                                on:action=move |_| count.update(|n| *n -= 1)
                            />
                            <toolbar_item
                                label="Forward"
                                icon=Icon::sf_symbol("chevron.right")
                                tool_tip="Go forward (increment counter)"
                                navigational=true
                                bordered=true
                                on:action=move |_| count.update(|n| *n += 1)
                            />

                            <toolbar_space/>

                            // Reactive label.
                            <toolbar_item
                                label=move || format!("Count: {}", count.get())
                                icon=Icon::sf_symbol("number")
                                tool_tip="Reactive label — updates with the count signal"
                                on:action=move |_| {
                                    println!("count action: {}", count.get_untracked());
                                }
                            />

                            // Reactive sf_symbol + reactive enabled.
                            <toolbar_item
                                label="Star"
                                icon=move || if starred.get() {
                                    Icon::sf_symbol("star.fill")
                                } else {
                                    Icon::sf_symbol("star")
                                }
                                enabled=move || { count.get() > 0 }
                                tool_tip="Toggle starred (disabled when count is 0)"
                                on:action=move |_| {
                                    starred.update(|b| *b = !*b);
                                }
                            />

                            <toolbar_flexible_space/>

                            // Native search field — proper macOS
                            // chrome (magnifying glass, clear ×,
                            // recent searches). bind:value is
                            // two-way; on:action fires on ⏎.
                            <toolbar_search_item
                                label="Search"
                                tool_tip="Search (bind:value + on:action)"
                                placeholder="Type a query…"
                                // `preferred_width` is the *focused*
                                // width per Apple docs — without a
                                // separate `width` pin the field
                                // shrinks back whenever it loses
                                // focus (every click on another
                                // toolbar item, including the
                                // sidebar toggle).
                                preferred_width=240.0
                                width=240.0
                                bind:value=query
                                on:action=move |_| {
                                    last_committed.set(query.get_untracked());
                                }
                            />

                            // Bordered share button.
                            <toolbar_item
                                label="Share"
                                icon=Icon::sf_symbol("square.and.arrow.up")
                                bordered=true
                                tool_tip="Bordered toolbar item"
                                on:action=move |_| {
                                    println!("share clicked");
                                }
                            />

                            // Standard print item.
                            <toolbar_print/>
                        </toolbar>

                        // ---- Window content: controls for everything ----
                        <vstack padding=20.0 gap=14.0 flex_grow=1.0>
                            <label font_size=18.0 bold=true>
                                {move || format!(
                                    "Count: {}   |   Starred: {}",
                                    count.get(),
                                    if starred.get() { "yes" } else { "no" },
                                )}
                            </label>

                            // Live readout of the toolbar search field.
                            <vstack gap=4.0>
                                <label font_size=13.0>
                                    {move || format!(
                                        "Live query: {:?}",
                                        query.get(),
                                    )}
                                </label>
                                <label font_size=13.0>
                                    {move || format!(
                                        "Last committed (⏎): {:?}",
                                        last_committed.get(),
                                    )}
                                </label>
                                <hstack gap=8.0>
                                    <button on:click=move |_| {
                                        query.set("hello from the button".to_string())
                                    }>
                                        "Push value → field"
                                    </button>
                                    <button on:click=move |_| query.set(String::new())>
                                        "Clear"
                                    </button>
                                </hstack>
                            </vstack>

                            <hstack gap=10.0>
                                <button on:click=move |_| count.update(|n| *n -= 1)>
                                    "−1"
                                </button>
                                <button on:click=move |_| count.update(|n| *n += 1)>
                                    "+1"
                                </button>
                                <button on:click=move |_| count.set(0)>
                                    "Reset"
                                </button>
                            </hstack>

                            // Display-mode picker — native segmented
                            // control bound to a usize index signal.
                            <vstack gap=6.0>
                                <label font_size=13.0>"Toolbar display mode:"</label>
                                <segmented_control
                                    items=vec![
                                        "Default",
                                        "Icon + Label",
                                        "Icon only",
                                        "Label only",
                                    ]
                                    bind:value=display_mode_idx
                                />
                            </vstack>

                            // Visibility toggle — native checkbox,
                            // bind:checked two-way wires through to
                            // the toolbar's reactive `visible` attr.
                            <checkbox bind:checked=toolbar_visible>
                                "Toolbar visible"
                            </checkbox>

                            // Dynamic items via ToolbarHandle.
                            <vstack gap=6.0>
                                <label font_size=13.0>
                                    "Dynamic items (inserted via ToolbarHandle, \
                                     not through the macro):"
                                </label>
                                <hstack gap=8.0>
                                    <button on:click=move |_| {
                                        dynamic_count.update(|n| *n += 1);
                                        let n = dynamic_count.get_untracked();
                                        let id = format!("dynamic-{n}");
                                        // Insert just before the
                                        // trailing flexible_space
                                        // (after toggle_sidebar +
                                        // separator + back + forward
                                        // + space + count + star = 7).
                                        handle.insert_item(
                                            toolbar_item()
                                                .identifier(id.clone())
                                                .label(format!("Item {n}"))
                                                .icon(Icon::sf_symbol("circle.fill"))
                                                .tool_tip("Inserted at runtime")
                                                .on(
                                                    leptos::tachys::html::event::action,
                                                    move |_| {
                                                        println!("dynamic {id} clicked");
                                                    },
                                                ),
                                            7,
                                        );
                                    }>
                                        "Add dynamic item"
                                    </button>
                                    <button on:click=move |_| {
                                        let n = dynamic_count.get_untracked();
                                        if n > 0 {
                                            let id = format!("dynamic-{n}");
                                            if handle.remove_item(&id) {
                                                dynamic_count.update(|n| *n -= 1);
                                            }
                                        }
                                    }>
                                        "Remove last dynamic"
                                    </button>
                                    <label>
                                        {move || format!(
                                            "Dynamic items currently in toolbar: {}",
                                            dynamic_count.get(),
                                        )}
                                    </label>
                                </hstack>
                            </vstack>
                        </vstack>
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
