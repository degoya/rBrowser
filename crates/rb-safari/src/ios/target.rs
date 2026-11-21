//! Where an iOS view runs (RB-095-04): `engines.ios.mode` ([`IosMode`]) decides between the
//! iOS Simulator (Xcode) and an iPhone or iPad on USB, the breakpoint's `iosDevice` names a
//! model, a simulator or a device, and [`resolve`] picks the one target or says why there is none
//! ([`NoTarget`], each with its text and the setting to change). Without Xcode and without a
//! device there is no iOS view; the text names both ways. [`capabilities`] is the new session's
//! ask to `safaridriver`.

use std::time::Duration;

use serde_json::{Value, json};

use super::inventory::{Device, Inventory, Simulator};
use super::udid::{self, is_udid};

/// The setting every refusal links.
pub const MODE_SETTING: &str = "engines.ios.mode";

/// `engines.ios.mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IosMode {
    /// The simulator when Xcode is installed, else a connected device.
    #[default]
    Auto,
    /// Only the iOS Simulator.
    Simulator,
    /// Only a connected iPhone or iPad.
    Device,
}

impl IosMode {
    /// The mode of the setting's value; `auto` for anything else (the schema refuses it).
    pub fn from_setting(value: &str) -> Self {
        match value {
            "simulator" => Self::Simulator,
            "device" => Self::Device,
            _ => Self::Auto,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Simulator => "simulator",
            Self::Device => "device",
        }
    }
}

/// What the iOS engine runs: `engines.ios.*`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IosConfig {
    /// `engines.ios.mode`.
    pub mode: IosMode,
    /// `engines.ios.defaultSimulator`.
    pub default_simulator: String,
    /// `engines.ios.bootTimeoutSec`.
    pub boot_timeout: Duration,
}

impl Default for IosConfig {
    fn default() -> Self {
        Self {
            mode: IosMode::Auto,
            default_simulator: "iPhone 16".to_owned(),
            boot_timeout: Duration::from_secs(120),
        }
    }
}

/// The one simulator or device a view runs on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IosTarget {
    /// An iOS simulator (Xcode).
    Simulator(Simulator),
    /// An iPhone or iPad on USB.
    Device(Device),
}

impl IosTarget {
    /// Its UDID.
    pub fn udid(&self) -> &str {
        match self {
            Self::Simulator(sim) => &sim.udid,
            Self::Device(device) => &device.udid,
        }
    }

    /// For people: `iPhone 16 (iOS 18.2)`, `iPhone (USB)`.
    pub fn label(&self) -> String {
        match self {
            Self::Simulator(sim) => sim.label(),
            Self::Device(device) => device.label(),
        }
    }

    /// Whether it is a simulator.
    pub fn is_simulator(&self) -> bool {
        matches!(self, Self::Simulator(_))
    }
}

/// Why no iOS view can run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoTarget {
    /// Neither Xcode nor a connected device: both ways named.
    NoWay,
    /// `simulator` without Xcode.
    NeedsXcode,
    /// Xcode, but no iOS simulator (no runtime installed) and no device.
    NoSimulator,
    /// `device` without a connected device.
    NoDevice,
    /// Xcode has no simulator of this model.
    UnknownSimulator(String),
    /// The device (its UDID cut short) is not connected.
    UnknownDevice(String),
    /// The breakpoint names a simulator or device the mode (its name) leaves out.
    ModeExcludes(&'static str),
}

impl NoTarget {
    /// The catalogue key of its text.
    pub fn key(&self) -> &'static str {
        match self {
            Self::NoWay => "engines.ios.no.way",
            Self::NeedsXcode => "engines.ios.no.xcode",
            Self::NoSimulator => "engines.ios.no.simulator",
            Self::NoDevice => "engines.ios.no.device",
            Self::UnknownSimulator(_) => "engines.ios.no.model",
            Self::UnknownDevice(_) => "engines.ios.no.udid",
            Self::ModeExcludes(_) => "engines.ios.no.mode",
        }
    }

    /// The setting that decides it, for the notice's link.
    pub fn setting(&self) -> &'static str {
        MODE_SETTING
    }

    /// The text for people.
    pub fn reason(&self) -> String {
        match self {
            Self::UnknownSimulator(name) => rb_i18n::t_with(self.key(), &[("model", name)]),
            Self::UnknownDevice(short) => rb_i18n::t_with(self.key(), &[("udid", short)]),
            Self::ModeExcludes(mode) => rb_i18n::t_with(self.key(), &[("mode", mode)]),
            _ => rb_i18n::t(self.key()),
        }
    }
}

/// The target of a view whose breakpoint names `wanted` (`iosDevice`), in `inventory`.
pub fn resolve(
    config: &IosConfig,
    inventory: &Inventory,
    wanted: Option<&str>,
) -> Result<IosTarget, NoTarget> {
    let Some(wanted) = wanted.map(str::trim).filter(|wanted| !wanted.is_empty()) else {
        return default(config, inventory);
    };
    if let Some(device) = inventory
        .devices
        .iter()
        .find(|device| device.udid.eq_ignore_ascii_case(wanted))
    {
        return match config.mode {
            IosMode::Simulator => Err(NoTarget::ModeExcludes(config.mode.name())),
            _ => Ok(IosTarget::Device(device.clone())),
        };
    }
    if let Some(sim) = inventory
        .simulators
        .iter()
        .find(|sim| sim.udid.eq_ignore_ascii_case(wanted))
    {
        return match config.mode {
            IosMode::Device => Err(NoTarget::ModeExcludes(config.mode.name())),
            _ => Ok(IosTarget::Simulator(sim.clone())),
        };
    }
    if inventory.xcode && config.mode != IosMode::Device && !inventory.simulators.is_empty() {
        return by_name(inventory, wanted)
            .map(IosTarget::Simulator)
            .ok_or_else(|| NoTarget::UnknownSimulator(rb_util::text::cut(wanted, 80)));
    }
    if is_udid(wanted) {
        return Err(NoTarget::UnknownDevice(udid::short(wanted)));
    }
    // A model without its simulator (no Xcode): the device way, as the mode allows.
    default(config, inventory)
}

/// The target of a view that names none.
fn default(config: &IosConfig, inventory: &Inventory) -> Result<IosTarget, NoTarget> {
    let device = || inventory.devices.first().cloned().map(IosTarget::Device);
    match config.mode {
        IosMode::Simulator if !inventory.xcode => Err(NoTarget::NeedsXcode),
        IosMode::Simulator => default_simulator(config, inventory).ok_or(NoTarget::NoSimulator),
        IosMode::Device => device().ok_or(NoTarget::NoDevice),
        IosMode::Auto => {
            let simulator = inventory
                .xcode
                .then(|| default_simulator(config, inventory))
                .flatten();
            match (simulator.or_else(device), inventory.xcode) {
                (Some(target), _) => Ok(target),
                (None, true) => Err(NoTarget::NoSimulator),
                (None, false) => Err(NoTarget::NoWay),
            }
        }
    }
}

/// `engines.ios.defaultSimulator`, else the newest iPhone.
fn default_simulator(config: &IosConfig, inventory: &Inventory) -> Option<IosTarget> {
    by_name(inventory, &config.default_simulator)
        .or_else(|| {
            best(
                inventory
                    .simulators
                    .iter()
                    .filter(|sim| sim.name.starts_with("iPhone")),
            )
        })
        .map(IosTarget::Simulator)
}

/// The simulator of the model `name`: a booted one, else the newest runtime's.
fn by_name(inventory: &Inventory, name: &str) -> Option<Simulator> {
    best(
        inventory
            .simulators
            .iter()
            .filter(|sim| sim.name.eq_ignore_ascii_case(name.trim())),
    )
}

fn best<'a>(candidates: impl Iterator<Item = &'a Simulator>) -> Option<Simulator> {
    candidates
        .max_by(|a, b| {
            a.booted
                .cmp(&b.booted)
                .then_with(|| a.version.cmp(&b.version))
        })
        .cloned()
}

/// The capabilities of a new session on `target`: Mobile Safari on that one UDID.
pub fn capabilities(target: &IosTarget) -> Value {
    json!({
        "browserName": "safari",
        "platformName": "iOS",
        "safari:useSimulator": target.is_simulator(),
        "safari:deviceUDID": target.udid(),
    })
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
