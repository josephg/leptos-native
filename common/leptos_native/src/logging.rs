//! Utilities for simple logging. Native ports always log to stdout/stderr
//! (the wasm `web_sys::console` path that lived here is gone).

/// Uses `println!()`-style formatting to log something to stdout.
#[macro_export]
macro_rules! log {
    ($($t:tt)*) => ($crate::logging::console_log(&format_args!($($t)*).to_string()))
}

/// Uses `println!()`-style formatting to log warnings to stderr.
#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => ($crate::logging::console_warn(&format_args!($($t)*).to_string()))
}

/// Uses `println!()`-style formatting to log errors to stderr.
#[macro_export]
macro_rules! error {
    ($($t:tt)*) => ($crate::logging::console_error(&format_args!($($t)*).to_string()))
}

/// Like `warn!`, but only in debug builds.
#[macro_export]
macro_rules! debug_warn {
    ($($x:tt)*) => {
        {
            #[cfg(debug_assertions)]
            {
                $crate::warn!($($x)*)
            }
            #[cfg(not(debug_assertions))]
            {
                ($($x)*)
            }
        }
    }
}

/// Log a string to stdout.
pub fn console_log(s: &str) {
    #[allow(clippy::print_stdout)]
    {
        println!("{s}");
    }
}

/// Log a warning to stderr.
pub fn console_warn(s: &str) {
    eprintln!("{s}");
}

/// Log an error to stderr.
#[inline(always)]
pub fn console_error(s: &str) {
    eprintln!("{s}");
}

/// Log a warning to stderr, but only in a debug build.
#[inline(always)]
pub fn console_debug_warn(s: &str) {
    #[cfg(debug_assertions)]
    {
        eprintln!("{s}");
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = s;
    }
}
