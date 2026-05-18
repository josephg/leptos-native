//! placecats — fetch and display a random cat image.
//!
//! Demonstrates **pattern 1** with a binary payload: the response
//! body is bytes (a JPEG/PNG), not text. The bytes flow into
//! `<image_view bytes=…>`, which calls `NSImage::initWithData:`
//! under the hood.
//!
//! Threading is identical to `ipify`: `tokio::spawn` runs the HTTP
//! on a worker, the main thread awaits the `JoinHandle`. Each
//! refresh gets a different cat — placecats.com randomises per
//! request.
//!
//! Note: window-size-driven re-fetch (so the cat resizes with the
//! window) would need a window-size signal in the framework, which
//! doesn't exist yet. This example uses a fixed 480×320 image.

use leptos::prelude::*;

const W: u32 = 480;
const H: u32 = 320;

async fn fetch_cat() -> Result<Vec<u8>, String> {
    let handle = tokio::spawn(async {
        let url = format!("https://placecats.com/{W}/{H}");
        let resp = reqwest::get(&url)
            .await
            .map_err(|e| e.to_string())?;
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        Ok::<_, String>(bytes.to_vec())
    });
    handle.await.map_err(|e| e.to_string())?
}

#[component]
fn App() -> impl IntoView {
    let refresh = RwSignal::new(0u32);
    let img = AsyncDerived::new(move || {
        let _ = refresh.get();
        async { fetch_cat().await }
    });

    // Pull the bytes out as `Option<Vec<u8>>` so `<image_view
    // bytes=…>` can clear the view on loading / error.
    let bytes = move || match img.get() {
        Some(Ok(b)) => Some(b),
        _ => None,
    };
    let status = move || match img.get() {
        Some(Ok(_)) => String::new(),
        Some(Err(e)) => format!("Error: {e}"),
        None => "Fetching…".to_string(),
    };

    view! {
        <vstack padding=16.0 gap=8.0>
            <label bold=true>"A cat (480 × 320)"</label>
            <image_view bytes=bytes width=W as f32 height=H as f32 />
            <label>{status}</label>
            <button on:click=move |_| refresh.update(|n| *n += 1)>
                "Another cat"
            </button>
        </vstack>
    }
}

fn main() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let _guard = rt.enter();

    mount_to_window("placecats", (520.0, 420.0), || view! { <App /> }).run();
}
