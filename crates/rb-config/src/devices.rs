//! The device presets (RB-010-12, contract 0.1-w1b): the built-in list and the user's own.
//!
//! The built-in list is [`BUILTIN_DEVICES_FILE`], embedded at build time; the user's `devices`
//! settings add to it (same id wins, in place) or, with `replaceDefaults`, replace it.
//! RB-010-05 resolves `breakpoints[].device` against [`effective_devices`], RB-010-16 offers the
//! list in the quick pick, the shell gets it with the IPC message `getDevices`.

use crate::{ConfigError, ConfigIssue, DevicePreset, DeviceSettings};

/// The built-in device list, relative to the crate (`crates/rb-config/data/devices.json`): a
/// JSON array of [`DevicePreset`]; built-in names are the i18n keys `devices.<id>`.
pub const BUILTIN_DEVICES_FILE: &str = "data/devices.json";

/// The built-in list, embedded.
const BUILTIN_DEVICES_JSON: &str = include_str!("../data/devices.json");

/// The largest viewport side a preset may have, in CSS px.
const MAX_VIEWPORT: u32 = 10_000;
/// The largest device pixel ratio a preset may have.
const MAX_DPR: f64 = 10.0;

/// The built-in presets in file order.
pub fn builtin_devices() -> Result<Vec<DevicePreset>, ConfigError> {
    serde_json::from_str(BUILTIN_DEVICES_JSON).map_err(|error| ConfigError::Parse {
        path: BUILTIN_DEVICES_FILE.into(),
        line: error.line(),
        column: error.column(),
        message: error.to_string(),
    })
}

/// The presets in effect: the built-in ones with `user.items` merged in by id (a known id is
/// replaced in place, a new one appended in order; a later item wins over an earlier one), or
/// only `user.items` when `user.replace_defaults`. Every user item is checked first: an empty
/// id, a viewport side of 0 or over 10000 px, or a DPR outside (0, 10] refuse the whole list,
/// with one issue per problem at `/devices/items/<index>/<field>`.
pub fn effective_devices(user: &DeviceSettings) -> Result<Vec<DevicePreset>, ConfigError> {
    let issues: Vec<ConfigIssue> = user
        .items
        .iter()
        .enumerate()
        .flat_map(|(index, device)| check_device(device, &format!("/devices/items/{index}")))
        .collect();
    if !issues.is_empty() {
        return Err(ConfigError::Invalid(issues));
    }
    let mut devices = if user.replace_defaults {
        Vec::new()
    } else {
        builtin_devices()?
    };
    for item in &user.items {
        match devices.iter_mut().find(|device| device.id == item.id) {
            Some(existing) => *existing = item.clone(),
            None => devices.push(item.clone()),
        }
    }
    Ok(devices)
}

/// The problems of one preset at the JSON pointer `path`.
fn check_device(device: &DevicePreset, path: &str) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    if device.id.trim().is_empty() {
        issues.push(issue(
            format!("{path}/id"),
            "config.error.device_id",
            "a device preset needs an id".to_owned(),
        ));
    }
    let side = |value: u32| (1..=MAX_VIEWPORT).contains(&value);
    if !side(device.viewport.width) || !side(device.viewport.height) {
        issues.push(issue(
            format!("{path}/viewport"),
            "config.error.device_viewport",
            format!(
                "the viewport {}x{} is outside 1..={MAX_VIEWPORT} px",
                device.viewport.width, device.viewport.height
            ),
        ));
    }
    if !(device.dpr.is_finite() && device.dpr > 0.0 && device.dpr <= MAX_DPR) {
        issues.push(issue(
            format!("{path}/dpr"),
            "config.error.device_dpr",
            format!("the pixel ratio {} is outside (0, {MAX_DPR}]", device.dpr),
        ));
    }
    issues
}

fn issue(path: String, key: &str, message: String) -> ConfigIssue {
    ConfigIssue {
        path,
        key: key.to_owned(),
        message,
    }
}

#[cfg(test)]
#[path = "devices_tests.rs"]
mod tests;
