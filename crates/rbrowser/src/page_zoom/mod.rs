//! The page zoom inside a view (RB-090-20): Chromium's own zoom of the page
//! (`CefBrowserHost::SetZoomLevel`), not the grid's zoom of the cell (`view.zoom_in`). A zoomed
//! page lays out at another CSS width than its breakpoint's, so the label strip shows the
//! level as a badge whose click sets it back (`cef_app/page_zoom.rs`).
//!
//! Who zooms: the commands `view.page_zoom.reset` (100 %, or `{ level }` as a factor),
//! `view.page_zoom.in` and `view.page_zoom.out` (Chromium's steps, [`PRESETS`]), each with
//! `target: "all" | viewId` (the active view without one), and MCP `view_page_zoom`; with the
//! focus in a page Ctrl+0, Ctrl+Plus and Ctrl+Minus run them for that view - the reservation
//! list keeps those keys for the browser, and this is the browser's meaning
//! (`views.pageZoom.keys`); on the canvas board Ctrl+0 stays the canvas zoom (RB-080-94).
//! Ctrl+wheel zooms inside Chromium without a word to rBrowser: the window reads every view's
//! level a few times a second and hands a change to [`observed`].
//!
//! [`Levels`] is the model: each view's level as rBrowser last set or read it.
//! `views.pageZoom.sync` makes every zoom every view's; `views.pageZoom.wheel` off puts a
//! wheel's zoom back; `views.pageZoom.keepOnReload` off starts every load at 100 %. Chromium
//! keeps a zoom per site, so views of one session on the same host zoom together whatever the
//! setting - the window reads what is, and each badge says it.
//!
//! A Firefox view zooms as well (RB-090-69): its cell holds a Chromium stand-in under the
//! Firefox window, and that is what the commands zoom, so a key with the focus there and the
//! sync work as for a Chromium view. Firefox's own window zooms on its own keys and wheel - BiDi
//! has no page zoom to set or read (`browsingContext.setViewport`'s `devicePixelRatio` scales the
//! device, not the page) - and its agent reports the key, which runs the same command for the
//! view (`engines/firefox/follow.rs`, [`page_key_action`]). A WebKit view does not zoom.

mod defs;
mod model;

use std::sync::{LazyLock, Mutex, MutexGuard};

use rb_commands::CommandError;
use rb_config::{EngineKind, PageZoomSettings};
use rb_engine::{EngineError, Target, ViewHost as _, ViewId};
use serde::Deserialize;
use serde_json::{Value, json};

pub use defs::{RESET, TOOL, ZOOM_IN, ZOOM_OUT, definitions, register};
pub use model::{
    Action, Levels, MAX_FACTOR, MIN_FACTOR, PRESETS, Reading, factor_of, is_zoomed, key_action,
    level_of, page_key_action, percent_of, split_followers, step,
};

use crate::cdp::lock;
use crate::control::Core;

/// The model of the window's views. One window per process.
static LEVELS: LazyLock<Mutex<Levels>> = LazyLock::new(|| Mutex::new(Levels::default()));

fn levels() -> MutexGuard<'static, Levels> {
    lock(&LEVELS)
}

/// Empties the model (the tests).
#[cfg(test)]
fn reset_model() {
    *levels() = Levels::default();
}

/// The arguments of the three commands.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoomArgs {
    /// `"all"` or a view id; the active view when absent.
    target: Option<Target>,
    /// `view.page_zoom.reset`: the factor, 1 for 100 %.
    level: Option<f64>,
}

/// Runs `view.page_zoom.reset`, `.in` or `.out` with `args`; answers
/// `{ views: [{ view, level, percent }] }`, `level` the factor each view is at now.
///
/// # Errors
///
/// An unknown id, arguments that do not read, a `level` outside 0.25 to 5, an unknown target or
/// one without a Chromium or Firefox view.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    let args: ZoomArgs = if args.is_null() {
        ZoomArgs::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    let action = match (id, args.level) {
        (RESET, None) => Action::Reset,
        (RESET, Some(factor)) => Action::Set(level_of(checked(factor)?)),
        (ZOOM_IN | ZOOM_OUT, Some(_)) => {
            return Err(CommandError::InvalidArgs(format!(
                "{id} takes no level; {RESET} sets one"
            )));
        }
        (ZOOM_IN, None) => Action::In,
        (ZOOM_OUT, None) => Action::Out,
        (other, _) => return Err(CommandError::UnknownCommand(other.to_owned())),
    };
    let sets = apply(core, args.target, action)?;
    Ok(answer(&sets))
}

/// `factor` when Chromium zooms to it.
fn checked(factor: f64) -> Result<f64, CommandError> {
    if factor.is_finite() && (MIN_FACTOR..=MAX_FACTOR).contains(&factor) {
        Ok(factor)
    } else {
        Err(CommandError::InvalidArgs(format!(
            "level is a factor from {MIN_FACTOR} to {MAX_FACTOR} (1 = 100 %), not {factor}"
        )))
    }
}

/// Does `action` on the zoomable views of `target` (every zoomable view with
/// `views.pageZoom.sync`, [`zoomable_views`]) and returns what it set.
fn apply(
    core: &Core,
    target: Option<Target>,
    action: Action,
) -> Result<Vec<(ViewId, f64)>, CommandError> {
    let zoomable = zoomable_views(core);
    let targets: Vec<ViewId> = core
        .host()
        .target_views(target)
        .map_err(failed)?
        .into_iter()
        .filter(|view| zoomable.contains(view))
        .collect();
    if targets.is_empty() {
        return Err(CommandError::Failed(
            "the page zoom works in Chromium and Firefox views only".to_owned(),
        ));
    }
    let settings = settings(core);
    let sets = change(core, |levels| {
        levels.retain(&zoomable);
        let sets = levels.plan(action, &targets, settings.sync.then_some(&zoomable));
        levels.record(&sets);
        sets
    });
    set_all(core, &sets);
    Ok(sets)
}

/// Runs `edit` on the model; when a view's level moved, the shell gets the new state - the
/// canvas board's labels show the level ([`badge`], RB-090-28).
fn change<T>(core: &Core, edit: impl FnOnce(&mut Levels) -> T) -> T {
    let (result, moved) = {
        let mut levels = levels();
        let before = levels.clone();
        let result = edit(&mut levels);
        (result, *levels != before)
    };
    if moved {
        core.host().state_changed();
    }
    result
}

/// What `view`'s label shows of its page zoom: the percent while the page is zoomed, `None` at
/// 100 % (the canvas board, RB-090-28; the grid's strips read the window's own reading).
#[must_use]
pub fn badge(view: ViewId) -> Option<u32> {
    let level = levels().get(view);
    is_zoomed(level).then(|| percent_of(level))
}

/// The window read `level` on `view`, a level rBrowser did not set (the wheel, a site's own
/// zoom): the model takes it, and what the settings answer is set.
pub fn observed(core: &Core, view: ViewId, level: f64) {
    let settings = settings(core);
    let all = zoomable_views(core);
    let sets = change(core, |levels| levels.observe(view, level, &settings, &all));
    set_all(core, &sets);
}

/// The levels that moved because rBrowser set a view's (Chromium keeps a zoom per site, so the
/// views on the same host follow): the model takes them as rBrowser's own, and the wheel's
/// guard leaves them alone - it would otherwise put back what Chromium moved and move the
/// first view with it, back and forth.
pub fn followed(core: &Core, sets: &[(ViewId, f64)]) {
    change(core, |levels| levels.record(sets));
}

/// `view`'s page finished loading at `level` (the window, UI thread).
pub fn loaded(core: &Core, view: ViewId, level: f64) {
    let settings = settings(core);
    let sets = change(core, |levels| levels.loaded(view, level, &settings));
    set_all(core, &sets);
}

/// `views.pageZoom.keys`: whether Ctrl+0, Ctrl+Plus and Ctrl+Minus in a page zoom it.
#[must_use]
pub fn keys_on(core: &Core) -> bool {
    settings(core).keys
}

/// The command of `action` from a key or a click (`Set` has none: it is `reset` with a level).
#[must_use]
pub fn command_of(action: Action) -> &'static str {
    match action {
        Action::Reset | Action::Set(_) => RESET,
        Action::In => ZOOM_IN,
        Action::Out => ZOOM_OUT,
    }
}

fn settings(core: &Core) -> PageZoomSettings {
    core.user_settings().views.page_zoom
}

/// The views whose page zooms: the Chromium views and the Firefox views, whose cell's
/// Chromium stand-in zooms (RB-090-69).
fn zoomable_views(core: &Core) -> Vec<ViewId> {
    core.host()
        .views()
        .iter()
        .filter(|view| matches!(view.engine, EngineKind::Chromium | EngineKind::Firefox))
        .map(|view| view.id)
        .collect()
}

fn set_all(core: &Core, sets: &[(ViewId, f64)]) {
    for (view, level) in sets {
        core.ui().set_page_zoom(*view, *level);
    }
}

/// `{ views: [{ view, level, percent }] }`, `level` the factor to four places.
fn answer(sets: &[(ViewId, f64)]) -> Value {
    let views: Vec<Value> = sets
        .iter()
        .map(|(view, level)| {
            let factor = (factor_of(*level) * 10_000.0).round() / 10_000.0;
            json!({ "view": view.0, "level": factor, "percent": percent_of(*level) })
        })
        .collect();
    json!({ "views": views })
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
