//! The size quick pick of a view (RB-010-16): what the native menu on the cell's label strip
//! lists - the project's breakpoints, the device presets (RB-010-12), the CSS breakpoints the
//! analyser found (RB-010-13) and the free input - each with the arguments of
//! `view.set_size` it runs. The command palette's "Set view to ..." entries (RB-060-11)
//! are the same list.

use rb_config::{DeviceKind, DevicePreset, ProjectConfig};
use rb_engine::ViewId;
use rb_ipc::{BoundaryKind, DetectedBreakpoint};
use serde_json::{Value, json};

use super::actions::{device_label, format_dpr};
use super::sizes::{by_width, size_of};

/// The i18n key of a breakpoint row; `{label}`, `{width}`, `{height}`.
pub const ROW_BREAKPOINT: &str = "sizes.pick.breakpoint";
/// The i18n key of a device row; `{label}`, `{width}`, `{height}`, `{dpr}`.
pub const ROW_DEVICE: &str = "sizes.pick.device";
/// The i18n key of a detected row with a framework name; `{label}`, `{width}`, `{query}`.
pub const ROW_DETECTED_NAMED: &str = "sizes.pick.detectedNamed";
/// The i18n key of a detected row; `{width}`, `{query}`.
pub const ROW_DETECTED: &str = "sizes.pick.detected";
/// The i18n key of the free input's row.
pub const ROW_CUSTOM: &str = "sizes.pick.custom";

/// The sections of the quick pick, in menu order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// The project's breakpoints, by width.
    Breakpoints,
    /// The device presets, phones then tablets then desktops.
    Devices,
    /// The CSS breakpoints the analyser found, by width.
    Detected,
    /// The free input (`B x H @DPR`) in the shell.
    Custom,
}

/// One row of the quick pick.
#[derive(Clone, Debug, PartialEq)]
pub struct PickItem {
    /// Its section.
    pub section: Section,
    /// The text (already translated).
    pub label: String,
    /// Whether the view shows it now.
    pub checked: bool,
    /// The arguments of `view.set_size` (the view and `newView` are added on choice);
    /// `null` opens the free input.
    pub args: Value,
}

/// What the view shows now: its breakpoint id and its width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Current<'a> {
    /// The breakpoint (or device preset) id; `None` for a free size.
    pub breakpoint: Option<&'a str>,
    /// The width in CSS px.
    pub width: u32,
}

/// The rows of the quick pick for a view showing `current`.
pub fn items(
    config: &ProjectConfig,
    devices: &[DevicePreset],
    detected: &[DetectedBreakpoint],
    current: Current<'_>,
) -> Vec<PickItem> {
    let mut items = Vec::new();
    for breakpoint in by_width(&config.breakpoints, devices) {
        let label = breakpoint.label.as_deref().unwrap_or(&breakpoint.id);
        let (width, height) = size_of(breakpoint, devices);
        items.push(PickItem {
            section: Section::Breakpoints,
            label: rb_i18n::t_with(
                ROW_BREAKPOINT,
                &[
                    ("label", label),
                    ("width", &width.to_string()),
                    ("height", &height.to_string()),
                ],
            ),
            checked: current.breakpoint == Some(breakpoint.id.as_str()),
            args: json!({ "breakpoint": breakpoint.id }),
        });
    }
    let mut devices: Vec<&DevicePreset> = devices.iter().collect();
    devices.sort_by_key(|device| (kind_order(device.kind), device.viewport.width));
    for device in devices {
        items.push(PickItem {
            section: Section::Devices,
            label: rb_i18n::t_with(
                ROW_DEVICE,
                &[
                    ("label", &device_label(device)),
                    ("width", &device.viewport.width.to_string()),
                    ("height", &device.viewport.height.to_string()),
                    ("dpr", &format_dpr(device.dpr)),
                ],
            ),
            checked: current.breakpoint == Some(device.id.as_str()),
            args: json!({ "breakpoint": device.id }),
        });
    }
    let mut detected: Vec<&DetectedBreakpoint> = detected.iter().collect();
    detected.sort_by_key(|found| found.width);
    detected.dedup_by_key(|found| found.width);
    for found in detected {
        let width = found.width.to_string();
        let query = match found.kind {
            BoundaryKind::Min => "min-width",
            BoundaryKind::Max => "max-width",
        };
        let label = match &found.label {
            Some(name) => rb_i18n::t_with(
                ROW_DETECTED_NAMED,
                &[("label", name), ("width", &width), ("query", query)],
            ),
            None => rb_i18n::t_with(ROW_DETECTED, &[("width", &width), ("query", query)]),
        };
        items.push(PickItem {
            section: Section::Detected,
            label,
            checked: current.breakpoint.is_none() && current.width == found.width,
            args: json!({ "width": found.width }),
        });
    }
    items.push(PickItem {
        section: Section::Custom,
        label: rb_i18n::t(ROW_CUSTOM),
        checked: false,
        args: Value::Null,
    });
    items
}

/// The arguments `item` runs `view.set_size` with for `view`; `None` for the free input.
pub fn command_args(item: &PickItem, view: ViewId, new_view: bool) -> Option<Value> {
    let mut args = item.args.as_object()?.clone();
    args.insert("view".to_owned(), json!(view));
    if new_view {
        args.insert("newView".to_owned(), Value::Bool(true));
    }
    Some(Value::Object(args))
}

fn kind_order(kind: DeviceKind) -> u8 {
    match kind {
        DeviceKind::Mobile => 0,
        DeviceKind::Tablet => 1,
        DeviceKind::Desktop => 2,
    }
}

#[cfg(test)]
#[path = "quick_pick_tests.rs"]
mod tests;
