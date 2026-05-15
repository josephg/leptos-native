//! `ErrorGuard` — RAII wrapper around `throw_error::ErrorId`.
//!
//! Used by `<label>.try_text()` (and any future builder method
//! that wants Result-returning closures) so that errors registered
//! with the nearest `<ErrorBoundary>` get cleared automatically
//! when the holding state drops — preventing stale errors from
//! lingering in the boundary after a re-render or unmount.

/// Holds a `throw_error::ErrorId` and calls `clear` on drop.
pub struct ErrorGuard(pub throw_error::ErrorId);

impl Drop for ErrorGuard {
    fn drop(&mut self) {
        throw_error::clear(&self.0);
    }
}
