//! `Date` — the value type for `<date_picker>` `bind:value=…`.
//!
//! A thin wrapper around `NSTimeInterval` (seconds since the Unix
//! epoch as `f64`). Send + Sync + Copy + 'static, so it threads
//! through the reactive system without ceremony. Conversion to
//! richer date types (chrono::DateTime, time::OffsetDateTime,
//! std::time::SystemTime) is up to the consumer — we deliberately
//! don't pull a calendar dep here.

use objc2::rc::Retained;
use objc2_foundation::NSDate;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Date {
    /// Seconds since 1970-01-01 UTC. Floating-point allows
    /// sub-second precision (NSTimeInterval is f64) and pre-epoch
    /// dates (negative values), matching `NSDate`'s semantics.
    pub seconds_since_epoch: f64,
}

impl Date {
    /// "Now" as reported by AppKit's `[NSDate date]`.
    pub fn now() -> Self {
        let d = NSDate::now();
        Self::from_nsdate(&d)
    }

    /// Build a `Date` from a Unix timestamp.
    pub const fn from_unix_secs(secs: f64) -> Self {
        Self { seconds_since_epoch: secs }
    }

    /// The Unix timestamp this Date represents.
    pub const fn unix_secs(self) -> f64 {
        self.seconds_since_epoch
    }

    /// Construct a `Retained<NSDate>` for AppKit calls.
    pub fn to_nsdate(self) -> Retained<NSDate> {
        NSDate::dateWithTimeIntervalSince1970(self.seconds_since_epoch)
    }

    /// Read seconds-since-epoch off an NSDate.
    pub fn from_nsdate(d: &NSDate) -> Self {
        Self {
            seconds_since_epoch: d.timeIntervalSince1970(),
        }
    }
}

impl Default for Date {
    fn default() -> Self {
        Self::now()
    }
}
