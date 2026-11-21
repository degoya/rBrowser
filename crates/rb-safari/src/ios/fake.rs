//! [`FakeIosTools`]: the Mac's iOS tools in memory, for tests (feature `test-support`): an
//! inventory the test sets, the boots it was asked for, a boot that fails.

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use rb_util::lock;

use super::inventory::{Device, Inventory, Simulator};
use super::tools::IosTools;
use crate::error::SafariError;

/// The simulator the fake offers by default: an iPhone 16 on iOS 18.2.
pub const SIMULATOR: &str = "8B1E5A9C-2F3D-4C11-9E77-0A1B2C3D4E5F";
/// The device the fake offers with [`FakeIosTools::with_device`].
pub const DEVICE: &str = "00008110-001A2B3C4D5E801E";

/// Fake iOS tools.
#[derive(Default)]
pub struct FakeIosTools {
    inventory: Mutex<Inventory>,
    booted: Mutex<Vec<String>>,
    fail_boot: Mutex<Option<String>>,
}

impl FakeIosTools {
    /// Xcode with one iPhone 16 simulator, shut down.
    pub fn with_xcode() -> Self {
        let tools = Self::default();
        tools.set(Inventory {
            xcode: true,
            simulators: vec![Simulator {
                udid: SIMULATOR.to_owned(),
                name: "iPhone 16".to_owned(),
                runtime: "iOS 18.2".to_owned(),
                version: vec![18, 2],
                booted: false,
            }],
            devices: Vec::new(),
        });
        tools
    }

    /// No Xcode, one iPhone on USB.
    pub fn with_device() -> Self {
        let tools = Self::default();
        tools.set(Inventory {
            xcode: false,
            simulators: Vec::new(),
            devices: vec![Device {
                udid: DEVICE.to_owned(),
                name: "iPhone".to_owned(),
            }],
        });
        tools
    }

    /// The inventory from now on.
    pub fn set(&self, inventory: Inventory) {
        *lock(&self.inventory) = inventory;
    }

    /// The UDIDs booted so far, in order.
    pub fn booted(&self) -> Vec<String> {
        lock(&self.booted).clone()
    }

    /// The next boots fail with `reason`.
    pub fn fail_boot(&self, reason: &str) {
        *lock(&self.fail_boot) = Some(reason.to_owned());
    }
}

#[async_trait]
impl IosTools for FakeIosTools {
    async fn inventory(&self) -> Inventory {
        lock(&self.inventory).clone()
    }

    async fn boot(&self, simulator: &Simulator, _timeout: Duration) -> Result<(), SafariError> {
        if let Some(reason) = lock(&self.fail_boot).clone() {
            return Err(SafariError::Launch(reason));
        }
        lock(&self.booted).push(simulator.udid.clone());
        let mut inventory = lock(&self.inventory);
        for sim in &mut inventory.simulators {
            if sim.udid == simulator.udid {
                sim.booted = true;
            }
        }
        Ok(())
    }
}
