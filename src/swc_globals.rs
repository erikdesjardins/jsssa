use swc_common::{Globals, GLOBALS};

/// A token proving that SWC's globals are initialized.
pub struct Initialized(());

/// Initializes the globals used by SWC and provides a token as proof.
///
/// A higher-rank lifetime ensures the token cannot escape the closure.
pub fn with<R>(f: impl FnOnce(&'_ Initialized) -> R) -> R {
    GLOBALS.set(&Globals::new(), || f(&Initialized(())))
}
