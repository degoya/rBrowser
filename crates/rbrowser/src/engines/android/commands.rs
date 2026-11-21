//! The commands of Android views (RB-095-02) and their MCP host tools.
//!
//! | Command | Default | MCP |
//! | --- | --- | --- |
//! | `view.add_android { serial? }`: the device as a view at its own size | `Leader V ;` (the job's `V J` is the JSON viewer's `toggle_raw`) | `view_add_android` |
//! | `tools.adb.install`: download the Android Platform Tools | `Leader X S` | omitted: the person's consent |
//!
//! `android_devices` lists the devices adb sees; it runs no command. The command answers at
//! once and ends in a notice (adb may take seconds; nothing waits on the UI thread); the tool
//! awaits the view. Every refusal names its reason and the setting that decides it
//! (`engines.android.enabled`, `engines.android.adbPath`); without adb the consent notice of
//! the download is shown as well.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, McpCoverage,
};
use rb_config::EngineKind;
use rb_ipc::NotificationLevel;
use serde::Deserialize;
use serde_json::{Value, json};

use super::adb;
use super::adb_tool::{self, PATH_SETTING};
use super::devices::{Device, breakpoint_id, valid_serial};
use super::engine::ready_device;
use super::locate::Found;
use crate::breakpoint_keys::FreeSize;
use crate::control::Core;

/// Add a device as a view.
pub const ADD: &str = "view.add_android";
/// Download the Android Platform Tools.
pub const INSTALL: &str = "tools.adb.install";
/// The MCP tool of [`ADD`].
pub const ADD_TOOL: &str = "view_add_android";
/// The MCP tool listing the devices.
pub const DEVICES_TOOL: &str = "android_devices";
/// The setting that switches Android views on.
pub const ENABLED_SETTING: &str = "engines.android.enabled";
/// Why the download has no MCP tool.
pub const INSTALL_OMITTED: &str =
    "protected: downloads a program; only the person, after the consent notice";
/// A view could not be added; `{error}`.
pub const ADD_FAILED_KEY: &str = "engines.android.notice.add_failed";

/// Id, group, default binding, MCP tool (`Ok`) or why there is none.
const ROWS: [(&str, Group, &str, Result<&str, &str>); 2] = [
    (ADD, Group::View, "Ctrl+Shift+Space V ;", Ok(ADD_TOOL)),
    (
        INSTALL,
        Group::Plugins,
        "Ctrl+Shift+Space X S",
        Err(INSTALL_OMITTED),
    ),
];

/// `view.add_android`'s arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddArgs {
    /// The device's serial (`android_devices`); the one ready device when absent.
    #[serde(default)]
    pub serial: Option<String>,
}

/// Why a view was not added: a stable code, the text, the setting that decides it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// `android.disabled`, `android.no_adb`, `android.no_device`, ...
    pub code: &'static str,
    /// For the person.
    pub message: String,
    /// The setting the notice links.
    pub setting: Option<&'static str>,
}

impl From<Refusal> for rb_mcp::ToolFailure {
    fn from(refusal: Refusal) -> Self {
        let failure = rb_mcp::ToolFailure::new(refusal.code, refusal.message);
        match refusal.setting {
            Some(key) => failure.with_setting(key),
            None => failure,
        }
    }
}

/// The definitions; title key `commands.<id>`.
///
/// # Errors
///
/// An id or binding that does not parse (a test holds them).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, group, binding, mcp)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: *group,
                default_bindings: vec![binding.parse::<Binding>()?],
                mcp: match mcp {
                    Ok(tool) => McpCoverage::Covered((*tool).to_owned()),
                    Err(reason) => McpCoverage::Omitted((*reason).to_owned()),
                },
            })
        })
        .collect()
}

/// Registers the commands with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let id = def.id.as_str().to_owned();
            let weak: Weak<Core> = Arc::downgrade(core);
            let handler: Handler = Arc::new(move |args: Value| {
                let core = weak
                    .upgrade()
                    .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
                run(&core, &id, &args)
            });
            registry.register(def, handler).err()
        })
        .collect()
}

/// Runs the command `id`: answers at once, the work goes on in the background.
fn run(core: &Arc<Core>, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        ADD => {
            let args: AddArgs = if args.is_null() {
                AddArgs::default()
            } else {
                serde_json::from_value(args.clone())
                    .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
            };
            let runner = core.clone();
            core.runtime().spawn(async move {
                if let Err(refusal) = add(&runner, args.serial).await {
                    tracing::info!(code = refusal.code, "no Android view was added");
                    let mut notice = adb_tool::notice(
                        NotificationLevel::Error,
                        ADD_FAILED_KEY,
                        &[("error", refusal.message.clone())],
                    );
                    if let rb_ipc::CoreToShell::Notification(notification) = &mut notice {
                        notification.settings_key = refusal.setting.map(ToOwned::to_owned);
                    }
                    runner.notify(notice);
                }
            });
            Ok(json!({ "started": true }))
        }
        INSTALL => {
            let notifier = core.clone();
            core.runtime().spawn_blocking(move || {
                let _ = adb_tool::install_default(&|message| notifier.notify(message));
            });
            Ok(json!({ "started": true }))
        }
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
}

/// The device of `serial`, or the one ready device; else why not.
///
/// # Errors
///
/// A bad serial, no such device, one not ready, none or several ready without a serial.
pub fn pick<'a>(serial: Option<&str>, devices: &'a [Device]) -> Result<&'a Device, Refusal> {
    let refused = |code, message| Refusal {
        code,
        message,
        setting: None,
    };
    if let Some(serial) = serial {
        if !valid_serial(serial) {
            return Err(refused(
                "invalid_arguments",
                format!("{serial:?} is no serial of a USB device"),
            ));
        }
        return ready_device(&breakpoint_id(serial), devices).map_err(|message| {
            let present = devices.iter().any(|device| device.serial == serial);
            let code = if present {
                "android.device_not_ready"
            } else {
                "android.no_device"
            };
            refused(code, message)
        });
    }
    let ready: Vec<&Device> = devices.iter().filter(|device| device.is_ready()).collect();
    match ready.as_slice() {
        [one] => Ok(one),
        [] => match devices.first() {
            Some(device) => ready_device(&breakpoint_id(&device.serial), devices)
                .map_err(|message| refused("android.device_not_ready", message)),
            None => Err(refused(
                "android.no_device",
                no_device_message(cfg!(windows)),
            )),
        },
        several => {
            let serials: Vec<&str> = several
                .iter()
                .map(|device| device.serial.as_str())
                .collect();
            Err(refused(
                "invalid_arguments",
                rb_i18n::t_with(
                    "engines.android.several",
                    &[("serials", &serials.join(", "))],
                ),
            ))
        }
    }
}

/// Why no device is listed, with what brings a connected phone to adb (RB-096-10): the cable
/// and its USB mode "File transfer", USB debugging, on `windows` the maker's USB driver, and
/// Samsung's Auto Blocker, which hides the phone's ADB side.
pub fn no_device_message(windows: bool) -> String {
    let mut keys = vec!["engines.android.noDevice", "engines.android.noDeviceUsb"];
    if windows {
        keys.push("engines.android.noDeviceDriver");
    }
    keys.push("engines.android.noDeviceSamsung");
    keys.into_iter()
        .map(rb_i18n::t)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Adds the device `serial` (the one ready device when `None`) as a view at its own size.
///
/// # Errors
///
/// Android is off, no adb, no device ready, adb failed, or the view could not be opened.
pub async fn add(core: &Arc<Core>, serial: Option<String>) -> Result<Value, Refusal> {
    let engine = super::for_host(core.host()).ok_or_else(|| Refusal {
        code: "android.unavailable",
        message: "the window's engines have not started".to_owned(),
        setting: None,
    })?;
    if !engine.is_enabled() {
        return Err(Refusal {
            code: "android.disabled",
            message: rb_i18n::t("engines.android.disabled"),
            setting: Some(ENABLED_SETTING),
        });
    }
    let runner = engine.adb().map_err(|message| {
        offer_download(core, &message);
        Refusal {
            code: "android.no_adb",
            message,
            setting: Some(PATH_SETTING),
        }
    })?;
    let failed = |error: adb::AdbError| Refusal {
        code: "android.adb_failed",
        message: error.to_string(),
        setting: Some(PATH_SETTING),
    };
    let devices = adb::devices(runner.as_ref()).await.map_err(failed)?;
    // The menus list what this listing found (RB-094-20).
    super::menu::store_devices(core, &devices);
    let device = pick(serial.as_deref(), &devices)?.clone();
    let screen = adb::screen(runner.as_ref(), &device.serial)
        .await
        .map_err(failed)?;
    let (width, height) = screen.css_size();
    let size = FreeSize::new(width, height, Some(screen.dpr)).ok_or_else(|| Refusal {
        code: "android.adb_failed",
        message: format!("the device reports no usable size ({width} x {height})"),
        setting: None,
    })?;
    let id = breakpoint_id(&device.serial);
    let label = rb_i18n::t_with("engines.android.viewLabel", &[("model", device.name())]);
    let blocking = core.clone();
    let mut answer = tokio::task::spawn_blocking(move || {
        crate::layout::view_edit::add_engine_view(&blocking, &id, size, EngineKind::Android, &label)
    })
    .await
    .map_err(|_| Refusal {
        code: "internal",
        message: "the view was not added".to_owned(),
        setting: None,
    })?
    .map_err(|error| Refusal {
        code: "command_failed",
        message: error.to_string(),
        setting: None,
    })?;
    answer["serial"] = json!(device.serial);
    Ok(answer)
}

/// Shows the consent notice of the download when a build is pinned for this platform.
fn offer_download(core: &Core, reason: &str) {
    if let Ok(entry) = crate::tools::adb_entry() {
        core.notify(adb_tool::needed_notice(&entry, reason));
    }
}

/// The answer of `android_devices`.
pub fn devices_answer(
    enabled: bool,
    adb: Option<&Found>,
    reason: Option<&str>,
    devices: &[Device],
) -> Value {
    let devices: Vec<Value> = devices
        .iter()
        .map(|device| {
            let mut entry = serde_json::to_value(device).unwrap_or_else(|_| json!({}));
            entry["ready"] = json!(device.is_ready());
            entry["breakpoint"] = json!(breakpoint_id(&device.serial));
            entry
        })
        .collect();
    let mut answer = json!({
        "enabled": enabled,
        "adb": adb.map(|found| json!({ "path": found.path, "source": found.source })),
        "devices": devices,
    });
    if let Some(reason) = reason {
        answer["reason"] = json!(reason);
    }
    answer
}

/// The MCP host tool `view_add_android { serial? }`.
///
/// # Errors
///
/// Arguments that do not read, or the refusal with its code and setting.
pub async fn add_tool(core: &Arc<Core>, args: Value) -> Result<Value, rb_mcp::ToolFailure> {
    let args: AddArgs = if args.is_null() {
        AddArgs::default()
    } else {
        serde_json::from_value(args)
            .map_err(|error| rb_mcp::ToolFailure::new("invalid_arguments", error.to_string()))?
    };
    add(core, args.serial).await.map_err(Into::into)
}

/// The MCP host tool `android_devices`: whether Android views are on, the adb in use and the
/// devices it sees; a read that never downloads or starts anything but adb itself.
///
/// # Errors
///
/// None so far: a missing adb or switched-off Android is answered as `reason`.
pub async fn devices_tool(core: &Arc<Core>) -> Result<Value, rb_mcp::ToolFailure> {
    let Some(engine) = super::for_host(core.host()) else {
        return Ok(devices_answer(false, None, Some("no window"), &[]));
    };
    let enabled = engine.is_enabled();
    if !enabled {
        let reason = rb_i18n::t("engines.android.disabled");
        return Ok(devices_answer(false, None, Some(&reason), &[]));
    }
    let found = match engine.locate_adb() {
        Ok(found) => found,
        Err(missing) => {
            let reason = super::engine::no_adb(&missing);
            return Ok(devices_answer(true, None, Some(&reason), &[]));
        }
    };
    let runner = match engine.adb() {
        Ok(runner) => runner,
        Err(reason) => return Ok(devices_answer(true, Some(&found), Some(&reason), &[])),
    };
    match adb::devices(runner.as_ref()).await {
        Ok(devices) => Ok(devices_answer(true, Some(&found), None, &devices)),
        Err(error) => Ok(devices_answer(
            true,
            Some(&found),
            Some(&error.to_string()),
            &[],
        )),
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
