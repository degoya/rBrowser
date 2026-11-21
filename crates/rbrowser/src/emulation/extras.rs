//! The commands of the emulation extras (RB-080-10): geolocation, time zone, CPU throttling and
//! offline for the target views (the active one by default, `target: "all"` for Shift-click),
//! and `view.add_print`, a print view on A4 or Letter beside the grid. Without a value a
//! command toggles: the active view's extra off when it has one, else the user's default
//! (Settings > Emulation > Geolocation, time zone and CPU). Each registers itself with one
//! leader chord; MCP reaches the extras through `emulation_set` and a print breakpoint through
//! `breakpoint_add`.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_config::{
    Breakpoint, DEFAULT_SESSION, EmulationFields, EmulationUserSettings, EngineKind, Geolocation,
    MediaFeatures, MediaType, Paper, geolocation_preset,
};
use rb_engine::{Emulation, Target, ViewHost, ViewId};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

use super::{change, failed};
use crate::control::Core;
use rb_util::url::origin_of;

/// Set or toggle the position.
pub const GEOLOCATION: &str = "view.emulate.geolocation";
/// Set or toggle the time zone.
pub const TIMEZONE: &str = "view.emulate.timezone";
/// Set or toggle the CPU throttling.
pub const CPU: &str = "view.emulate.cpu";
/// Set or toggle offline.
pub const OFFLINE: &str = "view.emulate.offline";
/// Open a print view.
pub const ADD_PRINT: &str = "view.add_print";

/// The MCP tool of the four emulation commands.
const MCP_EMULATION: &str = "emulation_set";
/// The MCP tool that adds a print breakpoint to the config.
const MCP_BREAKPOINT_ADD: &str = "breakpoint_add";

/// id, the keys after the leader (`V` group: G, T, H for "hertz", L for "line" - C, O and P are
/// taken; the print view has Y, as D is the external windows' arrange), MCP tool.
const COMMANDS: [(&str, [char; 2], &str); 5] = [
    (GEOLOCATION, ['V', 'G'], MCP_EMULATION),
    (TIMEZONE, ['V', 'T'], MCP_EMULATION),
    (CPU, ['V', 'H'], MCP_EMULATION),
    (OFFLINE, ['V', 'L'], MCP_EMULATION),
    (ADD_PRINT, ['V', 'Y'], MCP_BREAKPOINT_ADD),
];

/// The definitions, title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    COMMANDS
        .iter()
        .map(|(id, keys, tool)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::View,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: keys.iter().map(|key| Key::Char(*key)).collect(),
                })],
                mcp: McpCoverage::Covered((*tool).to_owned()),
            })
        })
        .collect()
}

/// Registers every command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(def.id.as_str(), Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(id: &str, core: Weak<Core>) -> Handler {
    let id = id.to_owned();
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &id, &args)
    })
}

/// Runs the command `id` of this module.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    if id == ADD_PRINT {
        return add_print(core, args);
    }
    let target = match args.get("target") {
        Some(target) => Some(
            serde_json::from_value::<Target>(target.clone())
                .map_err(|error| CommandError::InvalidArgs(error.to_string()))?,
        ),
        None => None,
    };
    let host = core.host();
    let views = host.target_views(target).map_err(failed)?;
    let current = host
        .active_view()
        .filter(|active| views.contains(active))
        .or_else(|| views.first().copied())
        .and_then(|view| host.emulation(view))
        .unwrap_or_default();
    let fields = fields_for(id, args, &current, &core.user_settings().emulation)?;
    change(core, views, |host, view| {
        host.set_emulation_fields(view, &fields)
    })
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeolocationArgs {
    #[serde(default)]
    #[allow(
        dead_code,
        reason = "read by `run`; declared so it is no unknown field"
    )]
    target: Option<Value>,
    /// A position, or `null` for none.
    #[serde(default, deserialize_with = "explicit_null")]
    geolocation: Option<Option<Geolocation>>,
    /// A preset id (built in or the user's).
    #[serde(default)]
    preset: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TimezoneArgs {
    #[serde(default)]
    #[allow(
        dead_code,
        reason = "read by `run`; declared so it is no unknown field"
    )]
    target: Option<Value>,
    /// An IANA name; empty for the system's.
    #[serde(default)]
    timezone: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CpuArgs {
    #[serde(default)]
    #[allow(
        dead_code,
        reason = "read by `run`; declared so it is no unknown field"
    )]
    target: Option<Value>,
    /// The factor; 1 is none.
    #[serde(default)]
    rate: Option<f64>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OfflineArgs {
    #[serde(default)]
    #[allow(
        dead_code,
        reason = "read by `run`; declared so it is no unknown field"
    )]
    target: Option<Value>,
    #[serde(default)]
    offline: Option<bool>,
}

/// The fields command `id` sets with `args`: the value given, else the toggle - off when the
/// reference view (`current`) has the extra, else the user's default from `settings`. Values
/// are checked when the views resolve them.
pub fn fields_for(
    id: &str,
    args: &Value,
    current: &Emulation,
    settings: &EmulationUserSettings,
) -> Result<EmulationFields, CommandError> {
    let mut fields = EmulationFields::default();
    match id {
        GEOLOCATION => {
            let args: GeolocationArgs = parse(args)?;
            let preset = |id: &str| {
                geolocation_preset(settings, id)
                    .ok_or_else(|| CommandError::InvalidArgs(format!("no geolocation preset {id}")))
            };
            fields.geolocation = Some(match (args.geolocation, args.preset) {
                (Some(_), Some(_)) => {
                    return Err(CommandError::InvalidArgs(
                        "give a geolocation or a preset, not both".to_owned(),
                    ));
                }
                (Some(position), None) => position,
                (None, Some(id)) => Some(preset(&id)?),
                (None, None) if current.geolocation.is_some() => None,
                (None, None) => Some(preset(&settings.default_geolocation)?),
            });
        }
        TIMEZONE => {
            let args: TimezoneArgs = parse(args)?;
            fields.timezone = Some(match args.timezone {
                Some(timezone) => timezone,
                None if current.timezone.is_some() => String::new(),
                None => settings.default_timezone.clone(),
            });
        }
        CPU => {
            let args: CpuArgs = parse(args)?;
            fields.cpu_throttling = Some(match args.rate {
                Some(rate) => rate,
                None if current.cpu_throttling.is_some() => 1.0,
                None => settings.default_cpu_throttling,
            });
        }
        OFFLINE => {
            let args: OfflineArgs = parse(args)?;
            let offline = current.network.is_some_and(|network| network.offline);
            fields.offline = Some(args.offline.unwrap_or(!offline));
        }
        other => return Err(CommandError::UnknownCommand(other.to_owned())),
    }
    Ok(fields)
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrintArgs {
    /// `A4` or `Letter`; the project's `emulation.paper`, else A4.
    #[serde(default)]
    paper: Option<Paper>,
    /// The view whose page and session the print view shows; the active view by default.
    #[serde(default)]
    view: Option<ViewId>,
}

/// `view.add_print`: a print view (print media, as wide as the paper) beside the grid, in the
/// session of the source view and on its page. Answers `{ "view": <id> }`.
fn add_print(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args: PrintArgs = parse(args)?;
    let host = core.host();
    let paper = args
        .paper
        .or(host.config().emulation.paper)
        .unwrap_or(Paper::A4);
    let source = args
        .view
        .or_else(|| host.active_view())
        .or_else(|| host.views().first().map(|info| info.id))
        .and_then(|view| host.view(view));
    let session = source
        .as_ref()
        .map(|info| info.session.clone())
        .filter(|session| session != DEFAULT_SESSION);
    let view = host
        .open_view(&print_breakpoint(paper, session.as_deref()))
        .map_err(failed)?;
    if let Some(url) = source
        .map(|info| info.url)
        .filter(|url| origin_of(url).is_some())
    {
        host.navigate(Target::View(view), &url).map_err(failed)?;
    }
    Ok(json!({ "view": view }))
}

/// The breakpoint of a print view on `paper` (`print-a4`, `print-letter`) in `session`.
pub fn print_breakpoint(paper: Paper, session: Option<&str>) -> Breakpoint {
    Breakpoint {
        id: format!("print-{}", paper.id()),
        label: None,
        width: None,
        height: None,
        device: None,
        engine: EngineKind::Chromium,
        ios_device: None,
        session: session.map(str::to_owned),
        sync_navigation: true,
        shortcut: None,
        color: None,
        emulation: EmulationFields {
            media: Some(MediaFeatures {
                media_type: Some(MediaType::Print),
                ..MediaFeatures::default()
            }),
            paper: Some(paper),
            ..EmulationFields::default()
        },
    }
}

/// `args` as `T`; no arguments (a shortcut) is `T::default()`.
fn parse<T: DeserializeOwned + Default>(args: &Value) -> Result<T, CommandError> {
    if args.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

/// A field whose `null` (off) differs from its absence (toggle).
fn explicit_null<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[cfg(test)]
#[path = "extras_tests.rs"]
mod tests;
