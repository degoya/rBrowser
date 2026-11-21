//! Windows and Linux: there is no iOS Simulator and no `safaridriver` for a device.

use std::time::Duration;

use crate::error::SafariError;
use crate::ios::inventory::{Inventory, Simulator};

/// Nothing: no Xcode, no simulator, no device.
pub(in crate::ios) async fn inventory() -> Inventory {
    Inventory::default()
}

/// No simulator to boot.
pub(in crate::ios) async fn boot(
    _simulator: &Simulator,
    _timeout: Duration,
) -> Result<(), SafariError> {
    Err(SafariError::Launch(rb_i18n::t("engines.ios.onlyMacos")))
}
