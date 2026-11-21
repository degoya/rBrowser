//! Emulation per view (RB-010-05): the CDP mapping of the full [`rb_engine::Emulation`]
//! (media features, device, locale, network, headers - those of `config.json` and
//! `config.local.json` only in a trusted workspace), `emulation.set` / `emulation.toggle_*` /
//! `emulation.reset_view` with [`rb_engine::EmulationSetArgs`], and the shell's `getEmulation`.
//! The host's metrics path (`host/attach.rs`, `host/state.rs::emulation_for`) is this job's in
//! wave 0.1-w1b.
//!
//! A view's emulation is its breakpoint resolved against the project defaults and the device
//! presets, with the live overrides of `emulation.set` on top until `emulation.reset_view`. The
//! host sends what changed, again after a reconnect, and lets the shell know when the page only
//! shows a change after a reload (user agent, language, headers).

pub mod cdp;
pub mod cdp_extras;
pub mod devices;
pub mod extras;
pub mod fields;
pub mod page_locale;

use std::collections::BTreeMap;

use rb_commands::CommandError;
use rb_config::{ColorScheme, EmulationFields};
use rb_engine::{EmulationChange, EmulationSetArgs, EngineError, Target, ViewHost, ViewId};
use rb_ipc::{CoreToShell, IpcErrorCode, Notification, NotificationLevel, ShellToCore};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::control::Core;
use crate::host::Host;
use crate::ipc::Failure;

pub use devices::{device_list, set_user_devices};

/// The i18n key of the notice that a change shows after a reload; placeholder `{count}`.
pub const RELOAD_REQUIRED_KEY: &str = "emulation.notice.reloadRequired";
/// The settings category `emulation.set` opens from its shortcut.
pub const SETTINGS_KEY: &str = "emulation";

/// The arguments of the toggles and `emulation.reset_view`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TargetArgs {
    /// `"all"` or a view id; the active view when absent.
    #[serde(default)]
    target: Option<Target>,
}

/// Runs the command `id` when it is this module's (`emulation.*`); `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    let result = match id {
        "emulation.set" => set(core, args),
        "emulation.toggle_color_scheme" => toggle(core, args, Toggle::ColorScheme),
        "emulation.toggle_reduced_motion" => toggle(core, args, Toggle::ReducedMotion),
        "emulation.reset_view" => target_of(args).and_then(|target| {
            let views = core.host().target_views(target).map_err(failed)?;
            change(core, views, |host, view| host.reset_emulation(view))
        }),
        _ => return None,
    };
    Some(result)
}

/// Answers `message` when it is this module's (`getEmulation`); `None` otherwise. Header values
/// are masked.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let ShellToCore::GetEmulation { view } = message else {
        return None;
    };
    Some(match core.host().emulation(*view) {
        Some(emulation) => Ok(CoreToShell::Emulation {
            view: *view,
            emulation: fields::masked(&emulation),
        }),
        None => Err((IpcErrorCode::NotFound, format!("unknown view {view}"))),
    })
}

/// The core is up: the user's device presets, then every view resolved against them.
pub fn on_start(core: &Core) {
    on_user_settings_changed(core);
}

/// The user settings changed (their `devices` may have): resolve every view again.
pub fn on_user_settings_changed(core: &Core) {
    set_user_devices(&core.user_settings().devices);
    core.host().refresh_emulation();
}

/// The project config was read again: its defaults and headers apply.
pub fn on_config_changed(core: &Core) {
    core.host().refresh_emulation();
}

/// `emulation.set`: the fields over the target views. From a shortcut (no arguments) it opens
/// Settings > Emulation, where the fields are chosen. `fields.colorScheme: null` drops the live
/// colour scheme override - the view is back at its breakpoint's (RB-080-49) - and
/// `fields.dir: null` / `fields.lang: null` the live text direction / language (RB-092-02);
/// every other absent or `null` field stays as it is.
fn set(core: &Core, args: &Value) -> Result<Value, CommandError> {
    if args.is_null() {
        core.run_command("settings.open", json!({ "key": SETTINGS_KEY }))?;
        return Ok(Value::Null);
    }
    let cleared = |field: &str| {
        args.pointer(&format!("/fields/{field}"))
            .is_some_and(Value::is_null)
    };
    let (no_scheme, no_dir, no_lang) = (cleared("colorScheme"), cleared("dir"), cleared("lang"));
    let args: EmulationSetArgs = serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    // A header set live may be a token: never in a log from here on.
    for value in args.fields.headers.values() {
        rb_diag::register_secret(value);
    }
    let views = core.host().target_views(args.target).map_err(failed)?;
    change(core, views, |host, view| {
        let mut reload = host.set_emulation_fields(view, &args.fields)?;
        if no_scheme {
            reload |= host.clear_color_scheme(view)?;
        }
        if no_dir || no_lang {
            reload |= host.clear_text_direction(view, no_dir, no_lang)?;
        }
        Ok(reload)
    })
}

#[derive(Clone, Copy)]
enum Toggle {
    ColorScheme,
    ReducedMotion,
}

/// A toggle flips the active view's value (or the first target's) and sets the result on every
/// target, so `target: "all"` leaves them all alike.
fn toggle(core: &Core, args: &Value, which: Toggle) -> Result<Value, CommandError> {
    let target = target_of(args)?;
    let host = core.host();
    let views = host.target_views(target).map_err(failed)?;
    let reference = host
        .active_view()
        .filter(|active| views.contains(active))
        .or_else(|| views.first().copied())
        .and_then(|view| host.emulation(view))
        .unwrap_or_default();
    let fields = match which {
        Toggle::ColorScheme => EmulationFields {
            color_scheme: Some(match reference.color_scheme {
                Some(ColorScheme::Dark) => ColorScheme::Light,
                _ => ColorScheme::Dark,
            }),
            ..EmulationFields::default()
        },
        Toggle::ReducedMotion => EmulationFields {
            reduced_motion: Some(reference.reduced_motion != Some(true)),
            ..EmulationFields::default()
        },
    };
    change(core, views, |host, view| {
        host.set_emulation_fields(view, &fields)
    })
}

/// Applies `apply` to each of `views`; the answer is [`EmulationChange`], and a change the page
/// shows only after a reload becomes a notice.
fn change(
    core: &Core,
    views: Vec<ViewId>,
    apply: impl Fn(&Host, ViewId) -> Result<bool, EngineError>,
) -> Result<Value, CommandError> {
    let mut reload_required = false;
    for view in &views {
        reload_required |= apply(core.host(), *view).map_err(failed)?;
    }
    if reload_required {
        core.notify(CoreToShell::Notification(Notification {
            level: NotificationLevel::Info,
            message_key: RELOAD_REQUIRED_KEY.to_owned(),
            args: BTreeMap::from([("count".to_owned(), views.len().to_string())]),
            settings_key: None,
        }));
    }
    serde_json::to_value(EmulationChange {
        views,
        reload_required,
    })
    .map_err(|error| CommandError::Failed(error.to_string()))
}

fn target_of(args: &Value) -> Result<Option<Target>, CommandError> {
    if args.is_null() {
        return Ok(None);
    }
    serde_json::from_value::<TargetArgs>(args.clone())
        .map(|args| args.target)
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

fn failed(error: EngineError) -> CommandError {
    match error {
        EngineError::UnknownView(_) | EngineError::Invalid(_) => {
            CommandError::InvalidArgs(error.to_string())
        }
        other => CommandError::Failed(other.to_string()),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
