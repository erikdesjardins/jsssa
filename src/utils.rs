use std::collections::hash_map::DefaultHasher;
use std::fmt::{self, Display};
use std::hash::{Hash, Hasher};
use std::time::Duration;

/// Shorthand for `Box::new` without `feature(box_syntax)`
#[allow(non_snake_case)]
pub fn P<T>(x: T) -> Box<T> {
    Box::new(x)
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
