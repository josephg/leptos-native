//! ipify — GTK mirror of `cocoa/examples/ipify`. Same bridging
//! pattern: tokio on its own threads, JoinHandle awaited on main.
//!
//! On GTK the "main thread" is the thread that owns
//! `glib::MainContext::default()` — i.e. the one running
//! `gtk::Application::run`. The framework's executor lives there
//! (via `any_spawner::init_glib`), so awaiting a tokio JoinHandle
//! inside an `AsyncDerived` works exactly like the cocoa version.

extern crate leptos_gtk as leptos_platform;

mod app {
    use leptos_platform::prelude::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize)]
    struct IpResponse {
        ip: String,
    }

    async fn fetch_ip() -> Result<String, String> {
        let handle = tokio::spawn(async {
            let resp = reqwest::get("https://api.ipify.org?format=json")
                .await
                .map_err(|e| e.to_string())?;
            let body: IpResponse =
                resp.json().await.map_err(|e| e.to_string())?;
            Ok::<_, String>(body.ip)
        });
        handle.await.map_err(|e| e.to_string())?
    }

    #[component]
    fn App() -> impl IntoView {
        let refresh = RwSignal::new(0u32);
        let ip = AsyncDerived::new(move || {
            let _ = refresh.get();
            async { fetch_ip().await }
        });

        view! {
            <vstack padding=20.0 gap=12.0>
                <label>"Your public IP"</label>
                <label>{move || match ip.get() {
                    Some(Ok(ip))  => ip,
                    Some(Err(e))  => format!("Error: {e}"),
                    None          => "Fetching…".to_string(),
                }}</label>
                <button on:click=move |_| refresh.update(|n| *n += 1)>
                    "Refresh"
                </button>
            </vstack>
        }
    }

    pub fn main() {
        // User owns the runtime, same as cocoa. `_guard = rt.enter()`
        // must live for the program's life so tokio::spawn from inside
        // AsyncDerived's main-thread future resolves to this runtime.
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let _guard = rt.enter();

        mount_to_window(
            "org.leptos.ipify_gtk",
            "ipify",
            (340, 160),
            || view! { <App /> },
        )
        .run();
    }
}

fn main() { app::main() }
