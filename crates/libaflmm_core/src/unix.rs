//! Unix [`Error`] conversions.

use crate::Error;

impl From<nix::Error> for Error {
    fn from(err: nix::Error) -> Self {
        crate::unknown!("Unix error: {err:?}")
    }
}
