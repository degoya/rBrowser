//! Windows and Linux: there is no Safari.

use crate::driver::Edition;

/// No bundle, no version.
pub(super) async fn bundle_version(_edition: Edition) -> Option<String> {
    None
}
