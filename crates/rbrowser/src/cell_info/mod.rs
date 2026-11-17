//! What each view's label strip shows beside its name and size (RB-090-27, `ui.cellLabel`): the
//! state of an external engine's window (RB-040-01) - in the engine badge beside the engine's
//! name while `ui.engineBadge` shows it (RB-090-71) - and the view's address (RB-020-06). The
//! pieces are built here from what the core already holds - the views and the shell state's
//! `externals` - and handed to the window ([`crate::host::ui::UiBackend::set_cell_info`]),
//! which draws them as native labels in the strip (`cef_app/cell_info.rs`). The same burst
//! words every view's vitals bar from the shell state's `metrics` (RB-090-71,
//! `crate::view_footer::vitals`) and hands it to the host's footers. They follow the view events
//! once per burst and only when something changed; a CPU reading that moves no warning changes
//! nothing. The shell state keeps the pieces too, for the canvas board, which draws its labels
//! itself ([`board`], RB-090-69). MCP reads the same data from `view_metrics` and `views_list`.

pub mod board;
mod text;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use rb_engine::{ViewHost as _, ViewId};

use crate::control::Core;

pub use text::{badge_words, cell_infos, engine_click, engine_menu_row};

/// How long a burst of view events is gathered before the pieces are built once.
const COALESCE: Duration = Duration::from_millis(100);

/// What a click on a piece does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceAction {
    /// The view becomes the active view, as a click on its strip.
    Activate,
    /// `engine.restart` for the view: its engine stopped, failed or its window was closed.
    Restart,
    /// `view.external.raise`: the external windows to the front.
    Raise,
    /// Marks the element behind the vital in the view (RB-060-10, `highlightVital`); the vitals
    /// bar's click (RB-090-71).
    Mark(rb_sync::VitalMetric),
}

/// One piece of a label strip, or the vitals bar (RB-090-71).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    /// What the strip shows.
    pub text: String,
    /// The details, one per line.
    pub tooltip: String,
    /// What a click does.
    pub action: PieceAction,
}

/// What the label strip of one view shows beside its name; `None` hides a piece.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellInfo {
    /// The view.
    pub view: ViewId,
    /// Its address (`ui.cellLabel.address`).
    pub address: Option<Piece>,
    /// Its external engine's window (`ui.cellLabel.engineStatus`).
    pub engine: Option<Piece>,
    /// `engine` stands in the engine badge beside the engine's name (`ui.engineBadge` on,
    /// RB-090-71), not as a piece of its own.
    pub engine_in_badge: bool,
}

/// What one burst hands out: the label strips' pieces and the vitals bars.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Published {
    infos: Vec<CellInfo>,
    bars: BTreeMap<ViewId, Piece>,
}

/// The pieces of every view as the core holds them now.
pub fn infos_of(core: &Core) -> Vec<CellInfo> {
    let host = core.host();
    let shell = host.shell_state();
    let ui = core.user_settings().ui;
    cell_infos(
        &host.views(),
        host.active_view(),
        &shell.externals,
        &ui.cell_label,
        ui.engine_badge,
    )
}

/// The pieces and the vitals bars as the core holds them now.
fn published(core: &Core) -> Published {
    let metrics = core.host().shell_state().metrics;
    Published {
        infos: infos_of(core),
        bars: crate::view_footer::vitals::bars(&metrics),
    }
}

/// Hands the window every view's pieces and the host every view's vitals bar.
pub fn publish(core: &Core) {
    hand_over(core, published(core));
}

/// The pieces to the window's strips and to the shell state, which the canvas board's labels
/// read (a change there sends the shell the board again); the vitals bars to the host's footers.
fn hand_over(core: &Core, published: Published) {
    let Published { infos, bars } = published;
    core.host()
        .update_shell_state(|shell| shell.cell_infos.clone_from(&infos));
    core.ui().set_cell_info(infos);
    core.host().set_footer_vitals(bars);
}

/// The core is up: the window gets the pieces and the host the vitals bars, and again after
/// every burst of view events that changed one.
pub fn on_start(core: &Core) {
    publish(core);
    let Some(strong) = core.arc() else {
        return;
    };
    let weak = Arc::downgrade(&strong);
    // Subscribed here, not in the task: an event right after the start must reach it.
    let mut events = core.host().subscribe();
    core.runtime().spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        let mut last: Option<Published> = None;
        loop {
            match events.recv().await {
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return,
            }
            tokio::time::sleep(COALESCE).await;
            while events.try_recv().is_ok() {}
            let Some(core) = weak.upgrade() else {
                return;
            };
            let now = published(&core);
            if last.as_ref() != Some(&now) {
                hand_over(&core, now.clone());
                last = Some(now);
            }
        }
    });
}

/// The user settings changed: the pieces follow `ui.cellLabel` and `ui.engineBadge` at once.
pub fn on_user_settings_changed(core: &Core) {
    publish(core);
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
