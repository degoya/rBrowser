//! [`IosTools`]: what the iOS engine asks the Mac (RB-095-04) - the inventory (Xcode, its
//! simulators, the devices on USB) and a simulator's boot - behind a trait, so the tests drive the
//! engine with a fake ([`super::fake::FakeIosTools`]) and the app with the Mac's tools
//! ([`HostTools`]: `xcrun simctl`, `system_profiler`, `open -a Simulator`; macOS only, an empty
//! inventory elsewhere).

use std::time::Duration;

use async_trait::async_trait;

use super::host;
use super::inventory::{Inventory, Simulator};
use crate::error::SafariError;

/// How the iOS engine learns what the Mac offers and boots a simulator.
#[async_trait]
pub trait IosTools: Send + Sync {
    /// Xcode, the iOS simulators and the devices on USB, as they are now.
    async fn inventory(&self) -> Inventory;

    /// Boots `simulator` (shown in the Simulator app) and waits until it is up, at most
    /// `timeout`.
    async fn boot(&self, simulator: &Simulator, timeout: Duration) -> Result<(), SafariError>;
}

/// This Mac's tools; on Windows and Linux an empty inventory and no boot.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostTools;

#[async_trait]
impl IosTools for HostTools {
    async fn inventory(&self) -> Inventory {
        host::inventory().await
    }

    async fn boot(&self, simulator: &Simulator, timeout: Duration) -> Result<(), SafariError> {
        host::boot(simulator, timeout).await
    }
}
