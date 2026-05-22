//! Shared `WindowSize` and `WindowPosition` newtypes used by the
//! per-port window builders.
//!
//! Each value is stored as `(f64, f64)` for precision; the
//! `From<(i32, i32)>` impl widens an integer pair so user code
//! can write `(640, 480)` or `(640.0, 480.0)` interchangeably.
//! Ports that only need integer pixels (GTK) convert back with
//! `.as_i32_tuple()`.

/// Window content-area size. Stored in points (Cocoa / iOS) or
/// pixels (GTK) depending on the port; users don't usually need
/// to care because window-relative positioning works in the
/// same units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSize(pub f64, pub f64);

impl WindowSize {
    /// Construct from explicit width and height.
    pub const fn new(width: f64, height: f64) -> Self {
        Self(width, height)
    }

    /// Width.
    pub fn width(self) -> f64 { self.0 }
    /// Height.
    pub fn height(self) -> f64 { self.1 }

    /// Convert to an `(i32, i32)` tuple, rounded toward zero.
    /// Used by GTK's mount entry points.
    pub fn as_i32_tuple(self) -> (i32, i32) {
        (self.0 as i32, self.1 as i32)
    }
}

impl From<(f64, f64)> for WindowSize {
    fn from((w, h): (f64, f64)) -> Self { Self(w, h) }
}

impl From<(i32, i32)> for WindowSize {
    fn from((w, h): (i32, i32)) -> Self {
        Self(w as f64, h as f64)
    }
}

impl From<(u32, u32)> for WindowSize {
    fn from((w, h): (u32, u32)) -> Self {
        Self(w as f64, h as f64)
    }
}

/// Window screen position. Origin convention is port-specific
/// (bottom-left on AppKit, top-left on GTK).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowPosition(pub f64, pub f64);

impl WindowPosition {
    pub const fn new(x: f64, y: f64) -> Self {
        Self(x, y)
    }
    pub fn x(self) -> f64 { self.0 }
    pub fn y(self) -> f64 { self.1 }
    pub fn as_i32_tuple(self) -> (i32, i32) {
        (self.0 as i32, self.1 as i32)
    }
}

impl From<(f64, f64)> for WindowPosition {
    fn from((x, y): (f64, f64)) -> Self { Self(x, y) }
}
impl From<(i32, i32)> for WindowPosition {
    fn from((x, y): (i32, i32)) -> Self {
        Self(x as f64, y as f64)
    }
}
