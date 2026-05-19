//! ipify — fetch and display the user's public IP. iOS mirror of
//! `cocoa/examples/ipify`. Same bridging pattern: `tokio::spawn`
//! runs the HTTP on a tokio worker, the framework's main-thread
//! spawner polls the `JoinHandle`. Threading discipline (main
//! thread owns UIKit, tokio owns its own threads, never the twain
//! shall touch the run loop) is identical to the cocoa version —
//! libdispatch's main queue is the same primitive on both.

#[cfg(target_os = "ios")]
mod app {
    use leptos_native::prelude::*;
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
            let body: IpResponse = resp.json().await.map_err(|e| e.to_string())?;
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
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let _guard = rt.enter();
        leptos_native::mount_ios::run(|| view! { <App /> });
    }
}

#[cfg(target_os = "ios")]
fn main() { app::main() }

#[cfg(not(target_os = "ios"))]
fn main() {}
