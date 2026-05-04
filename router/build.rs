use rustc_version::{version_meta, Channel};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(rustc_nightly)");
    println!("cargo:rustc-check-cfg=cfg(leptos_native)");

    // Set cfg flags depending on release channel
    if matches!(version_meta().unwrap().channel, Channel::Nightly) {
        println!("cargo:rustc-cfg=rustc_nightly");
    }

    // `leptos_native`: opt-in via the `native-ui` Cargo feature.
    // This crate is web-only — its lib.rs is gated on
    // `cfg(not(leptos_native))` so it compiles to an empty rlib when
    // `native-ui` is enabled (defensive — a native binary normally
    // shouldn't depend on leptos_router at all). See
    // ../tachys/build.rs for rationale.
    if std::env::var_os("CARGO_FEATURE_NATIVE_UI").is_some() {
        println!("cargo:rustc-cfg=leptos_native");
    }
}
