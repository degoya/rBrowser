//! The engine badge and its menu (RB-080-33): what the badge at the right end of a view's label
//! strip says - the engine's name, dimmed with the reason while the engine cannot run - and the
//! rows of the menu a click on it opens: Chromium, Firefox, WebKit (Playwright, RB-097-02),
//! Safari and iOS, the view's
//! engine checked, one that cannot run disabled with its reason (Safari and iOS outside macOS,
//! RB-095-03, RB-095-04), iOS followed by a row per simulator model and device of the Mac
//! (`crate::engines::ios::commands::device_items`). A choice runs `view.set_engine`
//! ([`crate::engines::set_engine`]); the canvas board draws the same badge and opens the same
//! menu (`CanvasBoard.vue`, `canvasEnginePick`).

use rb_config::EngineKind;
use rb_engine::{EngineHealth, ViewHost as _, ViewId, engine_name};
use serde_json::{Value, json};

use crate::control::Core;

/// The i18n key of the badge's tooltip; `{engine}`.
pub const BADGE_TOOLTIP: &str = "engines.badge.tooltip";
/// The i18n key of the tooltip of a badge whose engine cannot run; `{engine}`, `{reason}`.
pub const BADGE_UNAVAILABLE: &str = "engines.badge.unavailable";
/// The i18n key of a menu row whose engine cannot run; `{engine}`, `{reason}`.
pub const ROW_UNAVAILABLE: &str = "engines.pick.unavailable";
/// The i18n key of WebKit's row: "WebKit (Playwright)", apart from Safari's row (RB-097-02); the
/// badge's tooltip keeps "WebKit (not Safari)".
pub const WEBKIT_ROW: &str = "engines.pick.webkit";

/// The engines, in menu order.
pub const ENGINES: [EngineKind; 5] = [
    EngineKind::Chromium,
    EngineKind::Firefox,
    EngineKind::Webkit,
    // Disabled with its reason outside macOS (RB-095-03).
    EngineKind::Safari,
    // The same; on a Mac its simulators and devices follow (RB-095-04).
    EngineKind::Ios,
];

/// One row of the engine menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineItem {
    /// The engine the row chooses.
    pub engine: EngineKind,
    /// The text (already translated).
    pub label: String,
    /// Whether the view runs on it now.
    pub checked: bool,
    /// Why the engine cannot run; `None` when it can (the row is enabled).
    pub reason: Option<String>,
    /// An iOS row's model or device UDID (`iosDevice`, RB-095-04); `None` for an engine's row.
    pub ios_device: Option<String>,
}

impl EngineItem {
    /// Whether the row can be chosen.
    pub fn available(&self) -> bool {
        self.reason.is_none()
    }
}

/// What the badge of a view shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badge {
    /// The view's engine.
    pub engine: EngineKind,
    /// The badge's text: the engine's name.
    pub text: String,
    /// The tooltip: how to use the badge, or why the engine cannot run.
    pub tooltip: String,
    /// Why the engine cannot run; `None` when it does (the badge is not dimmed).
    pub reason: Option<String>,
}

impl Badge {
    /// Whether the engine runs (or will on the next view); a dimmed badge says it cannot.
    pub fn available(&self) -> bool {
        self.reason.is_none()
    }
}

/// The i18n key of the engine's full name (`engines.name.firefox`).
pub fn name_key(engine: EngineKind) -> String {
    format!("engines.name.{}", config_name(engine))
}

/// The engine's name in the config and the command arguments (`firefox`).
pub fn config_name(engine: EngineKind) -> String {
    engine.name().to_owned()
}

/// The rows of the menu for a view on `current`; `reason` says why an engine cannot run, if it
/// cannot.
pub fn items(
    current: EngineKind,
    reason: impl Fn(EngineKind) -> Option<String>,
) -> Vec<EngineItem> {
    ENGINES
        .into_iter()
        .map(|engine| {
            let name = row_name(engine);
            let reason = reason(engine);
            let label = match &reason {
                Some(reason) => {
                    rb_i18n::t_with(ROW_UNAVAILABLE, &[("engine", &name), ("reason", reason)])
                }
                None => name,
            };
            EngineItem {
                engine,
                label,
                checked: engine == current,
                reason,
                ios_device: None,
            }
        })
        .collect()
}

/// The name of `engine`'s menu row: WebKit by its builder, Playwright, every other engine by
/// its full name.
fn row_name(engine: EngineKind) -> String {
    match engine {
        EngineKind::Webkit => rb_i18n::t(WEBKIT_ROW),
        other => rb_i18n::t(&name_key(other)),
    }
}

/// The badge of a view on `engine`; `reason` when the engine cannot run. The tooltip has the
/// engine's full name ("WebKit (not Safari)").
pub fn badge(engine: EngineKind, reason: Option<String>) -> Badge {
    let name = engine_name(engine);
    let full = rb_i18n::t(&name_key(engine));
    let tooltip = match &reason {
        Some(reason) => {
            rb_i18n::t_with(BADGE_UNAVAILABLE, &[("engine", &full), ("reason", reason)])
        }
        None => rb_i18n::t_with(BADGE_TOOLTIP, &[("engine", &full)]),
    };
    Badge {
        engine,
        text: name.to_owned(),
        tooltip,
        reason,
    }
}

/// The arguments `item` runs `view.set_engine` with for `view`.
pub fn command_args(item: &EngineItem, view: ViewId) -> Value {
    match &item.ios_device {
        Some(device) => {
            json!({ "view": view, "engine": config_name(item.engine), "iosDevice": device })
        }
        None => json!({ "view": view, "engine": config_name(item.engine) }),
    }
}

/// Why the engine of `kind` cannot run in `core`'s window; `None` for Chromium, for an engine
/// that runs or starts on its next view, and while the external windows are not started.
pub fn unavailable_reason(core: &Core, kind: EngineKind) -> Option<String> {
    if kind == EngineKind::Chromium {
        return None;
    }
    let engines = crate::engines::for_host(core.host())?;
    match engines.windows().engine_health(kind) {
        EngineHealth::Unavailable(reason) => Some(reason),
        EngineHealth::Running | EngineHealth::Stopped => None,
    }
}

/// The badge of `view`; `None` for a view that does not exist.
pub fn badge_of(core: &Core, view: ViewId) -> Option<Badge> {
    let engine = core.host().view(view)?.engine;
    Some(badge(engine, unavailable_reason(core, engine)))
}

/// The rows of the engine menu of `view`; `None` for a view that does not exist.
pub fn engine_pick(core: &Core, view: ViewId) -> Option<Vec<EngineItem>> {
    let engine = core.host().view(view)?.engine;
    let items = items(engine, |kind| unavailable_reason(core, kind));
    Some(crate::engines::ios::commands::with_device_rows(
        core, view, items,
    ))
}

/// A row of the engine menu was chosen for `view`: it runs `view.set_engine` off the calling
/// (UI) thread; a disabled row does nothing.
pub fn pick_engine(core: &Core, view: ViewId, item: &EngineItem) {
    if !item.available() {
        return;
    }
    let args = command_args(item, view);
    core.run_command_async(
        crate::engines::set_engine::SET_ENGINE,
        args,
        move |result| {
            if let Err(error) = result {
                tracing::warn!(%view, %error, "the engine could not be changed");
            }
        },
    );
}

#[cfg(test)]
#[path = "engine_pick_tests.rs"]
mod tests;
