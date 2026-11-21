//! The words of a label strip's pieces (RB-090-27): the external engine's window state - in the
//! engine badge beside the engine's name while the badge shows (RB-090-71) - and the view's
//! address. The Web Vitals are the vitals bar's (`crate::view_footer::vitals`). Pure; the
//! catalogue gives every word.

use rb_config::{CellAddress, CellLabelSettings};
use rb_engine::external::{ExternalStatus, ExternalView};
use rb_engine::{ViewId, ViewInfo};
use rb_i18n::{t, t_with};

use super::{CellInfo, Piece, PieceAction};
use crate::breakpoint_keys::engine_pick::name_key;

/// The engine's name and its window's state in one badge (RB-090-71).
pub const BADGE_KEY: &str = "engines.external.badge";
/// The most characters of a failed start's reason the strip shows; the tooltip has it whole
/// (RB-094-20).
pub const MAX_REASON: usize = 48;

/// What every view's label strip shows beside its name, in the order of `views`; `in_badge`
/// (`ui.engineBadge`) puts the engine's state into the engine badge.
pub fn cell_infos(
    views: &[ViewInfo],
    active: Option<ViewId>,
    externals: &[ExternalView],
    settings: &CellLabelSettings,
    in_badge: bool,
) -> Vec<CellInfo> {
    let leader = active.and_then(|active| views.iter().find(|view| view.id == active));
    views
        .iter()
        .map(|view| CellInfo {
            view: view.id,
            address: address_piece(view, leader, settings.address),
            engine: externals
                .iter()
                .find(|entry| entry.view == view.id)
                .filter(|_| settings.engine_status)
                .map(|external| engine_piece(external, in_badge)),
            engine_in_badge: in_badge,
        })
        .collect()
}

fn status_id(status: ExternalStatus) -> &'static str {
    match status {
        ExternalStatus::Starting => "starting",
        ExternalStatus::Running => "running",
        ExternalStatus::Exited => "exited",
        ExternalStatus::Closed => "closed",
        ExternalStatus::Restarting => "restarting",
        ExternalStatus::Failed => "failed",
        ExternalStatus::Unavailable => "unavailable",
    }
}

/// What a click on the engine's state does and the words for it: restart a stopped engine, try
/// a failed start again, open a closed window again, bring a window of its own to the front; `None` while there is nothing
/// to do (a window in its cell, one starting). The engine badge's menu offers it too.
pub fn engine_click(external: &ExternalView) -> Option<(PieceAction, &'static str)> {
    let in_cell = external.status == ExternalStatus::Running && external.embedded;
    match external.status {
        ExternalStatus::Closed => Some((PieceAction::Restart, "engines.external.reopen")),
        ExternalStatus::Exited => Some((PieceAction::Restart, "engines.external.restart")),
        // A start that failed is tried again (RB-094-20).
        ExternalStatus::Failed => Some((PieceAction::Restart, "engines.external.retry")),
        ExternalStatus::Running if !in_cell => Some((PieceAction::Raise, "engines.external.show")),
        _ => None,
    }
}

/// The row of the engine badge's menu that does what the engine window's state calls for
/// (RB-090-71): reopen, restart or show the window of `view`; `None` while `ui.cellLabel.
/// engineStatus` is off, for a view without an external engine and while nothing is to do.
pub fn engine_menu_row(
    externals: &[ExternalView],
    view: ViewId,
    settings: &CellLabelSettings,
) -> Option<(PieceAction, &'static str)> {
    if !settings.engine_status {
        return None;
    }
    let external = externals.iter().find(|external| external.view == view)?;
    engine_click(external)
}

/// The engine badge's text and tooltip from its `own` (the engine, how to use it or why it
/// cannot run) and the engine window's `state` (RB-090-71): the state's text; the state's
/// details before the badge's own - but a `dimmed` badge, whose engine cannot run, names its
/// reason only, which already says what the state would.
pub fn badge_words(own: (&str, &str), dimmed: bool, state: Option<&Piece>) -> (String, String) {
    let (text, tooltip) = own;
    match state {
        None => (text.to_owned(), tooltip.to_owned()),
        Some(state) if dimmed => (state.text.clone(), tooltip.to_owned()),
        Some(state) => (state.text.clone(), format!("{}\n{tooltip}", state.tooltip)),
    }
}

/// The external engine's piece: the engine and its window's state - joined by a middle dot
/// `in_badge`, for the engine badge, else `Firefox: window closed` - the reason or what it does
/// not emulate in the tooltip, and what a click does ([`engine_click`]).
pub fn engine_piece(external: &ExternalView, in_badge: bool) -> Piece {
    let engine = t(&name_key(external.engine));
    let in_cell = external.status == ExternalStatus::Running && external.embedded;
    let failed_because = external
        .reason
        .as_deref()
        .filter(|reason| external.status == ExternalStatus::Failed && !reason.trim().is_empty());
    let status = if in_cell {
        t("engines.external.status.embedded")
    } else if let Some(reason) = failed_because {
        // RB-094-20: the strip says why, cut; the tooltip has the whole reason.
        t_with(
            "engines.external.status.failedBecause",
            &[(
                "reason",
                &rb_util::text::cut_ellipsis(reason.trim(), MAX_REASON),
            )],
        )
    } else {
        t(&format!(
            "engines.external.status.{}",
            status_id(external.status)
        ))
    };
    let key = if in_badge {
        BADGE_KEY
    } else {
        "engines.external.chip"
    };
    let text = t_with(key, &[("engine", &engine), ("status", &status)]);
    let mut tooltip =
        if external.status == ExternalStatus::Running && !external.unsupported.is_empty() {
            t_with(
                "engines.external.unsupported",
                &[
                    ("engine", &engine),
                    ("fields", &external.unsupported.join(", ")),
                ],
            )
        } else if in_cell {
            t_with("engines.external.hint.embedded", &[("engine", &engine)])
        } else {
            t_with(
                &format!("engines.external.hint.{}", status_id(external.status)),
                &[
                    ("engine", &engine),
                    ("reason", external.reason.as_deref().unwrap_or_default()),
                ],
            )
        };
    let (action, click) = match engine_click(external) {
        Some((action, click)) => (action, Some(click)),
        None => (PieceAction::Activate, None),
    };
    if let Some(click) = click {
        tooltip.push('\n');
        tooltip.push_str(&t_with("cells.info.click", &[("action", &t(click))]));
    }
    Piece {
        text,
        tooltip,
        action,
    }
}

/// The address piece of `view`, when `shown` says it shows: `differs` while it is not the
/// address of `leader` (the active view), which the toolbar's address bar names.
pub fn address_piece(
    view: &ViewInfo,
    leader: Option<&ViewInfo>,
    shown: CellAddress,
) -> Option<Piece> {
    if view.url.is_empty() {
        return None;
    }
    let show = match shown {
        CellAddress::Off => false,
        CellAddress::Always => true,
        CellAddress::Differs => {
            leader.is_some_and(|leader| leader.id != view.id && leader.url != view.url)
        }
    };
    show.then(|| Piece {
        text: display_url(&view.url),
        tooltip: format!(
            "{}\n{}",
            t_with("sync.address.label", &[("label", &view.label)]),
            view.url
        ),
        action: PieceAction::Activate,
    })
}

/// `url` as the strip writes it: without `http://` or `https://`, and without the slash of a
/// bare origin (`localhost:3000/` is `localhost:3000`).
pub fn display_url(url: &str) -> String {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    let Some(rest) = rest else {
        return url.to_owned();
    };
    match rest.strip_suffix('/') {
        Some(origin) if !origin.contains('/') && !origin.is_empty() => origin.to_owned(),
        _ => rest.to_owned(),
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
