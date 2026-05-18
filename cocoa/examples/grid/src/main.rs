//! Grid — demonstrates the new `<grid>` container backed by Taffy's
//! CSS-Grid algorithm. Layout: dashboard with a full-width header,
//! sidebar spanning two rows, main content, right rail, and a footer
//! that spans the two right-most columns.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;

    pub fn main() {
        mount_to_window("Grid", (640.0, 480.0), || {
            let counter = RwSignal::new(0);
            view! {
                <grid
                    columns=vec![fr(1.0), fr(3.0), fr(1.0)]
                    rows=vec![length(56.0), fr(1.0), length(40.0)]
                    gap=12.0
                    padding=12.0
                >
                    <vstack
                        background_color=Color::rgb(0.20, 0.30, 0.45)
                        padding=8.0
                        grid_row=(1, 2)
                        grid_column=(1, -1)
                    >
                        <label text_color=Color::WHITE font_size=18.0>
                            "Dashboard — Grid demo"
                        </label>
                    </vstack>

                    <vstack
                        background_color=Color::rgb(0.85, 0.85, 0.90)
                        padding=8.0
                        gap=4.0
                        grid_row=(2, 4)
                        grid_column_at=1
                    >
                        <label>"Sidebar"</label>
                        <label>"(rows 2–3, col 1)"</label>
                    </vstack>

                    <vstack
                        background_color=Color::rgb(0.95, 0.95, 0.97)
                        padding=8.0
                        gap=8.0
                    >
                        <label font_size=14.0>"Main content area"</label>
                        <label>"Resize the window — fr-weighted columns reflow."</label>
                        <hstack gap=8.0>
                            <button on:click=move |_| counter.update(|n| *n += 1)>
                                "+1"
                            </button>
                            <label>{move || format!("Clicks: {}", counter.get())}</label>
                        </hstack>
                    </vstack>

                    <vstack
                        background_color=Color::rgb(0.90, 0.92, 0.85)
                        padding=8.0
                    >
                        <label>"Right rail"</label>
                    </vstack>

                    <vstack
                        background_color=Color::rgb(0.30, 0.30, 0.30)
                        padding=8.0
                        grid_row_at=3
                        grid_column=(2, -1)
                    >
                        <label text_color=Color::WHITE>"Footer (cols 2–3)"</label>
                    </vstack>
                </grid>
            }
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
