use std::collections::hash_map::DefaultHasher;
use std::fmt::{self, Debug, Display};
use std::hash::{Hash, Hasher};
use std::time::Duration;

use failure::Error;

/// Shorthand for `Box::new` without `feature(box_syntax)`
#[allow(non_snake_case)]
pub fn P<T>(x: T) -> Box<T> {
    Box::new(x)
}

/// Delegates `Debug` to `Display`, since `Termination` always uses `Debug` to print errors
pub struct NiceError(Error);

impl Debug for NiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<T: Into<Error>> From<T> for NiceError {
    fn from(display: T) -> Self {
        NiceError(display.into())
    }
}

/// Pretty-printing wrapper for `Duration`, outputs "1.234s"
pub struct Time(pub Duration);

impl Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{:0>3}s", self.0.as_secs(), self.0.subsec_millis())
    }
}

/// Hash a value with the default hasher
pub fn default_hash<H: Hash + ?Sized>(h: &H) -> u64 {
    let mut hasher = DefaultHasher::new();
    h.hash(&mut hasher);
    hasher.finish()
}
