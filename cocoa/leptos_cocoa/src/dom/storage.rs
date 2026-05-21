//! Persistent key-value storage backed by `NSUserDefaults`.
//!
//! Mirrors the shape of `web_sys::Storage` (the API behind
//! `window.localStorage` on the web), so example code that does:
//!
//! ```ignore
//! window().local_storage()
//!     .ok().flatten()
//!     .and_then(|s| s.get_item(KEY).ok().flatten())
//! ```
//!
//! ports to macOS with one substitution — `window().local_storage()` →
//! [`local_storage()`]. Same `Result<Option<Storage>, _>` →
//! `Storage::get_item(...) -> Result<Option<String>, _>` shape so
//! the chained `.ok().flatten()` calls work unchanged.
//!
//! Underneath, [`Storage`] holds a `Retained<NSUserDefaults>`. Each
//! mutation is synchronous and writes through to disk lazily via
//! AppKit's internal scheduler — call [`Storage::synchronize`]
//! explicitly if you need an immediate flush (rare; usually only
//! before crash/exit).
//!
//! # Threading
//!
//! `NSUserDefaults` is documented thread-safe by Apple, but our
//! `Retained<NSUserDefaults>` is wrapped in `SendWrapper` to
//! match the rest of `cocoa_dom`'s main-thread contract. Using
//! `Storage` off the main thread will panic via the SendWrapper
//! guard.

use super::MainThreadMarker;
use objc2::rc::Retained;
use objc2_foundation::{NSString, NSUserDefaults};
use send_wrapper::SendWrapper;

/// Handle to the app's persistent key-value store.
///
/// Cheap to construct — `NSUserDefaults::standardUserDefaults` is
/// effectively a singleton lookup.
#[derive(Clone)]
pub struct Storage {
    defaults: SendWrapper<Retained<NSUserDefaults>>,
}

impl Storage {
    /// Read a string value. Returns `Ok(None)` if the key is
    /// absent or the stored value isn't a string. Web parity:
    /// `Result<Option<String>, _>`.
    pub fn get_item(&self, key: &str) -> Result<Option<String>, StorageError> {
        let key = NSString::from_str(key);
        let value = self.defaults.stringForKey(&key);
        Ok(value.map(|s| s.to_string()))
    }

    /// Write a string value.
    pub fn set_item(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), StorageError> {
        let key = NSString::from_str(key);
        let value = NSString::from_str(value);
        // NSString is an NSObject; cast for setObject:forKey:.
        let value_obj: &objc2::runtime::AnyObject = value.as_ref();
        unsafe {
            self.defaults.setObject_forKey(Some(value_obj), &key);
        }
        Ok(())
    }

    /// Remove a key.
    pub fn remove_item(&self, key: &str) -> Result<(), StorageError> {
        let key = NSString::from_str(key);
        self.defaults.removeObjectForKey(&key);
        Ok(())
    }

    /// Force an immediate write to disk. Rarely necessary —
    /// AppKit's internal scheduler flushes lazily — but useful
    /// before deliberate process exit. (Apple's docs note that
    /// this method is "no longer needed" for normal usage as of
    /// macOS 10.13; included for explicit-flush scenarios.)
    pub fn synchronize(&self) {
        unsafe {
            let _: bool = objc2::msg_send![&**self.defaults, synchronize];
        }
    }
}

/// Returns the standard user defaults wrapped as [`Storage`].
///
/// Returns `Err` only if not on the main thread. The `Result`-of-
/// `Option` shape matches `web_sys::window().local_storage()` so
/// the same `.ok().flatten()` chain in user code works unchanged.
pub fn local_storage() -> Result<Option<Storage>, StorageError> {
    let _mtm =
        MainThreadMarker::new().ok_or(StorageError::NotMainThread)?;
    Ok(Some(Storage {
        defaults: SendWrapper::new(NSUserDefaults::standardUserDefaults()),
    }))
}

#[derive(Debug)]
pub enum StorageError {
    NotMainThread,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::NotMainThread => write!(
                f,
                "cocoa_dom::storage must be accessed on the main thread"
            ),
        }
    }
}

impl std::error::Error for StorageError {}
