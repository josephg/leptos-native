fn main() {
    println!("cargo:rustc-check-cfg=cfg(leptos_native)");

    // `leptos_native`: opt-in via the `native-ui` Cargo feature. See
    // tachys/build.rs for rationale.
    if std::env::var_os("CARGO_FEATURE_NATIVE_UI").is_some() {
        println!("cargo:rustc-cfg=leptos_native");
    }
}
