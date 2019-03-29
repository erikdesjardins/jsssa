use std::fmt::{self, Debug, Display};
use std::time::Duration;

use failure::Error;

#[allow(non_snake_case)]
pub fn P<T>(x: T) -> Box<T> {
    Box::new(x)
}

pub struct DisplayError(Error);

impl Debug for DisplayError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<T: Into<Error>> From<T> for DisplayError {
    fn from(display: T) -> Self {
        DisplayError(display.into())
    }
}

pub struct Time(pub Duration);

impl Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{:0>3}s", self.0.as_secs(), self.0.subsec_millis())
    }
}
