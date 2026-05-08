use rustc_version::{version_meta, Channel};

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();

    println!("cargo:rustc-check-cfg=cfg(rustc_nightly)");
    println!("cargo:rustc-check-cfg=cfg(leptos_native)");

    // Set cfg flags depending on release channel
    if matches!(version_meta().unwrap().channel, Channel::Nightly) {
        println!("cargo:rustc-cfg=rustc_nightly");
    }
    // Set cfg flag for getrandom wasm_js
    if target == "wasm32-unknown-unknown" {
        // Set a custom cfg flag for wasm builds
        println!("cargo:rustc-cfg=getrandom_backend=\"wasm_js\"");
    }
    // `leptos_native`: opt-in via the `native-ui` Cargo feature. See
    // tachys/build.rs for rationale.
    if std::env::var_os("CARGO_FEATURE_NATIVE_UI").is_some() {
        println!("cargo:rustc-cfg=leptos_native");
    }
}
