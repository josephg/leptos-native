//! Grid — GTK port. Dashboard layout with a full-width header,
//! sidebar spanning two rows, and footer spanning two columns —
//! same shape as the cocoa / iOS grid examples.

mod app {
    use leptos_native::prelude::*;

    pub fn main() {
        mount_to_window(
            "org.leptos.grid_gtk",
            "Grid",
            (640, 480),
            || {
                let counter = RwSignal::new(0);
                view! {
                    <grid
                        columns=vec![fr(1.0), fr(3.0), fr(1.0)]
                        rows=vec![length(56.0), fr(1.0), length(40.0)]
                        gap=12.0
                        padding=12.0
                    >
                        <vstack
                            grid_row=(1, 2)
                            grid_column=(1, -1)
                        >
                            <label>"Dashboard — Grid demo"</label>
                        </vstack>

                        <vstack
                            gap=4.0
                            grid_row=(2, 4)
                            grid_column_at=1
                        >
                            <label>"Sidebar"</label>
                            <label>"(rows 2–3, col 1)"</label>
                        </vstack>

                        <vstack gap=8.0>
                            <label>"Main content"</label>
                            <label>"Resize the window — fr columns reflow."</label>
                            <hstack gap=8.0>
                                <button on:click=move |_| counter.update(|n| *n += 1)>
                                    "+1"
                                </button>
                                <label>{move || format!("Clicks: {}", counter.get())}</label>
                            </hstack>
                        </vstack>

                        <vstack>
                            <label>"Right rail"</label>
                        </vstack>

                        <vstack
                            grid_row_at=3
                            grid_column=(2, -1)
                        >
                            <label>"Footer (cols 2–3)"</label>
                        </vstack>
                    </grid>
                }
            },
        );
    }
}

fn main() { app::main() }
