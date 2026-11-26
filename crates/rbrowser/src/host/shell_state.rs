//! What the shell shows beside the views (contract 0.1-w1b): workspace trust (RB-010-09), the
//! environment, preset and focus of the layout (RB-010-12), the inspected view (RB-010-06) and
//! the CSS breakpoints the analyser found (RB-010-13). Each job writes its own fields through
//! [`Host::update_shell_state`]; the IPC snapshot reads them all.

use std::sync::Arc;

use rb_config::LayoutMode;
use rb_engine::{ViewEvent, ViewId};
use rb_ipc::{DetectedBreakpoint, DevtoolsState, RecorderStatus, SyncState};
use serde_json::Value;

use tokio::runtime::Handle;

use super::Host;
use super::ui::UiBackend;
use crate::capture::CaptureTarget;
use crate::cdp::{CdpRouter, lock};
use crate::emulation::cdp::metrics_params;

/// The shell's state beside the views.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShellState {
    /// The open project is trusted in its current form (RB-010-09); `false` until decided.
    pub trusted: bool,
    /// The current environment id (RB-010-12).
    pub environment: Option<String>,
    /// The current layout preset; `None` for the top-level `layout` (RB-010-12).
    pub preset: Option<String>,
    /// Grid or focus (RB-010-12).
    pub mode: LayoutMode,
    /// Focus mode: the breakpoint shown (RB-010-12).
    pub focus: Option<String>,
    /// Focus mode: the mode it was switched on from, which switching it off returns to
    /// (horizontal, split or canvas too, RB-080-04); `None` outside focus mode.
    pub focus_return: Option<LayoutMode>,
    /// Open DevTools (RB-010-06).
    pub devtools: Option<DevtoolsState>,
    /// The analyser's last result, by width (RB-010-13; `Leader B 1...9` of RB-010-16).
    pub detected: Vec<DetectedBreakpoint>,
    /// The sync state: switches, excluded and desynced views (RB-020-02); `None` until the
    /// SyncHub runs.
    pub sync: Option<SyncState>,
    /// The views on Firefox or WebKit with their status (RB-040-01); empty while
    /// `engines.external.showTiles` is off.
    pub externals: Vec<rb_engine::external::ExternalView>,
    /// The running recording; `None` while none runs (RB-050-01).
    pub recorder: Option<RecorderStatus>,
    /// The last sitemap capture; `None` before the first (RB-050-05).
    pub sitemap: Option<rb_ipc::SitemapStatus>,
    /// The Web Vitals and resources per view (RB-060-10, `crate::vitals`).
    pub metrics: Vec<rb_ipc::ViewMetrics>,
    /// The frameworks each view's page runs (RB-080-24, `crate::sync_agent::framework`).
    pub frameworks: Vec<rb_ipc::ViewFrameworks>,
    /// The session's side of the layout modes (RB-080-04, `crate::layout`).
    pub modes: crate::layout::spec::ModeSession,
    /// The running video recording (RB-090-67, `crate::activity_bar::publish_video`).
    pub video: Option<rb_ipc::VideoActivity>,
    /// The activity bar's place and folded entries (RB-090-67, `crate::activity_bar`).
    pub activity_bar: rb_ipc::ActivityBarState,
    /// Each view's label strip pieces as published last (RB-090-27, `crate::cell_info`): the
    /// canvas board draws them on its labels (RB-090-69).
    pub cell_infos: Vec<crate::cell_info::CellInfo>,
    /// The views' clock and animation rates while changed (RB-094-07, `crate::time_control`).
    pub time_control: Option<rb_ipc::TimeControlActivity>,
    /// The plugin calls running that reported progress (RB-093-01,
    /// `crate::plugins::services::progress`).
    pub plugin_progress: Vec<rb_ipc::PluginProgress>,
    /// The views with forced pseudo-class states (RB-094-16, `crate::force_state::publish`).
    pub forced_states: Vec<rb_ipc::ViewForcedStates>,
    /// The devices the menus offer (RB-094-20, `crate::engines::android::menu`).
    pub devices: Option<rb_ipc::DevicesMenu>,
}

impl ShellState {
    /// The state of a new window: the grid the host builds from the config, until the start
    /// selection is applied (`presets::on_start` - the canvas when the project names no start
    /// preset, RB-099-26). The built-in mode is no state the views are in yet.
    #[must_use]
    pub fn at_open() -> Self {
        Self {
            mode: LayoutMode::Grid,
            ..Self::default()
        }
    }
}

impl Host {
    /// The shell's state beside the views.
    pub fn shell_state(&self) -> ShellState {
        lock(&self.shell).clone()
    }

    /// Changes the shell's state with `change`; when it changed, the shell gets the new state.
    pub fn update_shell_state(&self, change: impl FnOnce(&mut ShellState)) {
        let changed = {
            let mut shell = lock(&self.shell);
            let before = shell.clone();
            change(&mut shell);
            *shell != before
        };
        if changed {
            self.emit(ViewEvent::StateChanged);
        }
    }

    /// Each view's label strip pieces as published last ([`ShellState::cell_infos`]).
    pub fn cell_infos(&self) -> Vec<crate::cell_info::CellInfo> {
        lock(&self.shell).cell_infos.clone()
    }

    /// Something the snapshot reads from outside the shell's state changed - a view's page
    /// zoom, which the canvas board's labels show (RB-090-28): the shell gets the new state.
    pub fn state_changed(&self) {
        self.emit(ViewEvent::StateChanged);
    }

    /// The window side (the DevTools dock, the frame colour; contract 0.1-w1b).
    pub fn ui(&self) -> &Arc<dyn UiBackend> {
        &self.ui
    }

    /// Sends `view`'s emulation again, all of it (after DevTools' device toolbar, RB-010-06; a
    /// changed trust, RB-010-09).
    pub fn reapply_emulation(&self, view: ViewId) {
        self.spawn_apply(view, true);
    }

    /// The CDP router on the pipe (DevTools' internal endpoint, RB-010-06).
    pub fn cdp(&self) -> &Arc<CdpRouter> {
        &self.cdp
    }

    /// `view`'s CDP target, once the view was attached (RB-010-06).
    pub fn target_id(&self, view: ViewId) -> Option<String> {
        self.state().views.get(&view)?.target_id.clone()
    }

    /// What a screenshot needs of `view` (RB-050-03): its session, the device metrics the host
    /// sent (to send them again after the capture), the breakpoint's height and whether the
    /// view is hidden; `None` before the attach.
    pub fn capture_target(&self, view: ViewId) -> Option<CaptureTarget> {
        let state = self.state();
        let entry = state.views.get(&view)?;
        let metrics = entry.applied.unwrap_or_else(|| state.metrics_of(entry));
        Some(CaptureTarget {
            session: entry.session.clone()?,
            metrics: metrics_params(&metrics, entry.emulation.orientation),
            height: entry.emulation.height,
            dpr: entry.emulation.dpr,
            hidden: entry.hidden,
        })
    }

    /// The parameters of `Emulation.setDeviceMetricsOverride` that show `view` at `width` CSS px,
    /// with its own height, DPR and zoom, fitted into its place as a breakpoint of that width
    /// would be: a plugin's width lease (RB-093-01). `dontSetVisibleSize` keeps the widget as the
    /// window laid it out, so a wide width never pushes the view over its neighbours. Nothing is
    /// sent or kept here: the host's own metrics stay what [`Host::capture_target`] answers.
    /// `None` before the attach.
    pub fn width_metrics(&self, view: ViewId, width: u32) -> Option<Value> {
        let state = self.state();
        let entry = state.views.get(&view)?;
        entry.session.as_ref()?;
        let mut emulation = entry.emulation.clone();
        emulation.width = width;
        let metrics = state.metrics_for(entry, &emulation);
        let mut params = metrics_params(&metrics, emulation.orientation);
        if let Some(object) = params.as_object_mut() {
            object.insert("dontSetVisibleSize".to_owned(), Value::Bool(true));
        }
        Some(params)
    }

    /// The runtime the host's tasks run on (RB-050-03's captures).
    pub fn runtime(&self) -> &Handle {
        &self.runtime
    }

    /// Every attached view with its flat CDP session (MCP's log buffers, RB-030-01).
    pub fn cdp_sessions(&self) -> Vec<(ViewId, String)> {
        self.state()
            .views
            .iter()
            .filter_map(|(view, entry)| Some((*view, entry.session.clone()?)))
            .collect()
    }
}

#[cfg(test)]
#[path = "shell_state_tests.rs"]
mod tests;
