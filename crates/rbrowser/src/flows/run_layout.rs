//! `automation.flows.hiddenViews: layout` (RB-090-84, the owner's option): when the layout hides
//! views - focus mode shows one breakpoint, horizontal mode scrolls views out of sight - a run
//! shows the layout of `automation.flows.runLayout` while it
//! goes on: a mode (`grid`, `horizontal`, `split`, `canvas`) over the preset shown, or a preset.
//! It waits up to [`PAGES_WAIT`] for the pages of the views that opened, and shows the layout
//! before again when the run ends - unless the person chose another layout meanwhile
//! ([`still_shown`]). A `flows.run_all` batch switches once and shows the layout before at its
//! end ([`batch_begin`], [`batch_end`]). With `skip` (the default) nothing changes: the run skips
//! the hidden views and names them ([`super::plan::without_hidden`]).

use std::sync::Mutex;
use std::time::Duration;

use rb_config::{HiddenViews, LayoutMode, ProjectConfig};
use rb_engine::{EngineKind, ViewHost as _};

use crate::cdp::lock;
use crate::control::Core;
use crate::presets::{Selection, apply_selection, current_selection};

/// How long a run waits for the pages of the views its layout opened.
pub const PAGES_WAIT: Duration = Duration::from_secs(5);
/// How often it looks.
const POLL: Duration = Duration::from_millis(100);

/// The layout a run replaced, to show again after it, and the one it showed.
#[derive(Clone, Debug, PartialEq)]
pub struct Replaced {
    before: Selection,
    shown: Selection,
    focus_return: Option<LayoutMode>,
}

/// What [`show`] did.
#[derive(Clone, Debug, PartialEq)]
pub enum Shown {
    /// The layout stayed.
    Same,
    /// It switched; the run shows the layout before again with this, `None` when a batch does.
    Switched(Option<Replaced>),
}

impl Shown {
    /// Whether the views changed, so the run reads them again.
    pub fn switched(&self) -> bool {
        matches!(self, Self::Switched(_))
    }

    /// What the run restores at its end.
    pub fn into_restore(self) -> Option<Replaced> {
        match self {
            Self::Switched(replaced) => replaced,
            Self::Same => None,
        }
    }
}

/// A `flows.run_all` batch going on, with the layout its first switch replaced.
static BATCH: Mutex<Option<Option<Replaced>>> = Mutex::new(None);

/// A batch starts: the runs' switches are kept for its end.
pub fn batch_begin() {
    *lock(&BATCH) = Some(None);
}

/// The batch ended: the layout before it comes back.
pub fn batch_end(core: &Core) {
    if let Some(replaced) = take_batch() {
        restore(core, replaced);
    }
}

/// Ends the batch and answers the layout its first switch replaced.
fn take_batch() -> Option<Replaced> {
    lock(&BATCH).take().flatten()
}

/// `replaced` kept for the batch going on (its first switch only); handed back outside one.
fn keep_for_batch(replaced: Replaced) -> Option<Replaced> {
    let mut batch = lock(&BATCH);
    match batch.as_mut() {
        Some(slot) => {
            if slot.is_none() {
                *slot = Some(replaced);
            }
            None
        }
        None => Some(replaced),
    }
}

/// Whether the layout `now` is still the one the run showed.
fn still_shown(now: &Selection, replaced: &Replaced) -> bool {
    *now == replaced.shown
}

/// The selection a run shows instead of `current` for `run_layout` - a mode's name over the
/// preset shown (a focus preset leaves for the top-level layout), else a preset's id, else the
/// grid -, when the layout `hides` views; `None` when it hides none or already is that one.
pub fn wanted(
    config: &ProjectConfig,
    current: &Selection,
    run_layout: &str,
    hides: bool,
) -> Option<Selection> {
    if !hides {
        return None;
    }
    let mode = match run_layout {
        "grid" => Some(LayoutMode::Grid),
        "horizontal" => Some(LayoutMode::Horizontal),
        "split" => Some(LayoutMode::Split),
        "canvas" => Some(LayoutMode::Canvas),
        _ => None,
    };
    let grid = || Selection {
        preset: None,
        mode: LayoutMode::Grid,
        focus: None,
    };
    let target = match mode {
        Some(mode) => Selection {
            preset: current.preset.clone().filter(|id| {
                config
                    .layouts
                    .presets
                    .get(id.as_str())
                    .is_some_and(|preset| preset.mode != LayoutMode::Focus)
            }),
            mode,
            focus: None,
        },
        None => Selection::preset(config, run_layout)
            .ok()
            .filter(|selection| selection.mode != LayoutMode::Focus)
            .unwrap_or_else(grid),
    };
    (target != *current).then_some(target)
}

/// Shows the run's layout when `automation.flows.hiddenViews` is `layout` and the layout shown
/// hides views; answers what to show again after the run.
pub async fn show(core: &Core) -> Shown {
    let flows = core.user_settings().automation.flows;
    if flows.hidden_views != HiddenViews::Layout {
        return Shown::Same;
    }
    let host = core.host();
    let before = current_selection(host);
    let views = host.views();
    // The views a person closed (`layout.hidden`) stay closed in every layout: no reason.
    let hides =
        before.mode == LayoutMode::Focus || views.iter().any(|view| host.is_off_screen(view.id));
    let Some(target) = wanted(&host.config(), &before, &flows.run_layout, hides) else {
        return Shown::Same;
    };
    let focus_return = host.shell_state().focus_return;
    if let Err(error) = apply_selection(core, &target, false) {
        tracing::warn!(%error, "the layout of a flow run could not be shown");
        return Shown::Same;
    }
    tracing::info!(mode = ?target.mode, preset = ?target.preset, "a flow run shows its layout");
    wait_for_pages(core).await;
    Shown::Switched(keep_for_batch(Replaced {
        before,
        shown: target,
        focus_return,
    }))
}

/// Shows the layout the run replaced again, unless the person chose another one meanwhile.
pub fn restore(core: &Core, replaced: Replaced) {
    if !still_shown(&current_selection(core.host()), &replaced) {
        tracing::info!("the layout changed during the flow run: it stays");
        return;
    }
    if let Err(error) = apply_selection(core, &replaced.before, false) {
        tracing::warn!(%error, "the layout before a flow run could not be shown again");
        return;
    }
    core.host()
        .update_shell_state(|shell| shell.focus_return = replaced.focus_return);
}

/// Waits until every Chromium view has its page, at most [`PAGES_WAIT`].
async fn wait_for_pages(core: &Core) {
    let deadline = tokio::time::Instant::now() + PAGES_WAIT;
    loop {
        let host = core.host();
        let ready = host
            .views()
            .iter()
            .filter(|view| view.engine == EngineKind::Chromium)
            .all(|view| host.target_id(view.id).is_some());
        if ready || tokio::time::Instant::now() >= deadline {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
}

#[cfg(test)]
#[path = "run_layout_tests.rs"]
mod tests;
