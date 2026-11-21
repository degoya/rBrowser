//! The activity bar (RB-090-67): a strip of the shell, [`HEIGHT`] DIP high, that keeps its room
//! with `ui.activityBar.mode` `always` (the default, RB-094-13) or is there only
//! while something runs - a video, a recording, a flow run, a check run, the portal, views out of
//! sync - with the running activity's controls, each a command of the registry. The shell pages
//! draw it (`ui/src/activity/`); four things are the core's:
//!
//! - **The room.** The shell page asks for it with `activityBar { place }` while it has entries
//!   to show and gives it back without a place. `top` makes the toolbar's view [`height`] DIP
//!   higher ([`shell_view_height`]), `bottom` the status bar's view ([`status_view_height`]),
//!   which is then shown even with the status bar itself hidden; the window's box moves the
//!   views ([`crate::host::ui::UiBackend::relayout_shell`]), so the bar never overlaps a view.
//!   Only after that does the state name the place (`activityBar.place`), and only the page there
//!   draws the bar, so no page draws beyond its view. A place the settings do not name, or any
//!   while `ui.activityBar.mode` is `indicator`, is refused.
//! - **The folded entries** (`collapseActivity`): an entry closed with its close button lives
//!   on as the toolbar's indicator; the state names it for every page until the shell opens it
//!   again when its activity ended.
//! - **The recording just stopped** ([`on_recorder`]): offered for "Play last" until it is
//!   played, closed or another recording starts.
//! - **The running video** ([`publish_video`]), the one activity that told the shell nothing,
//!   and its pause ([`publish_video_pause`], RB-090-68).
//!
//! The bar is shell UI in the toolbar's or the status bar's view: a screenshot or a video of a
//! view reads that view's own surface over CDP (`Page.captureScreenshot`,
//! `Page.startScreencast` in the view's session) and never contains it.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use rb_config::{ActivityBarMode, ActivityBarPosition, ActivityBarSettings};
use rb_engine::ViewId;
use rb_ipc::{Activity, CoreToShell, IpcErrorCode, RecorderStatus, ShellToCore, VideoActivity};

use crate::control::Core;
use crate::host::Host;
use crate::host::shell_state::ShellState;
use crate::ipc::Failure;
use crate::shell::STATUS_BAR_HEIGHT;

/// The bar's height in DIP at 100 % text size: one row of small buttons.
pub const HEIGHT: u32 = 28;

/// Where the bar has room: 0 none, 1 top, 2 bottom.
static PLACE: AtomicU8 = AtomicU8::new(0);
/// The status bar's own row is shown (`ui.statusBar.visible`).
static STATUS_ROW: AtomicBool = AtomicBool::new(true);

fn code(place: Option<ActivityBarPosition>) -> u8 {
    match place {
        None => 0,
        Some(ActivityBarPosition::Top) => 1,
        Some(ActivityBarPosition::Bottom) => 2,
    }
}

/// Where the bar has room now; `None` while it has none.
#[must_use]
pub fn place() -> Option<ActivityBarPosition> {
    match PLACE.load(Ordering::Acquire) {
        1 => Some(ActivityBarPosition::Top),
        2 => Some(ActivityBarPosition::Bottom),
        _ => None,
    }
}

/// The bar's height in DIP at the text size in effect (RB-090-26).
#[must_use]
pub fn height() -> u32 {
    crate::text_scale::dip(HEIGHT)
}

/// The bar's height below the toolbar: [`height`] while it stands there, else 0.
#[must_use]
pub fn top_height() -> u32 {
    if place() == Some(ActivityBarPosition::Top) {
        height()
    } else {
        0
    }
}

/// The bar's height above the status bar: [`height`] while it stands there, else 0.
#[must_use]
pub fn bottom_height() -> u32 {
    if place() == Some(ActivityBarPosition::Bottom) {
        height()
    } else {
        0
    }
}

/// The toolbar's view in DIP: its rows ([`crate::toolbar_rows::height`]) and the bar below
/// them while it stands there.
#[must_use]
pub fn shell_view_height() -> u32 {
    crate::toolbar_rows::height().saturating_add(top_height())
}

/// The status bar's own row is shown or not (`ui.statusBar.visible`; the window's start and every
/// change of the setting).
pub fn set_status_row(shown: bool) {
    STATUS_ROW.store(shown, Ordering::Release);
}

/// The status bar's view in DIP: its own row while it is shown and the bar above it while it
/// stands there.
#[must_use]
pub fn status_view_height() -> u32 {
    let row = if STATUS_ROW.load(Ordering::Acquire) {
        crate::text_scale::dip(STATUS_BAR_HEIGHT)
    } else {
        0
    };
    row.saturating_add(bottom_height())
}

/// Whether the status bar's view is shown: it has its row or the bar.
#[must_use]
pub fn status_view_shown() -> bool {
    status_view_height() > 0
}

/// The place a request gets under `settings`: none always; the settings' position unless the
/// mode is `indicator`.
///
/// # Errors
///
/// A place while the mode is `indicator`, or another than `ui.activityBar.position`.
pub fn checked_place(
    asked: Option<ActivityBarPosition>,
    settings: &ActivityBarSettings,
) -> Result<Option<ActivityBarPosition>, String> {
    let Some(asked) = asked else {
        return Ok(None);
    };
    if settings.mode == ActivityBarMode::Indicator {
        return Err("ui.activityBar.mode is indicator: the activity bar has no room".to_owned());
    }
    if asked != settings.position {
        return Err(format!(
            "ui.activityBar.position is {:?}, not {asked:?}",
            settings.position
        ));
    }
    Ok(Some(asked))
}

/// Answers `activityBar` and `collapseActivity`; `None` for every other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    match message {
        ShellToCore::ActivityBar { place } => {
            let settings = core.user_settings().ui.activity_bar;
            Some(match checked_place(*place, &settings) {
                Ok(place) => {
                    make_room(core, place);
                    Ok(CoreToShell::Ack)
                }
                Err(reason) => Err((IpcErrorCode::Refused, reason)),
            })
        }
        ShellToCore::CollapseActivity {
            activity,
            collapsed,
        } => {
            core.host()
                .update_shell_state(|shell| collapse(shell, *activity, *collapsed));
            Some(Ok(CoreToShell::Ack))
        }
        _ => None,
    }
}

/// Gives the bar its room at `place` (or none), then names the place in the state.
fn make_room(core: &Core, place: Option<ActivityBarPosition>) {
    if PLACE.swap(code(place), Ordering::AcqRel) != code(place) {
        // The status bar's view holds the bar at the bottom, its own row hidden or not.
        core.ui().set_status_bar_visible(status_view_shown());
        core.ui().relayout_shell();
    }
    core.host()
        .update_shell_state(|shell| shell.activity_bar.place = place);
}

/// Folds `activity`'s entry into its indicator or opens it again. Closing the recorder's
/// entry while no recording runs ends the offer to play the last one instead.
pub fn collapse(shell: &mut ShellState, activity: Activity, collapsed: bool) {
    if activity == Activity::Recorder && collapsed && shell.recorder.is_none() {
        shell.activity_bar.recorded = None;
        return;
    }
    shell.activity_bar.set_collapsed(activity, collapsed);
}

/// The recorder's state changes to `next` (`crate::recorder::glue`): a recording that stops
/// is offered for "Play last", a recording that starts ends that offer.
pub fn on_recorder(shell: &mut ShellState, next: Option<&RecorderStatus>) {
    match (&shell.recorder, next) {
        (Some(running), None) => shell.activity_bar.recorded = Some(running.name.clone()),
        (_, Some(_)) => shell.activity_bar.recorded = None,
        (None, None) => {}
    }
}

/// The video recording that runs - its views, its start in ms since the epoch, its limit in
/// seconds - or none any more (`crate::video::app`): the bar's entry and the toolbar's
/// indicator follow it.
pub fn publish_video(host: &Host, running: Option<(Vec<ViewId>, u64, u64)>) {
    let video = running.map(|(views, started_at_ms, max_duration_sec)| VideoActivity {
        views,
        started_at_ms,
        max_duration_sec,
        paused_at_ms: None,
        paused_ms: 0,
    });
    host.update_shell_state(|shell| shell.video = video);
}

/// The running video paused at `paused_at_ms` or goes on (`None`), after `paused_ms` of pauses
/// that ended (RB-090-68, `crate::video::pause`); nothing without a running video.
pub fn publish_video_pause(host: &Host, paused_at_ms: Option<u64>, paused_ms: u64) {
    host.update_shell_state(|shell| {
        if let Some(video) = shell.video.as_mut() {
            video.paused_at_ms = paused_at_ms;
            video.paused_ms = paused_ms;
        }
    });
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
