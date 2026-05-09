//! Shared test helpers for `leptos_gtk`. Mirror of
//! `cocoa/leptos_cocoa/tests/common/mod.rs`.

#![cfg(feature = "gtk")]
#![allow(dead_code)]

pub fn ensure_gtk_init() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = gtk4::init();
    });
}

pub fn is_headless() -> bool {
    !gtk4::is_initialized()
}

pub fn run_tests(tests: &[(&'static str, fn())]) {
    ensure_gtk_init();

    if is_headless() {
        println!(
            "\nrunning 0 tests\n\n\
             test result: ok. 0 passed; 0 failed; {} ignored \
             (no GTK display available — tests skipped)",
            tests.len()
        );
        return;
    }

    let total = tests.len();
    println!("\nrunning {} tests", total);

    let mut passed = 0usize;
    let mut failed: Vec<(String, String)> = Vec::new();

    for (name, body) in tests {
        let result = std::panic::catch_unwind(
            std::panic::AssertUnwindSafe(*body),
        );
        match result {
            Ok(()) => {
                println!("test {} ... ok", name);
                passed += 1;
            }
            Err(payload) => {
                let msg = downcast_panic(&payload);
                println!("test {} ... FAILED", name);
                failed.push((name.to_string(), msg));
            }
        }
    }

    println!();
    if failed.is_empty() {
        println!(
            "test result: ok. {} passed; 0 failed; 0 ignored",
            passed
        );
    } else {
        println!("failures:");
        for (name, msg) in &failed {
            println!();
            println!("---- {} stdout ----", name);
            println!("{}", msg);
        }
        println!();
        println!(
            "test result: FAILED. {} passed; {} failed; 0 ignored",
            passed,
            failed.len()
        );
        std::process::exit(1);
    }
}

fn downcast_panic(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}
