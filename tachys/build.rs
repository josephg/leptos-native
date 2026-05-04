use rustc_version::{version_meta, Channel};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(rustc_nightly)");
    println!("cargo:rustc-check-cfg=cfg(leptos_native)");

    if matches!(version_meta().unwrap().channel, Channel::Nightly) {
        println!("cargo:rustc-cfg=rustc_nightly");
    }

    // `leptos_native` is the umbrella for the native UI backends
    // (Cocoa on macOS, GTK on Linux). Source code uses
    // `cfg(leptos_native)` where the web/native distinction matters,
    // and the more specific `cfg(target_os = "macos")` /
    // `cfg(target_os = "linux")` where a particular backend is needed.
    //
    // Set when the `native-ui` Cargo feature is enabled. The choice
    // of backend (cocoa vs gtk) is then routed at source level by
    // `cfg(target_os = "macos")` / `cfg(target_os = "linux")` checks
    // inside `cfg(leptos_native)` regions.
    if std::env::var_os("CARGO_FEATURE_NATIVE_UI").is_some() {
        println!("cargo:rustc-cfg=leptos_native");
    }
}
