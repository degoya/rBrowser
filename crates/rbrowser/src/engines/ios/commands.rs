//! `ios.devices` (RB-095-04, `Leader B I`, MCP `ios_devices`): the simulators and devices of
//! this Mac, and the pick that sets the engine of the active view.
//!
//! With `device` (a model, a simulator's or a device's UDID) the view's breakpoint is written to
//! `engine: "ios"` and that `iosDevice` (`view.set_engine`); without, the Mac's inventory is
//! taken in the background and the engine menu of the view opens with a row per simulator model
//! and per device under "iOS" ([`device_items`]; a model row writes the model's name, which
//! holds on every Mac, a device row the device's UDID). A Mac with neither Xcode nor a device
//! gets the notice naming both ways and the setting. `ios_devices` answers the inventory, the
//! mode and the target a view without `iosDevice` gets, or why there is none; it boots nothing.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, McpCoverage,
};
use rb_config::{EngineKind, PresetFile};
use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_safari::ios::{Inventory, NoTarget, resolve, take_inventory};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{config_of, for_host, valid_device};
use crate::breakpoint_keys::EngineItem;
use crate::control::Core;

/// The command.
pub const DEVICES: &str = "ios.devices";
/// The MCP host tool.
pub const DEVICES_TOOL: &str = "ios_devices";
/// The text of a device row; `{device}`.
pub const ROW_KEY: &str = "engines.ios.pick.row";
/// The notice while the inventory is taken.
pub const LISTING_KEY: &str = "engines.ios.listing";
/// The most rows the menu lists.
pub const MAX_ROWS: usize = 24;

/// `ios.devices`' arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Args {
    /// The view; the active view when absent.
    #[serde(default, alias = "viewId")]
    pub view: Option<ViewId>,
    /// The model, simulator or device to run on; absent opens the menu.
    #[serde(default)]
    pub device: Option<String>,
    /// `local` (the default) or `project`: the config file written to.
    #[serde(default)]
    pub file: Option<PresetFile>,
}

/// The definition: Breakpoints group, `Leader B I`, MCP `ios_devices`.
///
/// # Errors
///
/// The id or the binding does not parse (a test holds them).
pub fn definition() -> Result<CommandDef, CommandError> {
    Ok(CommandDef {
        id: CommandId::new(DEVICES)?,
        title_key: format!("commands.{DEVICES}"),
        group: Group::Breakpoints,
        default_bindings: vec!["Ctrl+Shift+Space B I".parse::<Binding>()?],
        mcp: McpCoverage::Covered(DEVICES_TOOL.to_owned()),
    })
}

/// Registers the command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let def = match definition() {
        Ok(def) => def,
        Err(error) => return vec![error],
    };
    let weak: Weak<Core> = Arc::downgrade(core);
    let handler: Handler = Arc::new(move |args: Value| {
        let core = weak
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &args)
    });
    registry.register(def, handler).err().into_iter().collect()
}

/// The arguments of `args`.
pub(crate) fn parse(args: &Value) -> Result<Args, CommandError> {
    if args.is_null() {
        return Ok(Args::default());
    }
    let args: Args = serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    if let Some(device) = &args.device
        && !valid_device(device)
    {
        return Err(CommandError::InvalidArgs(
            "device is a model, a simulator's or a device's UDID (at most 80 characters)"
                .to_owned(),
        ));
    }
    Ok(args)
}

/// Runs `ios.devices`: the pick with `device`, else the menu after the inventory.
fn run(core: &Arc<Core>, args: &Value) -> Result<Value, CommandError> {
    let args = parse(args)?;
    if let Some(device) = args.device {
        let args = json!({
            "view": args.view,
            "engine": "ios",
            "iosDevice": device.trim(),
            "file": args.file,
        });
        return crate::engines::set_engine::run(core, &args);
    }
    let tools = for_host(core.host()).and_then(|engine| engine.ios_tools());
    let Some(tools) = tools else {
        core.notify(notice(
            NotificationLevel::Warning,
            "engines.ios.onlyMacos",
            None,
        ));
        return Ok(json!({ "listing": false }));
    };
    // The inventory takes a moment (`system_profiler`): its start is named at once.
    core.notify(notice(NotificationLevel::Info, LISTING_KEY, None));
    let core = core.clone();
    let view = args.view;
    let runtime = core.runtime().clone();
    runtime.spawn(async move {
        let inventory = take_inventory(tools.as_ref()).await;
        if let Some(empty) = empty_notice(&inventory) {
            core.notify(empty);
            return;
        }
        // The engine menu of the view, its device rows from the inventory just taken.
        if let Err(error) = crate::engines::set_engine::run(&core, &json!({ "view": view })) {
            tracing::debug!(%error, "the engine menu did not open");
        }
    });
    Ok(json!({ "listing": true }))
}

/// The notice for a Mac that offers no iOS view: neither way (both named), or Xcode without a
/// simulator; `None` when there is one.
pub fn empty_notice(inventory: &Inventory) -> Option<CoreToShell> {
    if !inventory.devices.is_empty() || !inventory.simulators.is_empty() {
        return None;
    }
    let refusal = if inventory.xcode {
        NoTarget::NoSimulator
    } else {
        NoTarget::NoWay
    };
    Some(notice(
        NotificationLevel::Warning,
        refusal.key(),
        Some(refusal.setting()),
    ))
}

fn notice(level: NotificationLevel, key: &str, setting: Option<&str>) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: Default::default(),
        settings_key: setting.map(str::to_owned),
    })
}

/// The menu's rows under "iOS": one per simulator model (its newest runtime), then one per
/// device, at most [`MAX_ROWS`]; `current` is the view's `iosDevice` (checked while `on_ios`),
/// `reason` disables every row.
pub fn device_items(
    inventory: &Inventory,
    current: Option<&str>,
    on_ios: bool,
    reason: Option<String>,
) -> Vec<EngineItem> {
    let mut seen: Vec<&str> = Vec::new();
    let simulators = inventory.simulators.iter().filter_map(|sim| {
        if seen.contains(&sim.name.as_str()) {
            return None;
        }
        seen.push(&sim.name);
        Some((sim.label(), sim.name.clone()))
    });
    let picks: Vec<(String, String)> = simulators
        .collect::<Vec<_>>()
        .into_iter()
        .chain(
            inventory
                .devices
                .iter()
                .map(|device| (device.label(), device.udid.clone())),
        )
        .take(MAX_ROWS)
        .collect();
    picks
        .into_iter()
        .map(|(label, pick)| EngineItem {
            engine: EngineKind::Ios,
            label: rb_i18n::t_with(ROW_KEY, &[("device", &label)]),
            checked: on_ios && current.is_some_and(|current| current.eq_ignore_ascii_case(&pick)),
            reason: reason.clone(),
            ios_device: Some(pick),
        })
        .collect()
}

/// `items` (the engine menu of `view`) with the device rows under the iOS row, from the last
/// inventory; unchanged before the first or without an iOS row.
pub fn with_device_rows(core: &Core, view: ViewId, mut items: Vec<EngineItem>) -> Vec<EngineItem> {
    let Some(inventory) = rb_safari::ios::last_inventory() else {
        return items;
    };
    let Some(at) = items
        .iter()
        .position(|item| item.engine == EngineKind::Ios && item.ios_device.is_none())
    else {
        return items;
    };
    let host = core.host();
    let Some(info) = host.view(view) else {
        return items;
    };
    let config = host.config();
    let current = crate::engines::set_engine::breakpoint_for(&config, &info)
        .and_then(|id| config.breakpoints.iter().find(|bp| bp.id == id).cloned())
        .and_then(|breakpoint| breakpoint.ios_device);
    let on_ios = info.engine == EngineKind::Ios;
    let reason = items[at].reason.clone();
    let rows = device_items(&inventory, current.as_deref(), on_ios, reason);
    items.splice(at + 1..at + 1, rows);
    items
}

/// MCP `ios_devices`: Xcode, the simulators and devices (with the UDIDs `iosDevice` takes), the
/// mode, and the target a view without `iosDevice` gets - or the reason, its code and setting.
///
/// # Errors
///
/// None so far: outside macOS the answer says `supported: false`.
pub async fn devices_tool(core: &Arc<Core>) -> Result<Value, rb_mcp::ToolFailure> {
    let settings = core.user_settings().engines.ios.clone();
    let tools = for_host(core.host()).and_then(|engine| engine.ios_tools());
    let Some(tools) = tools else {
        return Ok(json!({
            "supported": false,
            "reason": rb_i18n::t("engines.ios.onlyMacos"),
            "xcode": false,
            "simulators": [],
            "devices": [],
        }));
    };
    let inventory = take_inventory(tools.as_ref()).await;
    let mut answer = inventory.to_json();
    answer["supported"] = json!(true);
    answer["mode"] = json!(settings.mode);
    answer["defaultSimulator"] = json!(settings.default_simulator);
    match resolve(&config_of(&settings), &inventory, None) {
        Ok(target) => {
            answer["default"] = json!({
                "udid": target.udid(),
                "label": target.label(),
                "simulator": target.is_simulator(),
            });
        }
        Err(refusal) => {
            answer["reason"] = json!(refusal.reason());
            answer["code"] = json!(refusal.key());
            answer["setting"] = json!(refusal.setting());
        }
    }
    Ok(answer)
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
