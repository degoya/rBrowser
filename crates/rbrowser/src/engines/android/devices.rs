//! What adb says about the devices (RB-095-02): `adb devices -l` ([`parse_devices`]), the
//! screen of one (`wm size`, `wm density`, [`parse_screen`]), and the breakpoint that stands for
//! a device in the config ([`breakpoint_id`], [`serial_for`]). Pure: the runs are
//! `super::adb`'s. adb's output is untrusted text; a serial is taken only when it is a plain
//! USB serial ([`valid_serial`]) - never one that could read as an option after `-s`, and never
//! a network device (`host:port`, WLAN ADB is not in this job).

use rb_util::hex;
use serde::Serialize;
use sha2::{Digest as _, Sha256};

/// The longest serial taken.
pub const MAX_SERIAL: usize = 64;
/// The prefix of a device's breakpoint id.
pub const BREAKPOINT_PREFIX: &str = "android-";
/// The longest breakpoint id (`view.add_size`'s name limit).
const MAX_ID: usize = 40;
/// The bytes of a serial's hash a cut breakpoint id ends in (8 hex digits).
const HASH_BYTES: usize = 4;
/// Android's density of a pixel ratio of 1.
const BASE_DENSITY: f64 = 160.0;

/// A device's state as adb lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceState {
    /// Connected and authorised.
    Device,
    /// The person has not allowed USB debugging on the device yet.
    Unauthorized,
    /// Not answering.
    Offline,
    /// The OS does not let adb open the device (udev rules).
    NoPermissions,
    /// Anything else (`bootloader`, `recovery`, `sideload`).
    Other(String),
}

/// One device of `adb devices -l`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// The serial (`-s`).
    pub serial: String,
    /// The state.
    pub state: DeviceState,
    /// `model:` with spaces for `_`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// `product:`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
}

impl Device {
    /// Whether a view can be opened on it.
    pub fn is_ready(&self) -> bool {
        self.state == DeviceState::Device
    }

    /// The model, else the serial.
    pub fn name(&self) -> &str {
        self.model.as_deref().unwrap_or(&self.serial)
    }
}

/// A plain USB serial: 1 to [`MAX_SERIAL`] ASCII letters, digits, `.`, `_`, `-`, not starting
/// with `-` or `.`.
pub fn valid_serial(serial: &str) -> bool {
    !serial.is_empty()
        && serial.len() <= MAX_SERIAL
        && !serial.starts_with(['-', '.'])
        && serial
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The devices of `adb devices -l`; lines that are no device (the header, the daemon's `*`
/// notes) and serials [`valid_serial`] refuses are left out.
pub fn parse_devices(output: &str) -> Vec<Device> {
    output
        .lines()
        .filter(|line| !line.starts_with('*') && !line.starts_with("List of devices"))
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let serial = words.next()?;
            if !valid_serial(serial) {
                return None;
            }
            let rest: Vec<&str> = words.collect();
            let state = match rest.as_slice() {
                ["device", ..] => DeviceState::Device,
                ["unauthorized", ..] => DeviceState::Unauthorized,
                ["offline", ..] => DeviceState::Offline,
                ["no", "permissions", ..] => DeviceState::NoPermissions,
                [other, ..] => DeviceState::Other((*other).to_owned()),
                [] => return None,
            };
            let field = |name: &str| {
                rest.iter()
                    .find_map(|word| word.strip_prefix(name))
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            };
            Some(Device {
                serial: serial.to_owned(),
                state,
                model: field("model:").map(|model| model.replace('_', " ")),
                product: field("product:"),
            })
        })
        .collect()
}

/// The breakpoint id of the device `serial`: `android-<serial>`; one over the id limit keeps
/// the serial's start and ends in `-` and a hash of the whole serial, so two serials that share
/// their start keep apart.
pub fn breakpoint_id(serial: &str) -> String {
    let id = format!("{BREAKPOINT_PREFIX}{serial}");
    if id.chars().count() <= MAX_ID {
        return id;
    }
    let digest = Sha256::digest(serial.as_bytes());
    let suffix = hex(digest.get(..HASH_BYTES).unwrap_or_default());
    let kept = MAX_ID - suffix.len() - 1;
    let mut cut: String = id.chars().take(kept).collect();
    cut.push('-');
    cut.push_str(&suffix);
    cut
}

/// The device among `devices` whose breakpoint is `id`.
pub fn serial_for<'a>(id: &str, devices: &'a [Device]) -> Option<&'a Device> {
    devices
        .iter()
        .find(|device| breakpoint_id(&device.serial) == id)
}

/// A device's screen in device pixels and its pixel ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    /// Width in device pixels.
    pub width_px: u32,
    /// Height in device pixels.
    pub height_px: u32,
    /// Device pixels per CSS px (`density / 160`).
    pub dpr: f64,
}

impl Screen {
    /// The size in CSS px, rounded down (what the page's viewport is at most).
    pub fn css_size(&self) -> (u32, u32) {
        let css = |px: u32| {
            (f64::from(px) / self.dpr)
                .floor()
                .clamp(0.0, f64::from(u32::MAX)) as u32
        };
        (css(self.width_px), css(self.height_px))
    }
}

/// The screen from `wm size` and `wm density`: the override when one is set, else the
/// physical value; `None` when either does not read.
pub fn parse_screen(size: &str, density: &str) -> Option<Screen> {
    let value = |text: &str| {
        let pick = |label: &str| {
            text.lines()
                .find_map(|line| line.trim().strip_prefix(label))
                .map(str::trim)
                .map(ToOwned::to_owned)
        };
        pick("Override size:")
            .or_else(|| pick("Override density:"))
            .or_else(|| pick("Physical size:"))
            .or_else(|| pick("Physical density:"))
    };
    let (width, height) = value(size)?
        .split_once('x')
        .and_then(|(w, h)| Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?)))?;
    let density: u32 = value(density)?.parse().ok()?;
    (width > 0 && height > 0 && density > 0).then(|| Screen {
        width_px: width,
        height_px: height,
        dpr: f64::from(density) / BASE_DENSITY,
    })
}

#[cfg(test)]
#[path = "devices_tests.rs"]
mod tests;
