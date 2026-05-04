fn main() {
    println!("cargo:rustc-check-cfg=cfg(leptos_native)");

    // `leptos_native`: opt-in via the `native-ui` Cargo feature.
    // This crate is web-only (an SSR integration helper) — its
    // lib.rs is gated on `cfg(not(leptos_native))`, so when
    // `native-ui` is enabled the crate compiles to an empty rlib.
    // See ../../tachys/build.rs for rationale.
    if std::env::var_os("CARGO_FEATURE_NATIVE_UI").is_some() {
        println!("cargo:rustc-cfg=leptos_native");
    }
}
