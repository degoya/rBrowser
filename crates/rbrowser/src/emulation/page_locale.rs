//! The page's text direction and pseudo-localisation (RB-092-02): `emulation.toggle_dir` turns
//! the target views' `<html dir>` to right to left and back (`dir` is an emulation field, set
//! live like the others through `emulation.set`), and `overlays.pseudo_locale` pseudo-localises
//! the page's texts in the target views - on, off or as a toggle, with the user's
//! `emulation.pseudoLocale` where the call gives no values. Both default to the active view;
//! `target: "all"` is the Shift-click of RB-010-05. The sync agent does the work in the page
//! (`agent/src/locale/`); Firefox and WebKit views answer as `unsupported`.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_config::{
    EmulationFields, EngineKind, PSEUDO_FACTOR_RANGE, PseudoLocaleSettings, TextDirection,
    clamp_pseudo_factor,
};
use rb_engine::{Target, ViewHost, ViewId};
use rb_sync::PseudoLocale;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{change, failed};
use crate::control::Core;

/// Right to left and back.
pub const TOGGLE_DIR: &str = "emulation.toggle_dir";
/// The pseudo-localisation of the page on, off or toggled.
pub const PSEUDO_LOCALE: &str = "overlays.pseudo_locale";

/// id, group, the keys after the leader, MCP tool. `V .` - every letter of the `V` group is
/// taken; `O Y` - the job's `O P` is the overlays plugin's, `O U` the container queries'.
const COMMANDS: [(&str, Group, [char; 2], &str); 2] = [
    (TOGGLE_DIR, Group::View, ['V', '.'], "emulation_set"),
    (PSEUDO_LOCALE, Group::Overlays, ['O', 'Y'], "pseudo_locale"),
];

/// The definitions, title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    COMMANDS
        .iter()
        .map(|(id, group, keys, tool)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: *group,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: keys.iter().map(|key| Key::Char(*key)).collect(),
                })],
                mcp: McpCoverage::Covered((*tool).to_owned()),
            })
        })
        .collect()
}

/// Registers both commands with `registry`; returns what the registry refused.
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
    match id {
        TOGGLE_DIR => toggle_dir(core, args),
        PSEUDO_LOCALE => pseudo_locale(core, args),
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
}

/// The arguments of `emulation.toggle_dir`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirArgs {
    /// `"all"` or a view id; the active view when absent.
    #[serde(default)]
    target: Option<Target>,
    /// The direction to set; without, the reference view's flips (right to left and back).
    #[serde(default)]
    dir: Option<TextDirection>,
}

/// The arguments of `overlays.pseudo_locale` (and the MCP tool `pseudo_locale`).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PseudoArgs {
    /// `"all"` or a view id; the active view when absent.
    #[serde(default)]
    pub target: Option<Target>,
    /// On or off; without, the reference view's flips.
    #[serde(default)]
    pub on: Option<bool>,
    /// The length factor, 1 to 3; the user's setting when absent.
    #[serde(default)]
    pub factor: Option<f64>,
    /// Accented letters; the user's setting when absent.
    #[serde(default)]
    pub accents: Option<bool>,
    /// Right to left; the user's setting when absent.
    #[serde(default)]
    pub rtl: Option<bool>,
}

/// `args` as `T`; no arguments (a shortcut) is `T::default()`.
fn parse<T: DeserializeOwned + Default>(args: &Value) -> Result<T, CommandError> {
    if args.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

/// The view a toggle reads: the active one among `views`, else the first.
fn reference(core: &Core, views: &[ViewId]) -> Option<ViewId> {
    let host = core.host();
    host.active_view()
        .filter(|active| views.contains(active))
        .or_else(|| views.first().copied())
}

fn toggle_dir(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args: DirArgs = parse(args)?;
    let views = core.host().target_views(args.target).map_err(failed)?;
    let current = reference(core, &views)
        .and_then(|view| core.host().emulation(view))
        .and_then(|emulation| emulation.dir);
    let fields = EmulationFields {
        dir: Some(args.dir.unwrap_or_else(|| TextDirection::toggled(current))),
        ..EmulationFields::default()
    };
    change(core, views, |host, view| {
        host.set_emulation_fields(view, &fields)
    })
}

/// The pseudo-localisation `args` ask for: on or off (without `on`, the opposite of `current`,
/// the reference view's), each value from `args`, else from `settings`. A factor outside 1 to 3
/// is refused.
pub fn pseudo_for(
    args: &PseudoArgs,
    current: Option<PseudoLocale>,
    settings: &PseudoLocaleSettings,
) -> Result<Option<PseudoLocale>, CommandError> {
    if let Some(factor) = args.factor
        && !(factor.is_finite()
            && (PSEUDO_FACTOR_RANGE.0..=PSEUDO_FACTOR_RANGE.1).contains(&factor))
    {
        return Err(CommandError::InvalidArgs(
            "factor is a number from 1 to 3".to_owned(),
        ));
    }
    if !args.on.unwrap_or(current.is_none()) {
        return Ok(None);
    }
    Ok(Some(PseudoLocale {
        factor: clamp_pseudo_factor(args.factor.unwrap_or(settings.factor)),
        accents: args.accents.unwrap_or(settings.accents),
        rtl: args.rtl.unwrap_or(settings.rtl),
    }))
}

/// `overlays.pseudo_locale`: answers `{ on, views, unsupported }` - the Chromium views it
/// changed, and the Firefox and WebKit views it leaves (not in scope, as RB-080-10's extras).
fn pseudo_locale(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args: PseudoArgs = parse(args)?;
    let host = core.host();
    let views = host.target_views(args.target).map_err(failed)?;
    let agents = host.sync_agents().clone();
    let current = reference(core, &views).and_then(|view| agents.page_locale(view).pseudo);
    let pseudo = pseudo_for(
        &args,
        current,
        &core.user_settings().emulation.pseudo_locale,
    )?;
    let (chromium, unsupported): (Vec<ViewId>, Vec<ViewId>) = views
        .into_iter()
        .partition(|view| host.view(*view).map(|info| info.engine) == Some(EngineKind::Chromium));
    let changed: Vec<ViewId> = chromium
        .iter()
        .copied()
        .filter(|view| agents.set_pseudo(*view, pseudo))
        .collect();
    if !changed.is_empty() {
        core.runtime().spawn(async move {
            for view in changed {
                agents.push_locale(view).await;
            }
        });
    }
    Ok(json!({
        "on": pseudo.is_some(),
        "views": chromium,
        "unsupported": unsupported,
    }))
}

#[cfg(test)]
#[path = "page_locale_tests.rs"]
mod tests;
