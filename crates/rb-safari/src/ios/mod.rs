//! iOS as the engine of a view on macOS (RB-095-04): Mobile Safari over `safaridriver` - in the
//! iOS Simulator when Xcode is installed (any model, no device needed), else on an iPhone or
//! iPad on USB. The engine is a [`crate::SafariEngine`] of kind `ios`
//! ([`crate::SafariEngine::ios`]): the same driver, client, agent and replay as Safari's, with
//! the session asked for one simulator or device, no window to size (the size is the device's,
//! marked) and touch in place of the mouse.
//!
//! | Part | Where |
//! | --- | --- |
//! | Xcode, the simulators, the devices on USB; the parsers of `simctl` and `system_profiler` | [`inventory`] |
//! | `engines.ios.mode`, the pick of the one target, why there is none, the capabilities | [`target`] |
//! | The Mac's tools behind a trait (`xcrun`, `system_profiler`, `open`; macOS only) | [`tools`], [`host`] |
//! | UDIDs: their shape, from a USB serial, cut short for logs | [`udid`] |
//! | A click as a touch | [`touch`] |
//! | Fake tools for tests (feature `test-support`) | `fake` |
//!
//! The last inventory is kept ([`last_inventory`]) for the engine badge's menu, which lists the
//! simulators and devices without asking the Mac on the UI thread.

#[cfg(any(test, feature = "test-support"))]
pub mod fake;
pub mod host;
pub mod inventory;
pub mod target;
pub mod tools;
pub mod touch;
pub mod udid;

use std::sync::Mutex;

pub use inventory::{Device, Inventory, Simulator};
pub use target::{IosConfig, IosMode, IosTarget, NoTarget, capabilities, resolve};
pub use tools::{HostTools, IosTools};

/// The inventory last taken; `None` before the first.
static LAST: Mutex<Option<Inventory>> = Mutex::new(None);

/// The inventory last taken, without asking the Mac.
pub fn last_inventory() -> Option<Inventory> {
    rb_util::lock(&LAST).clone()
}

/// Takes the inventory from `tools`, keeps it and logs it (UDIDs cut short).
pub async fn take_inventory(tools: &dyn IosTools) -> Inventory {
    let inventory = tools.inventory().await;
    tracing::debug!(found = %inventory.log_line(), "iOS inventory");
    *rb_util::lock(&LAST) = Some(inventory.clone());
    inventory
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
