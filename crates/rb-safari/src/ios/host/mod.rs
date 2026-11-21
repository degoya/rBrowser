//! The Mac's iOS tools (RB-095-04): the commands are macOS's ([`macos`], linted and tested on a
//! Mac only, the CI job `macos`); on Windows and Linux there is no iOS ([`other`]). The command
//! lines are pure and tested everywhere: every UDID that goes into one passed
//! [`super::udid::is_udid`].

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod other;

#[cfg(target_os = "macos")]
pub(super) use macos::{boot, inventory};
#[cfg(not(target_os = "macos"))]
pub(super) use other::{boot, inventory};

/// `xcrun --find simctl`: succeeds only with Xcode (the Command Line Tools have no `simctl`).
pub const FIND_SIMCTL: [&str; 2] = ["--find", "simctl"];
/// `xcrun simctl list devices available --json`.
pub const LIST_SIMULATORS: [&str; 5] = ["simctl", "list", "devices", "available", "--json"];
/// `system_profiler` for the USB devices: the data type up to macOS 14 and the one of macOS 15.
pub const LIST_USB: [&str; 3] = ["SPUSBDataType", "SPUSBHostDataType", "-json"];

/// `xcrun simctl boot <udid>`.
pub fn boot_args(udid: &str) -> Vec<String> {
    vec!["simctl".to_owned(), "boot".to_owned(), udid.to_owned()]
}

/// `xcrun simctl bootstatus <udid>`: waits until the simulator is up.
pub fn bootstatus_args(udid: &str) -> Vec<String> {
    vec![
        "simctl".to_owned(),
        "bootstatus".to_owned(),
        udid.to_owned(),
    ]
}

/// `open -a Simulator --args -CurrentDeviceUDID <udid>`: the Simulator app shows that device.
pub fn show_args(udid: &str) -> Vec<String> {
    ["-a", "Simulator", "--args", "-CurrentDeviceUDID", udid]
        .iter()
        .map(|part| (*part).to_owned())
        .collect()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
