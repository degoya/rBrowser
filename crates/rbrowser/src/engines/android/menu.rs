//! The Android devices of the menus (RB-094-20): the main menu's "Devices & engines" and the
//! toolbar's "+" list the devices adb saw last, each with its state, from the shell state
//! (`rb_ipc::DevicesMenu`), so building a menu never waits on adb. The list is taken again in
//! the background ([`refresh`]) when the window's engines start, when the settings change,
//! whenever one of those menus opens (`crate::layout::native_menu`), and from the listing
//! `view.add_android` makes anyway ([`store_devices`]). One listing runs at a time; with Android
//! views off nothing runs and the list is empty.

use std::sync::atomic::{AtomicBool, Ordering};

use rb_ipc::{AndroidDeviceRow, AndroidDeviceState, DevicesMenu};

use super::adb;
use super::devices::{Device, DeviceState};
use super::engine::AndroidEngine;
use crate::control::Core;

/// Safari and iOS views run in this build (macOS only).
pub const APPLE: bool = cfg!(target_os = "macos");

/// A listing in flight; one at a time.
static LISTING: AtomicBool = AtomicBool::new(false);

/// Holds the one listing; dropping it frees the next (also when its task ends early).
pub(super) struct Listing;

impl Listing {
    /// The listing, unless one runs.
    pub(super) fn start() -> Option<Self> {
        (!LISTING.swap(true, Ordering::AcqRel)).then_some(Self)
    }
}

impl Drop for Listing {
    fn drop(&mut self) {
        LISTING.store(false, Ordering::Release);
    }
}

/// `device` as the menu lists it.
pub fn row(device: &Device) -> AndroidDeviceRow {
    AndroidDeviceRow {
        serial: device.serial.clone(),
        name: device.name().to_owned(),
        state: match device.state {
            DeviceState::Device => AndroidDeviceState::Ready,
            DeviceState::Unauthorized => AndroidDeviceState::Unauthorized,
            DeviceState::Offline => AndroidDeviceState::Offline,
            DeviceState::NoPermissions => AndroidDeviceState::NoPermissions,
            DeviceState::Other(_) => AndroidDeviceState::Other,
        },
    }
}

/// The menu's devices: `devices` in adb's order, `reason` why the list is empty.
pub fn menu_of(devices: &[Device], reason: Option<String>) -> DevicesMenu {
    DevicesMenu {
        android: devices.iter().map(row).collect(),
        android_reason: reason,
        apple: APPLE,
    }
}

/// What `engine` lists now: nothing while Android views are off, else adb's devices or why
/// there are none.
pub(super) async fn list(engine: &AndroidEngine) -> DevicesMenu {
    if !engine.is_enabled() {
        return menu_of(&[], None);
    }
    let runner = match engine.adb() {
        Ok(runner) => runner,
        Err(reason) => return menu_of(&[], Some(reason)),
    };
    match adb::devices(runner.as_ref()).await {
        Ok(devices) => menu_of(&devices, None),
        Err(error) => menu_of(&[], Some(error.to_string())),
    }
}

/// Takes the list again in the background and puts it into the shell state; nothing while a
/// listing runs or before the window's engines started.
pub fn refresh(core: &Core) {
    let Some(core) = core.arc() else {
        return;
    };
    let Some(listing) = Listing::start() else {
        return;
    };
    let runtime = core.runtime().clone();
    runtime.spawn(async move {
        let _listing = listing;
        let menu = match super::for_host(core.host()) {
            Some(engine) => list(&engine).await,
            None => menu_of(&[], None),
        };
        store(&core, menu);
    });
}

/// The devices a listing made elsewhere (`view.add_android`) found, for the menus.
pub fn store_devices(core: &Core, devices: &[Device]) {
    store(core, menu_of(devices, None));
}

fn store(core: &Core, menu: DevicesMenu) {
    core.host()
        .update_shell_state(|shell| shell.devices = Some(menu));
}

#[cfg(test)]
#[path = "menu_tests.rs"]
mod tests;
