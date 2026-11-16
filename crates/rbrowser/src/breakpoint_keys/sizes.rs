//! The arithmetic of the breakpoint shortcuts (RB-010-16), without a view: the order by width
//! for `Leader ]` / `Leader [`, the +/-1 px step, the free size of the quick pick (`500x800`,
//! `500x800@2`), the breakpoint a device preset or a free size stands for, and the n-th
//! detected CSS breakpoint.

use rb_config::{Breakpoint, DevicePreset, EmulationFields, EngineKind, ProjectConfig};
use rb_ipc::DetectedBreakpoint;

/// The largest width or height a view takes, in CSS px.
pub const MAX_SIDE: u32 = 16_384;
/// The largest device pixel ratio a free size takes (the host's own limit).
pub const MAX_DPR: f64 = 10.0;
/// The id prefix of the breakpoint a free size stands for (`size-500x800@2`).
pub const SIZE_PREFIX: &str = "size-";

/// A free size: width and height in CSS px, optionally a device pixel ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreeSize {
    /// Width in CSS px.
    pub width: u32,
    /// Height in CSS px.
    pub height: u32,
    /// Device pixel ratio; `None` keeps the view's.
    pub dpr: Option<f64>,
}

impl FreeSize {
    /// Checks the sides (1 to [`MAX_SIDE`]) and the DPR (above 0, at most [`MAX_DPR`]).
    pub fn new(width: u32, height: u32, dpr: Option<f64>) -> Option<Self> {
        let side = |value: u32| (1..=MAX_SIDE).contains(&value);
        let dpr_ok = dpr.is_none_or(|dpr| dpr.is_finite() && dpr > 0.0 && dpr <= MAX_DPR);
        (side(width) && side(height) && dpr_ok).then_some(Self { width, height, dpr })
    }

    /// The id of the breakpoint it stands for: `size-500x800`, `size-500x800@2`.
    pub fn id(&self) -> String {
        match self.dpr {
            Some(dpr) => format!("{SIZE_PREFIX}{}x{}@{dpr}", self.width, self.height),
            None => format!("{SIZE_PREFIX}{}x{}", self.width, self.height),
        }
    }

    /// The breakpoint a new view with this size gets (the new-view modifier).
    pub fn breakpoint(&self) -> Breakpoint {
        Breakpoint {
            id: self.id(),
            label: Some(format!("{}x{}", self.width, self.height)),
            width: Some(self.width),
            height: Some(self.height),
            device: None,
            engine: EngineKind::Chromium,
            session: None,
            sync_navigation: true,
            shortcut: None,
            color: None,
            emulation: EmulationFields {
                dpr: self.dpr,
                ..EmulationFields::default()
            },
        }
    }
}

/// Parses the free input of the quick pick: `500x800`, `500 x 800`, `500X800`, `500*800`,
/// `500\u{d7}800`, each optionally with `@2` or `@2.5x` for the DPR.
pub fn parse_free_size(text: &str) -> Option<FreeSize> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let (size, dpr) = match text.split_once('@') {
        Some((size, dpr)) => {
            let dpr = dpr.strip_suffix(['x', 'X']).unwrap_or(dpr);
            (size, Some(dpr.parse::<f64>().ok()?))
        }
        None => (text.as_str(), None),
    };
    let (width, height) = size.split_once(['x', 'X', '*', '\u{d7}'])?;
    FreeSize::new(width.parse().ok()?, height.parse().ok()?, dpr)
}

/// The free size an id of [`FreeSize::id`] names.
pub fn free_size_of_id(id: &str) -> Option<FreeSize> {
    parse_free_size(id.strip_prefix(SIZE_PREFIX)?)
}

/// The breakpoint a device preset stands for: its viewport, and `device` for the rest of its
/// emulation (UA, DPR, touch), which RB-010-05's resolution fills in.
pub fn device_breakpoint(device: &DevicePreset, label: String) -> Breakpoint {
    Breakpoint {
        id: device.id.clone(),
        label: Some(label),
        width: Some(device.viewport.width),
        height: Some(device.viewport.height),
        device: Some(device.id.clone()),
        engine: EngineKind::Chromium,
        session: None,
        sync_navigation: true,
        shortcut: None,
        color: None,
        emulation: EmulationFields::default(),
    }
}

/// The breakpoint `id` names: one of the config, a device preset, or a free size.
pub fn find_breakpoint(
    config: &ProjectConfig,
    devices: &[DevicePreset],
    id: &str,
    device_label: impl Fn(&DevicePreset) -> String,
) -> Option<Breakpoint> {
    if let Some(breakpoint) = config.breakpoints.iter().find(|b| b.id == id) {
        return Some(breakpoint.clone());
    }
    if let Some(device) = devices.iter().find(|device| device.id == id) {
        return Some(device_breakpoint(device, device_label(device)));
    }
    free_size_of_id(id).map(|size| size.breakpoint())
}

/// Width and height of `breakpoint` in CSS px: its own, a side it leaves out from its device
/// preset in `devices` ([`Breakpoint::viewport`], RB-010-12); `(0, 0)` when it has none, which
/// a validated config rules out.
pub fn size_of(breakpoint: &Breakpoint, devices: &[DevicePreset]) -> (u32, u32) {
    breakpoint
        .viewport(devices)
        .map_or((0, 0), |viewport| (viewport.width, viewport.height))
}

/// The Chromium breakpoints by width, then height (both resolved against `devices`), then
/// config order (the view in the grid is Chromium; Firefox and WebKit views follow in 0.4).
pub fn by_width<'a>(
    breakpoints: &'a [Breakpoint],
    devices: &[DevicePreset],
) -> Vec<&'a Breakpoint> {
    let mut sorted: Vec<&Breakpoint> = breakpoints
        .iter()
        .filter(|breakpoint| breakpoint.engine == EngineKind::Chromium)
        .collect();
    // A stable sort keeps the config order among equal sizes.
    sorted.sort_by_key(|breakpoint| size_of(breakpoint, devices));
    sorted
}

/// The next (`forward`) or previous breakpoint by width from a view that shows `current` (a
/// breakpoint id) at `width`; it wraps around at both ends. From a free size it is the first
/// breakpoint wider (narrower) than `width`.
pub fn step<'a>(
    breakpoints: &'a [Breakpoint],
    devices: &[DevicePreset],
    current: Option<&str>,
    width: u32,
    forward: bool,
) -> Option<&'a Breakpoint> {
    let sorted = by_width(breakpoints, devices);
    let count = sorted.len();
    if count == 0 {
        return None;
    }
    if let Some(index) = current.and_then(|id| sorted.iter().position(|b| b.id == id)) {
        let next = if forward {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        };
        return sorted.get(next).copied();
    }
    if forward {
        sorted
            .iter()
            .find(|b| size_of(b, devices).0 > width)
            .or_else(|| sorted.first())
            .copied()
    } else {
        sorted
            .iter()
            .rev()
            .find(|b| size_of(b, devices).0 < width)
            .or_else(|| sorted.last())
            .copied()
    }
}

/// `width` moved by `delta` px, inside 1 to [`MAX_SIDE`]; `None` when it would leave them.
pub fn nudged(width: u32, delta: i32) -> Option<u32> {
    let moved = i64::from(width) + i64::from(delta);
    u32::try_from(moved)
        .ok()
        .filter(|moved| (1..=MAX_SIDE).contains(moved))
}

/// The width of the `number`-th (1-based) detected CSS breakpoint, by width.
pub fn detected_width(detected: &[DetectedBreakpoint], number: usize) -> Option<u32> {
    let mut widths: Vec<u32> = detected.iter().map(|found| found.width).collect();
    widths.sort_unstable();
    widths.dedup();
    widths.get(number.checked_sub(1)?).copied()
}

#[cfg(test)]
#[path = "sizes_tests.rs"]
mod tests;
