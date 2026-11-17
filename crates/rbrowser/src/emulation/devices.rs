//! The device presets `breakpoints[].device` is resolved against (RB-010-05 over RB-010-12's
//! list): the built-in list until the user settings are known, then the effective one. One list
//! per process - one instance shows one project with one user's settings.

use std::sync::{PoisonError, RwLock};

use rb_config::{DevicePreset, DeviceSettings};

static DEVICES: RwLock<Option<Vec<DevicePreset>>> = RwLock::new(None);

/// The presets in effect: the user's once [`set_user_devices`] ran, else the built-in ones (none
/// while RB-010-12's list is not there).
pub fn device_list() -> Vec<DevicePreset> {
    if let Some(devices) = DEVICES
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
    {
        return devices.clone();
    }
    let builtin = rb_config::builtin_devices().unwrap_or_else(|error| {
        tracing::debug!(%error, "no built-in device presets");
        Vec::new()
    });
    *DEVICES.write().unwrap_or_else(PoisonError::into_inner) = Some(builtin.clone());
    builtin
}

/// Puts the user's `devices` settings in effect (start-up, a changed settings file).
pub fn set_user_devices(settings: &DeviceSettings) {
    match rb_config::effective_devices(settings) {
        Ok(devices) => {
            *DEVICES.write().unwrap_or_else(PoisonError::into_inner) = Some(devices);
        }
        Err(error) => tracing::warn!(%error, "the device presets were not merged"),
    }
}
