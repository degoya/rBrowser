//! The viewport a breakpoint shows (RB-010-12, owner decision (a)): its own `width` and
//! `height`, and for a side it leaves out the viewport of the device preset it names.
//!
//! This is the one place the size is resolved; every consumer (emulation, grid, labels,
//! breakpoint shortcuts, the shell's breakpoint list) asks [`Breakpoint::viewport`]. A
//! breakpoint with neither a side nor a `device` is refused by [`crate::check`]; one whose
//! missing side should come from an unknown preset by [`check_viewports`], which needs the
//! presets in effect (built-in plus the user's `devices`).

use crate::{Breakpoint, ConfigIssue, DevicePreset, ProjectConfig, Viewport};

impl Breakpoint {
    /// The viewport in CSS px: `width` and `height` of the breakpoint, a side it leaves out from
    /// its `paper` (a print view, RB-080-10), else from the viewport of its `device` preset in
    /// `devices`. `None` when a side has neither (no preset, or one `devices` does not hold).
    pub fn viewport(&self, devices: &[DevicePreset]) -> Option<Viewport> {
        if let (Some(width), Some(height)) = (self.width, self.height) {
            return Some(Viewport { width, height });
        }
        // A print view's paper (RB-080-10) comes before a device preset.
        if let Some(paper) = self.emulation.paper {
            let sheet = paper.viewport();
            return Some(Viewport {
                width: self.width.unwrap_or(sheet.width),
                height: self.height.unwrap_or(sheet.height),
            });
        }
        let id = self.device.as_deref()?;
        let preset = devices.iter().find(|device| device.id == id)?;
        Some(Viewport {
            width: self.width.unwrap_or(preset.viewport.width),
            height: self.height.unwrap_or(preset.viewport.height),
        })
    }
}

/// Every breakpoint of `config` that leaves out a side and names a preset `devices` does not
/// hold, at `/breakpoints/<index>/device` (`config.error.unknown_device`). An unknown preset
/// beside a full size is no issue here: the emulation reports it and keeps the size (RB-010-05).
pub fn check_viewports(config: &ProjectConfig, devices: &[DevicePreset]) -> Vec<ConfigIssue> {
    config
        .breakpoints
        .iter()
        .enumerate()
        .filter(|(_, breakpoint)| {
            (breakpoint.width.is_none() || breakpoint.height.is_none())
                && breakpoint.emulation.paper.is_none()
        })
        .filter_map(|(index, breakpoint)| {
            let id = breakpoint.device.as_deref()?;
            if devices.iter().any(|device| device.id == id) {
                return None;
            }
            Some(ConfigIssue {
                path: format!("/breakpoints/{index}/device"),
                key: "config.error.unknown_device".to_owned(),
                message: format!(
                    "breakpoint `{}` takes its size from the device preset `{id}`, which is \
                     neither built in nor in the user settings' `devices`: set `width` and \
                     `height`, or name a known preset",
                    breakpoint.id
                ),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "viewport_tests.rs"]
mod tests;
