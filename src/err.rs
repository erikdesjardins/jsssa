use std::fmt::{self, Debug, Display};

pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

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
