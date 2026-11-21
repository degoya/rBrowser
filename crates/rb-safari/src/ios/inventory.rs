//! What iOS this Mac offers (RB-095-04): whether Xcode is installed, its iOS simulators
//! (`xcrun simctl list devices available --json`, [`parse_simctl`]) and the iPhones and iPads
//! on USB (`system_profiler SPUSBDataType SPUSBHostDataType -json`, [`parse_usb`] - built into
//! macOS, no Xcode needed). Both outputs are untrusted input: only what looks like a UDID is
//! kept, names are cut short.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::udid::{self, from_usb_serial, is_udid};

/// The longest name kept, in characters.
const MAX_NAME: usize = 80;
/// The runtimes of iOS (and iPadOS) simulators: `com.apple.CoreSimulator.SimRuntime.iOS-18-2`.
const IOS_RUNTIME: &str = "com.apple.CoreSimulator.SimRuntime.iOS-";

/// One iOS simulator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Simulator {
    /// Its UDID.
    pub udid: String,
    /// The model (`iPhone 16`).
    pub name: String,
    /// Its runtime for people (`iOS 18.2`).
    pub runtime: String,
    /// The runtime's version, for the order.
    pub version: Vec<u32>,
    /// Whether it runs now.
    pub booted: bool,
}

impl Simulator {
    /// `iPhone 16 (iOS 18.2)`.
    pub fn label(&self) -> String {
        format!("{} ({})", self.name, self.runtime)
    }
}

/// One iPhone or iPad on USB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// Its UDID (from the USB serial number).
    pub udid: String,
    /// What its USB port calls it (`iPhone`).
    pub name: String,
}

impl Device {
    /// `iPhone (USB)`.
    pub fn label(&self) -> String {
        format!("{} (USB)", self.name)
    }
}

/// What iOS this Mac offers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    /// Whether Xcode (and with it `simctl`) is installed.
    pub xcode: bool,
    /// The available iOS simulators, by name, the newest runtime first.
    pub simulators: Vec<Simulator>,
    /// The iPhones and iPads on USB.
    pub devices: Vec<Device>,
}

impl Inventory {
    /// The answer of `ios_devices`: Xcode, the simulators and the devices with their UDIDs
    /// (what `iosDevice` takes).
    pub fn to_json(&self) -> Value {
        json!({
            "xcode": self.xcode,
            "simulators": self.simulators.iter().map(|sim| json!({
                "udid": sim.udid,
                "name": sim.name,
                "runtime": sim.runtime,
                "booted": sim.booted,
            })).collect::<Vec<_>>(),
            "devices": self.devices.iter().map(|device| json!({
                "udid": device.udid,
                "name": device.name,
            })).collect::<Vec<_>>(),
        })
    }

    /// One log line: what was found, the devices' UDIDs cut short.
    pub fn log_line(&self) -> String {
        let devices: Vec<String> = self
            .devices
            .iter()
            .map(|device| format!("{} {}", device.name, udid::short(&device.udid)))
            .collect();
        format!(
            "xcode={} simulators={} devices=[{}]",
            self.xcode,
            self.simulators.len(),
            devices.join(", ")
        )
    }
}

/// The available iOS simulators in `simctl`'s JSON, by name and the newest runtime first.
pub fn parse_simctl(output: &str) -> Vec<Simulator> {
    let Ok(value) = serde_json::from_str::<Value>(output) else {
        return Vec::new();
    };
    let Some(runtimes) = value.get("devices").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut simulators = Vec::new();
    for (runtime, devices) in runtimes {
        let Some((runtime, version)) = runtime_of(runtime) else {
            continue;
        };
        for device in devices.as_array().into_iter().flatten() {
            let text = |key: &str| device.get(key).and_then(Value::as_str);
            let available = device.get("isAvailable").and_then(Value::as_bool) != Some(false);
            let (Some(udid), Some(name)) = (text("udid"), text("name")) else {
                continue;
            };
            if !available || !is_udid(udid) || name.trim().is_empty() {
                continue;
            }
            simulators.push(Simulator {
                udid: udid.to_owned(),
                name: rb_util::text::cut(name.trim(), MAX_NAME),
                runtime: runtime.clone(),
                version: version.clone(),
                booted: text("state") == Some("Booted"),
            });
        }
    }
    simulators.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| b.version.cmp(&a.version)));
    simulators
}

/// `iOS 18.2` and its version of an iOS runtime's id; `None` for another platform's.
pub fn runtime_of(id: &str) -> Option<(String, Vec<u32>)> {
    let version: Vec<u32> = id
        .strip_prefix(IOS_RUNTIME)?
        .split('-')
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    let text: Vec<String> = version.iter().map(u32::to_string).collect();
    Some((format!("iOS {}", text.join(".")), version))
}

/// The iPhones, iPads and iPods in `system_profiler`'s USB JSON (both data types), once each.
pub fn parse_usb(output: &str) -> Vec<Device> {
    let Ok(value) = serde_json::from_str::<Value>(output) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    walk(&value, &mut found, &mut seen);
    found
}

fn walk(value: &Value, found: &mut Vec<Device>, seen: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(device) = device_of(object)
                && seen.insert(device.udid.clone())
            {
                found.push(device);
            }
            object.values().for_each(|child| walk(child, found, seen));
        }
        Value::Array(items) => items.iter().for_each(|item| walk(item, found, seen)),
        _ => {}
    }
}

/// The device one USB entry describes, when it is an Apple phone or tablet with a serial.
fn device_of(object: &serde_json::Map<String, Value>) -> Option<Device> {
    let name = object.get("_name").and_then(Value::as_str)?.trim();
    if !["iPhone", "iPad", "iPod"]
        .iter()
        .any(|model| name.starts_with(model))
    {
        return None;
    }
    let field = |part: &str| {
        object
            .iter()
            .find(|(key, _)| key.to_ascii_lowercase().contains(part))
            .map(|(_, value)| value)
    };
    let apple = match field("vendor") {
        Some(Value::String(vendor)) => {
            let vendor = vendor.to_ascii_lowercase();
            vendor.contains("apple") || vendor.contains("05ac")
        }
        Some(Value::Number(id)) => id.as_u64() == Some(0x05ac),
        _ => true,
    };
    let serial = field("serial").and_then(Value::as_str)?;
    let udid = from_usb_serial(serial)?;
    apple.then(|| Device {
        udid,
        name: rb_util::text::cut(name, MAX_NAME),
    })
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
