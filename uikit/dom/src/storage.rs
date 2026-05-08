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
//! ports to iOS with one substitution — `window().local_storage()` →
//! [`local_storage()`]. Same `Result<Option<Storage>, _>` →
//! `Storage::get_item(...) -> Result<Option<String>, _>` shape so
//! the chained `.ok().flatten()` calls work unchanged.
//!
//! Underneath, [`Storage`] holds a `Retained<NSUserDefaults>`. Each
//! mutation is synchronous and writes through to disk lazily via
//! the system's internal scheduler.
//!
//! # Threading
//!
//! `NSUserDefaults` is documented thread-safe by Apple, but our
//! `Retained<NSUserDefaults>` is wrapped in `SendWrapper` to
//! match the rest of `ios_dom`'s main-thread contract.

use crate::MainThreadMarker;
use objc2::rc::Retained;
use objc2_foundation::{NSString, NSUserDefaults};
use send_wrapper::SendWrapper;

#[derive(Clone)]
pub struct Storage {
    defaults: SendWrapper<Retained<NSUserDefaults>>,
}

impl Storage {
    pub fn get_item(&self, key: &str) -> Result<Option<String>, StorageError> {
        let key = NSString::from_str(key);
        let value = self.defaults.stringForKey(&key);
        Ok(value.map(|s| s.to_string()))
    }

    pub fn set_item(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), StorageError> {
        let key = NSString::from_str(key);
        let value = NSString::from_str(value);
        let value_obj: &objc2::runtime::AnyObject = value.as_ref();
        unsafe {
            self.defaults.setObject_forKey(Some(value_obj), &key);
        }
        Ok(())
    }

    pub fn remove_item(&self, key: &str) -> Result<(), StorageError> {
        let key = NSString::from_str(key);
        self.defaults.removeObjectForKey(&key);
        Ok(())
    }

    pub fn synchronize(&self) {
        unsafe {
            let _: bool = objc2::msg_send![&**self.defaults, synchronize];
        }
    }
}

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
                "ios_dom::storage must be accessed on the main thread"
            ),
        }
    }
}

impl std::error::Error for StorageError {}
