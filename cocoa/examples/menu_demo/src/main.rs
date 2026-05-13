//! `menu_demo` — exercise the new `<menu_bar>` / `<menu>` /
//! `<menu_item>` builders. Shows:
//!
//! - A top-level `<menu_bar>` sibling to `<window>` in `run()`.
//! - Static + reactive titles (`title=move || …`).
//! - Reactive `enabled=` and `checked=` (the check-mark column).
//! - Keyboard shortcuts with the default ⌘ modifier and with an
//!   explicit `Modifiers::CMD_SHIFT` override.
//! - A `<menu_separator/>` between groups of related items.
//! - A nested `<menu>` ("Open Recent") populated from a static
//!   `Vec<MenuItem>` (dynamic, signal-driven repopulation of a
//!   menu is a future enhancement — see the plan doc).

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;
    // The `view!{}` macro emits `event::action` for `on:action=…`. The
    // macro path resolves to `::leptos::tachys::html::event::action`,
    // not to `event::action` in scope, so this `use` is only needed
    // when constructing builders directly (see `recent_items` below).
    use leptos::tachys::html::event;

    pub fn main() {
        run(|| {
            let count = RwSignal::new(0);
            let detail_shown = RwSignal::new(true);

            // A static set of "recent files" for the nested submenu.
            // Reactive menu reconciliation isn't wired yet — see the
            // module docstring.
            let recent_titles = ["Untitled-1.txt", "notes.md", "budget.xlsx"];
            let recent_items: Vec<_> = recent_titles
                .iter()
                .map(|&name| {
                    menu_item()
                        .title(name)
                        .on(event::action, move |_| {
                            println!("(stub) opening: {}", name);
                        })
                })
                .collect();

            view! {
                <menu_bar>
                    // The first menu in the bar is the "App menu"
                    // by AppKit convention. Its title is normally
                    // auto-replaced with the running process's name
                    // by the system, but for clarity we set it to
                    // the binary name here. Quit must live here for
                    // ⌘Q to do the right thing — `quit()` runs the
                    // normal AppKit terminate sequence.
                    <menu title="menu_demo">
                        <menu_item
                            title="Quit menu_demo"
                            shortcut="q"
                            on:action=move |_| quit()
                        />
                    </menu>
                    <menu title="File">
                        <menu_item
                            title="New"
                            shortcut="n"
                            on:action=move |_| count.update(|n| *n += 1)
                        />
                        <menu_item
                            title="Reset"
                            shortcut="r"
                            modifiers=Modifiers::CMD_SHIFT
                            on:action=move |_| count.set(0)
                        />
                        <menu_separator/>
                        <menu title="Open Recent">
                            {recent_items}
                        </menu>
                    </menu>
                    <menu title="View">
                        <menu_item
                            title="Show Detail"
                            shortcut="d"
                            checked=move || detail_shown.get()
                            on:action=move |_| {
                                detail_shown.update(|b| *b = !*b);
                            }
                        />
                        <menu_item
                            title=move || if detail_shown.get() {
                                String::from("Hide Detail (live title)")
                            } else {
                                String::from("(detail is hidden)")
                            }
                            enabled=move || detail_shown.get()
                            on:action=move |_| {
                                println!("clicked the reactive item");
                            }
                        />
                    </menu>
                    <menu title="Help">
                        <menu_item
                            title="About menu_demo"
                            on:action=move |_| {
                                println!("about box would open here");
                            }
                        />
                    </menu>
                </menu_bar>

                <window title="menu_demo" size=(420.0, 240.0)>
                    <vstack padding=20.0 gap=12.0>
                        <label>
                            {move || format!("Count: {}", count.get())}
                        </label>
                        <label>
                            {move || if detail_shown.get() {
                                "Detail is shown — toggle from View menu".to_string()
                            } else {
                                "Detail is hidden — toggle from View menu".to_string()
                            }}
                        </label>
                        <hstack gap=8.0>
                            <button on:click=move |_| count.update(|n| *n += 1)>
                                "+1"
                            </button>
                            <button on:click=move |_| count.set(0)>
                                "Reset"
                            </button>
                        </hstack>
                    </vstack>
                </window>
            }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
