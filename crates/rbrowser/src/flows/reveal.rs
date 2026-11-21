//! A played recording in sight (RB-090-89, the owner's finding on 030125f): with
//! `automation.flows.hiddenViews: skip` a run skips the views the layout hides
//! ([`super::plan::without_hidden`]), but when every view a played recording would run in is
//! hidden, Play brings the first of them into sight instead of failing - the canvas centres on it,
//! the horizontal strip scrolls to it, focus mode focuses the first Chromium breakpoint the
//! recording names when it shows another - and a notice says so ([`NOTICE`]), once a view of the
//! run is shown. `automation.flows.revealRecording` (on by default) switches it off. A recording that still has a view in sight, a flow
//! file and views asked for keep `skip`; `layout` shows its own layout ([`super::run_layout`]).
//! What it changed stays after the run: the person sees where it played.

use std::collections::BTreeSet;

use rb_config::{Breakpoint, HiddenViews, LayoutMode};
use rb_engine::{EngineKind, ViewHost as _, ViewId, ViewInfo};
use rb_ipc::NotificationLevel;
use serde_json::json;

use super::app::notification;
use super::run::RunRequest;
use super::{catalog, run_layout};
use crate::control::Core;
use crate::presets::{apply_selection, current_selection};

/// The notice that a view was brought into sight; `{flow}` and `{view}`.
pub const NOTICE: &str = "flows.notice.revealed";

/// How a view of a played recording comes into sight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reveal {
    /// The canvas centred on the view.
    Center(ViewId),
    /// The horizontal strip scrolled to the view at this place.
    Scroll(usize),
    /// Focus mode on this breakpoint.
    Focus(String),
}

/// What brings a view of a recording that names `breakpoints` (ids or labels; every view when
/// none) into sight, the layout `mode` hiding `hidden` of `views` (grid order) of the config's
/// `known` breakpoints. `None` while a Chromium view of it is shown, when no view shows one of its
/// breakpoints outside focus mode, or in a mode that hides no view by its place (grid, split).
#[must_use]
pub fn needed(
    mode: LayoutMode,
    views: &[ViewInfo],
    hidden: &BTreeSet<ViewId>,
    breakpoints: &[String],
    known: &[Breakpoint],
) -> Option<Reveal> {
    let ours: Vec<(usize, &ViewInfo)> = views
        .iter()
        .enumerate()
        .filter(|(_, view)| view.engine == EngineKind::Chromium)
        .filter(|(_, view)| {
            breakpoints.is_empty()
                || breakpoints
                    .iter()
                    .any(|name| super::plan::names(view, name))
        })
        .collect();
    let Some(&(index, first)) = ours.first() else {
        // Focus mode shows another breakpoint: the first Chromium one the recording names.
        if mode != LayoutMode::Focus {
            return None;
        }
        let same = |text: &str, name: &str| text.eq_ignore_ascii_case(name);
        return breakpoints
            .iter()
            .find_map(|name| {
                known.iter().find(|breakpoint| {
                    breakpoint.engine == rb_config::EngineKind::Chromium
                        && (same(&breakpoint.id, name)
                            || breakpoint
                                .label
                                .as_deref()
                                .is_some_and(|label| same(label, name)))
                })
            })
            .map(|breakpoint| Reveal::Focus(breakpoint.id.clone()));
    };
    if ours.iter().any(|(_, view)| !hidden.contains(&view.id)) {
        return None;
    }
    match mode {
        LayoutMode::Canvas => Some(Reveal::Center(first.id)),
        LayoutMode::Horizontal => Some(Reveal::Scroll(index)),
        _ => None,
    }
}

/// Before the played recording `request` runs: brings its view into sight when [`needed`] and
/// says so. Answers whether the views changed, so the run reads them again.
pub async fn show(core: &Core, request: &RunRequest) -> bool {
    let Some(inline) = request.inline.as_ref().filter(|_| request.views.is_none()) else {
        return false;
    };
    let flows = core.user_settings().automation.flows;
    if flows.hidden_views != HiddenViews::Skip || !flows.reveal_recording {
        return false;
    }
    let host = core.host();
    let mut breakpoints = catalog::parse_head(&inline.source).views;
    if breakpoints.is_empty() {
        breakpoints.clone_from(&inline.views);
    }
    let views = host.views();
    let hidden: BTreeSet<ViewId> = views
        .iter()
        .map(|view| view.id)
        .filter(|id| host.is_off_screen(*id))
        .collect();
    let mode = current_selection(host).mode;
    let known = host.config().breakpoints;
    let Some(reveal) = needed(mode, &views, &hidden, &breakpoints, &known) else {
        return false;
    };
    let Some(shown) = bring(core, &reveal).await else {
        return false;
    };
    tracing::info!(?reveal, "a played recording's view was brought into sight");
    core.notify(notification(
        NotificationLevel::Info,
        NOTICE,
        &[("flow", request.name.clone()), ("view", shown)],
    ));
    true
}

/// Does `reveal`; answers the view of the run brought into sight by its label, or `None` when
/// none is shown afterwards.
pub async fn bring(core: &Core, reveal: &Reveal) -> Option<String> {
    let host = core.host();
    let label_of = |id: ViewId| host.view(id).map(|view| view.label);
    let done = match reveal {
        Reveal::Center(view) => {
            crate::layout::canvas_ops::center(core, &json!({ "view": view.0 })).map(|_| *view)
        }
        Reveal::Scroll(index) => {
            let view = host.views().get(*index).map(|view| view.id)?;
            crate::layout::canvas_ops::center(core, &json!({ "index": index })).map(|_| view)
        }
        Reveal::Focus(breakpoint) => {
            let mut next = current_selection(host);
            next.focus = Some(breakpoint.clone());
            if let Err(error) = apply_selection(core, &next, false) {
                tracing::warn!(%error, "the recording's breakpoint could not be focused");
                return None;
            }
            run_layout::wait_for_pages(core).await;
            let shown = host.views().into_iter().find(|view| {
                view.cell_breakpoint.as_deref() == Some(breakpoint.as_str())
                    && !host.is_off_screen(view.id)
            });
            return shown.map(|view| view.label);
        }
    };
    match done {
        Ok(view) if !host.is_off_screen(view) => label_of(view),
        Ok(_) => None,
        Err(error) => {
            tracing::warn!(%error, "a played recording's view could not be brought into sight");
            None
        }
    }
}

#[cfg(test)]
#[path = "reveal_tests.rs"]
mod tests;
