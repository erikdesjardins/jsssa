use swc_common::comments::Comments;
use swc_common::{Globals, GLOBALS};
use swc_ecma_transforms::util::COMMENTS;

/// A token proving that SWC's globals are initialized.
pub struct Initialized(());

/// Initializes the globals used by SWC and provides a token as proof.
///
/// A higher-rank lifetime ensures the token cannot escape the closure.
pub fn with<R>(f: impl FnOnce(&'_ Initialized) -> R) -> R {
    GLOBALS.set(&Globals::new(), || {
        COMMENTS.set(&Comments::default(), || f(&Initialized(())))
    })
}
