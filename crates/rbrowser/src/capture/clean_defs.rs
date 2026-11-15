//! The commands of the capture preparation (RB-080-12), in the capture group:
//! `capture.toggle_hide_banners` (`Leader C H`) and `capture.toggle_freeze` (`Leader C N`, "no
//! motion"; `C F` is the full-page screenshot) turn hiding and freezing on or off for the session
//! (argument `{ "enabled"?: bool }`, a flip without it), over `capture.hideBanners` and
//! `capture.freezeAnimations`; `capture.preview_clean` (`Leader C P`) shows the preparation in
//! the active view until it is pressed again. MCP sets the defaults with `settings_set` and each
//! call with `hideBanners`/`freezeAnimations`, so the toggles and the preview have no tool. The
//! preview runs on the host's runtime, never on the UI thread.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_engine::{EngineError, ViewHost, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_visual::CleanOverrides;
use serde_json::{Value, json};

use super::clean::{self, Prepared};
use crate::cdp::lock;
use crate::control::Core;
use crate::emulation::cdp::media_params;

/// Hiding banners on or off for the session.
pub const TOGGLE_HIDE: &str = "capture.toggle_hide_banners";
/// Freezing animations on or off for the session.
pub const TOGGLE_FREEZE: &str = "capture.toggle_freeze";
/// The preparation shown in the active view, or ended.
pub const PREVIEW: &str = "capture.preview_clean";
/// Why the toggles have no MCP tool.
pub const TOGGLE_OMITTED: &str =
    "ui-only: session toggle; per MCP as hideBanners/freezeAnimations on each capture tool";
/// Why the preview has no MCP tool.
pub const PREVIEW_OMITTED: &str = "ui-only: a preview in the active view";

/// The notice: banners hidden for the session.
pub const HIDE_ON_KEY: &str = "capture.notice.hideBannersOn";
/// The notice: banners kept for the session.
pub const HIDE_OFF_KEY: &str = "capture.notice.hideBannersOff";
/// The notice: animations frozen for the session.
pub const FREEZE_ON_KEY: &str = "capture.notice.freezeOn";
/// The notice: animations running for the session.
pub const FREEZE_OFF_KEY: &str = "capture.notice.freezeOff";
/// The notice: the preview is shown.
pub const PREVIEW_ON_KEY: &str = "capture.notice.previewOn";
/// The notice: the preview ended.
pub const PREVIEW_OFF_KEY: &str = "capture.notice.previewOff";
/// The notice: the preview failed; placeholder `{error}`.
pub const PREVIEW_FAILED_KEY: &str = "capture.notice.previewFailed";

/// Id, the keys after the leader, why there is no tool.
const ROWS: [(&str, [char; 2], &str); 3] = [
    (TOGGLE_HIDE, ['C', 'H'], TOGGLE_OMITTED),
    (TOGGLE_FREEZE, ['C', 'N'], TOGGLE_OMITTED),
    (PREVIEW, ['C', 'P'], PREVIEW_OMITTED),
];

/// The preview shown now: its view and what to undo.
static SHOWN: Mutex<Option<(ViewId, Prepared)>> = Mutex::new(None);

/// The definitions; title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, keys, omitted)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Capture,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: keys.iter().map(|key| Key::Char(*key)).collect(),
                })],
                mcp: McpCoverage::Omitted((*omitted).to_owned()),
            })
        })
        .collect()
}

/// Registers the commands with `registry`; returns what the registry refused (a test holds it
/// empty).
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

fn notice(
    level: NotificationLevel,
    key: &str,
    setting: &str,
    args: &[(&str, String)],
) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect::<BTreeMap<_, _>>(),
        settings_key: Some(setting.to_owned()),
    })
}

/// `enabled` of `args`: `None` for a flip.
pub fn enabled(args: &Value) -> Result<Option<bool>, CommandError> {
    match args.get("enabled") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(on)) => Ok(Some(*on)),
        Some(_) => Err(CommandError::InvalidArgs(
            "`enabled` is true or false".to_owned(),
        )),
    }
}

/// The session's overrides after a toggle of hiding (`hide`) or freezing, from the value in
/// effect (`current`): `enabled`, else its opposite.
pub fn toggled(
    session: CleanOverrides,
    hide: bool,
    current: bool,
    enabled: Option<bool>,
) -> (CleanOverrides, bool) {
    let on = enabled.unwrap_or(!current);
    let mut next = session;
    if hide {
        next.hide_banners = Some(on);
    } else {
        next.freeze_animations = Some(on);
    }
    (next, on)
}

fn run(core: &Arc<Core>, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        TOGGLE_HIDE | TOGGLE_FREEZE => {
            let hide = id == TOGGLE_HIDE;
            let enabled = enabled(args)?;
            let session = super::session_clean();
            let settings = super::settings(core).clean;
            let current = if hide {
                session.hide_banners.unwrap_or(settings.hide_banners)
            } else {
                session
                    .freeze_animations
                    .unwrap_or(settings.freeze_animations)
            };
            let (next, on) = toggled(session, hide, current, enabled);
            super::set_session_clean(next);
            let (key, setting) = match (hide, on) {
                (true, true) => (HIDE_ON_KEY, "capture.hideBanners"),
                (true, false) => (HIDE_OFF_KEY, "capture.hideBanners"),
                (false, true) => (FREEZE_ON_KEY, "capture.freezeAnimations"),
                (false, false) => (FREEZE_OFF_KEY, "capture.freezeAnimations"),
            };
            core.notify(notice(NotificationLevel::Info, key, setting, &[]));
            let field = if hide {
                "hideBanners"
            } else {
                "freezeAnimations"
            };
            Ok(json!({ field: on }))
        }
        PREVIEW => {
            let runner = core.clone();
            core.host().runtime().spawn(async move {
                let message = match preview(&runner).await {
                    Ok(true) => notice(NotificationLevel::Info, PREVIEW_ON_KEY, "capture", &[]),
                    Ok(false) => notice(NotificationLevel::Info, PREVIEW_OFF_KEY, "capture", &[]),
                    Err(error) => notice(
                        NotificationLevel::Error,
                        PREVIEW_FAILED_KEY,
                        "capture",
                        &[("error", error.to_string())],
                    ),
                };
                runner.notify(message);
            });
            Ok(json!({ "started": true }))
        }
        other => Err(CommandError::Failed(format!("no capture command {other}"))),
    }
}

/// Ends the preview shown (`false`), or prepares the active view as a capture would (`true`).
async fn preview(core: &Arc<Core>) -> Result<bool, EngineError> {
    let host = core.host().clone();
    let shown = lock(&SHOWN).take();
    if let Some((_view, prepared)) = shown {
        prepared.undo(host.cdp()).await;
        return Ok(false);
    }
    let view = host
        .target_views(None)?
        .first()
        .copied()
        .ok_or_else(|| EngineError::Invalid("there is no view".to_owned()))?;
    let target = host
        .capture_target(view)
        .ok_or_else(|| EngineError::Invalid(format!("view {view} is not attached yet")))?;
    let media = host.emulation(view).map_or_else(
        || json!({ "media": "", "features": [] }),
        |emulation| media_params(&emulation),
    );
    let plan = super::clean_plan(core, CleanOverrides::default());
    let prepared = clean::prepare(host.cdp(), &target.session, &plan, &media).await;
    *lock(&SHOWN) = Some((view, prepared));
    Ok(true)
}

#[cfg(test)]
#[path = "clean_defs_tests.rs"]
mod tests;
