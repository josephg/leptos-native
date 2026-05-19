//! `menu_demo_gtk` — GTK4 sibling of `menu_demo_cocoa`. Same surface
//! (`<menu_bar>` / `<menu>` / `<menu_item>` / `<menu_separator/>`),
//! same reactive / static patterns, different underlying model
//! (`gio::Menu` + `gio::SimpleAction` instead of NSMenu/NSMenuItem).
//!
//! The desktop shell decides where the menu bar is rendered — on
//! traditional GNOME this is a "Hamburger" menu, on Cinnamon /
//! XFCE it's a classic title-bar menu, on macOS-style overlays it
//! goes to the top-screen menu bar via the AppMenu extension.

mod app {
    use leptos_native::prelude::*;
    // The `view!{}` macro emits `event::action` for `on:action=…`.
    // Only needed when constructing items in Rust outside the macro.
    use leptos_native::tachys::html::event;

    pub fn main() {
        run("org.leptos.menu_demo_gtk", |app| {
            let count = RwSignal::new(0);
            let detail_shown = RwSignal::new(true);

            // Static list of "recent files" — dynamic, signal-driven
            // submenu repopulation is a v2 enhancement.
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

            let bar = view! {
                <menu_bar>
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
            };

            // The window builder needs the gtk::Application
            // explicitly; pair it with the menu bar as siblings in
            // the run() tuple.
            let win = window()
                .application(app.clone())
                .title("menu_demo")
                .size(420, 240)
                .child(view! {
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
                });

            (bar, win)
        });
    }
}

fn main() { app::main() }
