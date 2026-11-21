//! The devices the menus offer (RB-094-20): the Android devices adb listed last, each with its
//! state, why the list is empty when it is, and whether Safari and iOS can run here (macOS).
//! The core refreshes the list in the background (`rbrowser::engines::android::menu`) - when
//! the window starts, the settings change, a device is added, and whenever the main menu or the
//! toolbar's "+" opens - so building a menu never waits on adb.

use serde::{Deserialize, Serialize};

/// What the main menu's "Devices & engines" and the toolbar's "+" list.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicesMenu {
    /// The devices of the last `adb devices -l`; empty while Android views are off.
    #[serde(default)]
    pub android: Vec<AndroidDeviceRow>,
    /// Why the list is empty or could not be read (no adb, adb failed), for the person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android_reason: Option<String>,
    /// Safari and iOS views can run in this build: macOS only.
    #[serde(default)]
    pub apple: bool,
}

/// One Android device as the menu lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidDeviceRow {
    /// The serial `view.add_android` takes.
    pub serial: String,
    /// The model, else the serial.
    pub name: String,
    /// Whether a view can be opened on it.
    pub state: AndroidDeviceState,
}

/// A device's state for the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AndroidDeviceState {
    /// Connected and authorised: a view can be opened.
    Ready,
    /// USB debugging not allowed on the device yet.
    Unauthorized,
    /// Not answering.
    Offline,
    /// The system does not let adb open it.
    NoPermissions,
    /// Bootloader, recovery, sideload.
    Other,
}

#[cfg(test)]
#[path = "devices_menu_tests.rs"]
mod tests;
