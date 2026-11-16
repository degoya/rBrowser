//! `view.add_size` (RB-080-41, RB-080-46): a breakpoint of a new size or of a device preset in
//! `config.local.json`, opened as a view.
//!
//! | Arguments | |
//! | --- | --- |
//! | `{ width, height, dpr?, name?, x?, y? }` | a breakpoint of that size (`size-<width>x<height>` without a name) |
//! | `{ device, name?, x?, y? }` | a breakpoint that names the device preset: its size, pixel ratio, user agent and touch apply (without a name an id made from the preset's, [`device_breakpoint_id`]) |
//! | none | the picker: the palette's popup at its step "New view" (sizes, devices, "Custom...", `ui/src/palette/newViewProvider.ts`) |
//! | `{ custom: true }` | the picker's "Custom...": the size input in the toolbar (`NewSizeBar.vue`) |
//!
//! A view that was added is named in the status bar with its size ([`KEY_ADDED`]).

use std::collections::BTreeMap;

use rb_commands::CommandError;
use rb_config::{Breakpoint, ConfigFile, DevicePreset};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::store;
use super::view_edit::{ADD_SIZE, hidden_value, place_on_canvas, point, reload};
use super::view_selection::{base_grid, has_cell, own_preset_grid, selection};
use crate::breakpoint_keys::FreeSize;
use crate::breakpoint_keys::sizes_public::size_of;
use crate::control::Core;
use crate::page_window::PageWindow;

/// The `uiCommand` that opens the size input in the toolbar (the picker's "Custom...").
pub const ADD_SIZE_CUSTOM: &str = "view.add_size.custom";
/// The palette's route key for the picker: the popup's first load starts in that step, a
/// loaded popup hears the `uiCommand`.
pub const PICKER_STEP: &str = "new-view";
/// The longest name `view.add_size` takes, in characters.
pub const MAX_NAME_CHARS: usize = 40;
/// The status bar's message for an added view; `{name}`, `{width}`, `{height}`.
pub const KEY_ADDED: &str = "views.added";

/// `view.add_size`'s arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AddSizeArgs {
    width: Option<u32>,
    height: Option<u32>,
    /// A device preset's id, instead of `width` and `height`.
    device: Option<String>,
    /// The breakpoint's id; `size-<width>x<height>` or the device's id when absent.
    name: Option<String>,
    dpr: Option<f64>,
    /// Canvas: where the view goes, in canvas DIP.
    x: Option<f64>,
    y: Option<f64>,
    /// Open the size input in the toolbar.
    #[serde(default)]
    custom: bool,
}

/// What the new breakpoint is.
enum Wanted {
    Size(FreeSize),
    Device(DevicePreset),
}

impl Wanted {
    /// The breakpoint's id without a name of its own: the size's, or one made from the
    /// device's id ([`device_breakpoint_id`]).
    fn id(&self, breakpoints: &[Breakpoint]) -> String {
        match self {
            Self::Size(size) => size.id(),
            Self::Device(device) => device_breakpoint_id(device, breakpoints),
        }
    }

    /// Width and height.
    fn size(&self) -> (u32, u32) {
        match self {
            Self::Size(size) => (size.width, size.height),
            Self::Device(device) => (device.viewport.width, device.viewport.height),
        }
    }

    /// The entry of `breakpoints[]` under `id`.
    fn entry(&self, id: &str) -> Value {
        match self {
            Self::Size(size) => {
                let mut entry = json!({ "id": id, "width": size.width, "height": size.height });
                if let Some(dpr) = size.dpr {
                    entry["emulation"] = json!({ "dpr": dpr });
                }
                entry
            }
            Self::Device(device) => json!({ "id": id, "device": device.id }),
        }
    }

    /// Whether `existing`, a breakpoint of the id asked for, is what is asked for: the same
    /// device, or the same size (and pixel ratio, when one is asked for). A name stands for one
    /// thing - another size under it is refused, never silently opened as it is.
    fn is(&self, existing: &Breakpoint, devices: &[DevicePreset]) -> bool {
        match self {
            Self::Device(device) => existing.device.as_deref() == Some(device.id.as_str()),
            Self::Size(size) => {
                size_of(existing, devices) == (size.width, size.height)
                    && (size.dpr.is_none() || existing.emulation.dpr == size.dpr)
            }
        }
    }
}

/// A breakpoint id for `device`, whose own id only has to be non-empty (`My Phone`): its
/// letters, digits, `-`, `_` and `.` with a dash for everything else (`My-Phone`), from the
/// label when the id leaves nothing, `device` at last. A breakpoint of that id that names
/// another device or a size is not taken over: the id gets `-2`, `-3`, ... until it is free or
/// names this device already.
pub fn device_breakpoint_id(device: &DevicePreset, breakpoints: &[Breakpoint]) -> String {
    let base = [Some(device.id.as_str()), device.label.as_deref()]
        .into_iter()
        .flatten()
        .map(slug)
        .find(|slug| !slug.is_empty())
        .unwrap_or_else(|| "device".to_owned());
    let free = |id: &str| {
        breakpoints
            .iter()
            .find(|breakpoint| breakpoint.id == id)
            .is_none_or(|breakpoint| breakpoint.device.as_deref() == Some(device.id.as_str()))
    };
    if free(&base) {
        return base;
    }
    (2u32..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| free(id))
        .unwrap_or(base)
}

/// `text` as the start of a breakpoint id: every run of other characters one dash, none at the
/// ends, short enough for a `-<n>` after it.
fn slug(text: &str) -> String {
    let mut slug = String::new();
    for c in text.trim().chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.') {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let cut: String = slug
        .trim_matches('-')
        .chars()
        .take(MAX_NAME_CHARS - 6)
        .collect();
    cut.trim_end_matches('-').to_owned()
}

/// What the arguments ask for; `None` when they name neither a size nor a device.
fn wanted(args: &AddSizeArgs) -> Result<Option<Wanted>, CommandError> {
    let invalid = |text: &str| Err(CommandError::InvalidArgs(text.to_owned()));
    match (&args.device, args.width, args.height) {
        (None, None, None) => Ok(None),
        (Some(_), None, None) if args.dpr.is_some() => {
            invalid("a device has its own pixel ratio: pass device without dpr")
        }
        (Some(id), None, None) => crate::emulation::device_list()
            .into_iter()
            .find(|device| device.id == *id)
            .map(|device| Some(Wanted::Device(device)))
            .ok_or_else(|| CommandError::InvalidArgs(format!("no device preset `{id}`"))),
        (Some(_), _, _) => invalid("pass device, or width and height"),
        (None, Some(width), Some(height)) => FreeSize::new(width, height, args.dpr)
            .map(|size| Some(Wanted::Size(size)))
            .ok_or_else(|| {
                CommandError::InvalidArgs(format!(
                    "{width}x{height} at DPR {:?} is not a size",
                    args.dpr
                ))
            }),
        (None, _, _) => invalid("pass width and height, a device, or neither"),
    }
}

/// `view.add_size`: answers `{ breakpoint, width, height, dpr?, device?, added, changed, hidden }`
/// with what is open in the end (the size and device of a breakpoint that existed already), or
/// `null` when the picker or the size input was opened instead. A view that was open already
/// changes nothing and is no news for the status bar.
pub(super) fn add_size(core: &Core, args: AddSizeArgs) -> Result<Value, CommandError> {
    if args.custom {
        // The size input sits in the toolbar strip: the shell takes the keyboard.
        core.ui().focus_shell();
        core.send(&CoreToShell::UiCommand {
            id: ADD_SIZE_CUSTOM.to_owned(),
        });
        return Ok(Value::Null);
    }
    let Some(wanted) = wanted(&args)? else {
        // The palette's popup lies in front of the views and takes the typing; its first load
        // reads the step from the route, a loaded one from the `uiCommand`.
        let url = crate::shell::page_url(&format!("{}/{PICKER_STEP}", PageWindow::Palette.route()));
        core.ui().open_page_window(PageWindow::Palette, url);
        core.send(&CoreToShell::UiCommand {
            id: ADD_SIZE.to_owned(),
        });
        return Ok(Value::Null);
    };
    let at = point(args.x, args.y)?;
    let host = core.host();
    let config = host.config();
    let id = match &args.name {
        Some(name) => breakpoint_id(name)?,
        None => wanted.id(&config.breakpoints),
    };
    let hidden = selection(host);
    let devices = crate::emulation::device_list();
    let existing = config
        .breakpoints
        .iter()
        .find(|breakpoint| breakpoint.id == id);
    // What is open in the end: the breakpoint that exists, else the one asked for.
    let (width, height) = match existing {
        Some(existing) => {
            let size = size_of(existing, &devices);
            if !wanted.is(existing, &devices) {
                return Err(CommandError::InvalidArgs(format!(
                    "a breakpoint `{id}` exists with another size or device ({} x {}): choose another name",
                    size.0, size.1
                )));
            }
            size
        }
        None => wanted.size(),
    };
    let device = match existing {
        Some(existing) => existing.device.clone(),
        None => match &wanted {
            Wanted::Device(device) => Some(device.id.clone()),
            Wanted::Size(_) => None,
        },
    };
    let exists = existing.is_some();
    let changed = !exists || hidden.contains(&id);
    let next: Vec<String> = hidden
        .iter()
        .filter(|known| **known != id)
        .cloned()
        .collect();
    if changed {
        // An explicit grid shows only its cells: a breakpoint without one gets a column of its
        // own. A preset's own grid is not extended - the view would not show.
        let grid = base_grid(host);
        if !has_cell(&grid, &id) && own_preset_grid(host).is_some() {
            return Err(CommandError::InvalidArgs(format!(
                "the preset shown has a grid of its own without a cell for `{id}`"
            )));
        }
        let cells = (!has_cell(&config.layout, &id))
            .then(|| crate::host::state::layout_with_extra_column(&config.layout, &id));
        let path: Vec<String> = Vec::new();
        store::write_into(core, Some(ConfigFile::Local), &path, |document| {
            if !exists {
                add_entry(document, &id, wanted.entry(&id));
            }
            if let Some(cells) = &cells {
                set_cells(document, cells);
            }
            let mut fields = Map::new();
            fields.insert("hidden".to_owned(), hidden_value(core, &next));
            store::patch(document, &["layout".to_owned()], None, &fields);
        })?;
        reload(core)?;
        crate::presets::on_config_changed(core);
        if let Some((x, y)) = at {
            place_on_canvas(core, &id, x, y);
        }
        // Only a view that opened is news.
        core.notify(added_notice(&id, width, height));
    }
    let mut answer = json!({
        "breakpoint": id,
        "width": width,
        "height": height,
        "added": !exists,
        "changed": changed,
        "hidden": next,
    });
    if let Some(device) = device {
        if let Some(preset) = devices.iter().find(|preset| preset.id == device) {
            answer["dpr"] = json!(preset.dpr);
        }
        answer["device"] = json!(device);
    }
    Ok(answer)
}

/// The status bar's message for the view of breakpoint `id`.
fn added_notice(id: &str, width: u32, height: u32) -> CoreToShell {
    let args: BTreeMap<String, String> = [
        ("name", id.to_owned()),
        ("width", width.to_string()),
        ("height", height.to_string()),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value))
    .collect();
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: KEY_ADDED.to_owned(),
        args,
        settings_key: None,
    })
}

/// `name` trimmed, when it is 1 to [`MAX_NAME_CHARS`] letters, digits, `-`, `_` or `.`.
pub fn breakpoint_id(name: &str) -> Result<String, CommandError> {
    let id = name.trim();
    let count = id.chars().count();
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    if count == 0 || count > MAX_NAME_CHARS || !id.chars().all(allowed) {
        return Err(CommandError::InvalidArgs(format!(
            "a breakpoint name is 1 to {MAX_NAME_CHARS} letters, digits, '-', '_' or '.'"
        )));
    }
    Ok(id.to_owned())
}

/// Adds the breakpoint `id` of `size` to `document`'s `breakpoints` (the merge joins it by id).
pub fn add_breakpoint(document: &mut Value, id: &str, size: &FreeSize) {
    add_entry(document, id, Wanted::Size(*size).entry(id));
}

/// Adds `entry` as the breakpoint `id` to `document`'s `breakpoints`, unless it is there.
fn add_entry(document: &mut Value, id: &str, entry: Value) {
    if !document.is_object() {
        *document = json!({});
    }
    let Some(object) = document.as_object_mut() else {
        return;
    };
    let breakpoints = object
        .entry("breakpoints".to_owned())
        .or_insert_with(|| json!([]));
    if !breakpoints.is_array() {
        *breakpoints = json!([]);
    }
    if let Some(list) = breakpoints.as_array_mut()
        && !list
            .iter()
            .any(|item| item.get("id").and_then(Value::as_str) == Some(id))
    {
        list.push(entry);
    }
}

/// Writes the grid `cells` (columns, rows, cells) into `document`'s `layout`: an explicit grid
/// of the project with one more column, so the new view has a cell.
fn set_cells(document: &mut Value, cells: &rb_config::Layout) {
    let Ok(Value::Object(computed)) = serde_json::to_value(cells) else {
        return;
    };
    let mut fields = Map::new();
    for key in ["columns", "rows", "cells"] {
        if let Some(value) = computed.get(key) {
            fields.insert(key.to_owned(), value.clone());
        }
    }
    store::patch(document, &["layout".to_owned()], None, &fields);
}

#[cfg(test)]
#[path = "view_add_size_tests.rs"]
mod tests;
