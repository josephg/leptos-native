//! Canvas demo — the `<canvas>` retained-scene drawing surface.
//!
//! Renders a small static scene (rects, ellipses, a polyline, text)
//! plus a marker that follows the last mouse-down, and a label that
//! reports the click coordinates. Demonstrates:
//!   * `scene=closure` — reactive `Vec<DrawCmd>` regeneration
//!   * `on:mouse_down` — `CanvasPoint` payload in canvas-local
//!     top-left-origin coordinates

extern crate leptos_cocoa as leptos_platform;

#[cfg(target_os = "macos")]
mod app {
    use leptos_platform::prelude::*;

    fn build_scene(click: Option<CanvasPoint>) -> Vec<DrawCmd> {
        let mut cmds = vec![
            DrawCmd::Text {
                x: 12.0,
                y: 10.0,
                text: "Click anywhere".to_string(),
                color: Color::LABEL,
                size: 14.0,
            },
            DrawCmd::FillRect {
                x: 12.0,
                y: 40.0,
                w: 80.0,
                h: 50.0,
                color: Color::rgba(0.2, 0.6, 1.0, 0.6),
            },
            DrawCmd::StrokeRect {
                x: 110.0,
                y: 40.0,
                w: 80.0,
                h: 50.0,
                color: Color::SYSTEM_PURPLE,
                width: 2.0,
                dashed: true,
            },
            DrawCmd::FillEllipse {
                x: 210.0,
                y: 40.0,
                w: 50.0,
                h: 50.0,
                color: Color::SYSTEM_GREEN,
            },
            DrawCmd::StrokeEllipse {
                x: 280.0,
                y: 40.0,
                w: 70.0,
                h: 50.0,
                color: Color::SYSTEM_ORANGE,
                width: 3.0,
                dashed: false,
            },
            DrawCmd::Polyline {
                points: vec![
                    (12.0, 130.0),
                    (60.0, 105.0),
                    (110.0, 140.0),
                    (170.0, 100.0),
                    (230.0, 135.0),
                ],
                color: Color::SYSTEM_RED,
                width: 4.0,
            },
        ];
        // Crosshair marker at the last mouse_down.
        if let Some(p) = click {
            cmds.push(DrawCmd::StrokeEllipse {
                x: p.x - 8.0,
                y: p.y - 8.0,
                w: 16.0,
                h: 16.0,
                color: Color::SYSTEM_BLUE,
                width: 2.0,
                dashed: false,
            });
        }
        cmds
    }

    #[component]
    pub fn App() -> impl IntoView {
        let last_down = RwSignal::new(None::<CanvasPoint>);

        view! {
            <vstack padding=12.0 gap=8.0 flex_grow=1.0>
                <label>
                    {move || match last_down.get() {
                        Some(p) => {
                            format!("last mouse_down: ({:.1}, {:.1})", p.x, p.y)
                        }
                        None => "last mouse_down: (none yet)".to_string(),
                    }}
                </label>
                <canvas
                    flex_grow=1.0
                    background_color=Color::rgb(1.0, 1.0, 1.0)
                    scene=move || build_scene(last_down.get())
                    on:mouse_down=move |p: CanvasPoint| last_down.set(Some(p))
                />
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Canvas demo", (480.0, 360.0), || view! { <App /> })
            .run();
    }
}

#[cfg(target_os = "macos")]
fn main() {
    app::main()
}

#[cfg(not(target_os = "macos"))]
fn main() {}
