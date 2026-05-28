//! Persisting settings to disk. The slider position, mute
//! checkbox, and theme dropdown round-trip through a JSON file
//! in `~/.config/leptos_persistent_settings/state.json`. Restart
//! the app to see the values come back.
//!
//! The pattern is general:
//!   1. Load initial state from disk (defaulting on failure).
//!   2. Drive an `Effect` that re-serialises and writes whenever
//!      any tracked signal changes.
//!   3. Persistence happens in the background; the UI is fully
//!      decoupled.
//!
//! For GNOME-integration storage you can swap the JSON round-trip
//! for `gio::Settings` — the *signal-to-storage* shape stays
//! identical, but `gio::Settings` requires a registered schema
//! installed under `/usr/share/glib-2.0/schemas/` (see
//! `glib-compile-schemas`).

extern crate leptos_gtk as leptos_platform;

use leptos_platform::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct State {
    volume: f64,
    mute:   bool,
    theme:  usize,
}

impl Default for State {
    fn default() -> Self {
        Self { volume: 50.0, mute: false, theme: 0 }
    }
}

fn state_path() -> PathBuf {
    let mut p = dirs_minimal::config_dir();
    p.push("leptos_persistent_settings");
    fs::create_dir_all(&p).ok();
    p.push("state.json");
    p
}

fn load() -> State {
    fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(state: &State) {
    if let Ok(s) = serde_json::to_string_pretty(state) {
        let _ = fs::write(state_path(), s);
    }
}

mod dirs_minimal {
    use std::path::PathBuf;
    pub fn config_dir() -> PathBuf {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut p = std::env::var_os("HOME").map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/tmp"));
                p.push(".config");
                p
            })
    }
}

#[component]
fn Settings() -> impl IntoView {
    let initial = load();
    let volume = RwSignal::new(initial.volume);
    let mute   = RwSignal::new(initial.mute);
    let theme  = RwSignal::new(initial.theme);

    Effect::new(move |_| {
        save(&State {
            volume: volume.get(),
            mute:   mute.get(),
            theme:  theme.get(),
        });
    });

    view! {
        <vstack padding=16.0 gap=12.0>
            <label>"Volume"</label>
            <slider
                bind:value=volume
                min_value=0.0
                max_value=100.0
                enabled=move || !mute.get() />
            <label>{move || {
                if mute.get() { "Muted".to_string() }
                else { format!("{:.0}%", volume.get()) }
            }}</label>

            <checkbox bind:checked=mute>"Mute audio"</checkbox>

            <hstack gap=8.0>
                <label>"Theme:"</label>
                <pop_up_button
                    items=vec!["System", "Light", "Dark"]
                    bind:value=theme />
            </hstack>

            <label>"Changes persist immediately. Restart to verify."</label>
        </vstack>
    }
}

fn main() {
    mount_to_window(
        "org.leptos.persistent_settings",
        "Persistent settings",
        (380, 360),
        || view! { <Settings /> },
    )
    .run();
}
