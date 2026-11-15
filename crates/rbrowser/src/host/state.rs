//! The host's state: the views, their cells and emulation, and the pure rules over them.

use std::collections::BTreeMap;

use rb_config::{Breakpoint, EmulationFields, EngineKind, Layout, ProjectConfig, Zoom};
use rb_engine::{CellRect, Emulation, GridCell, ViewId, ViewInfo};

use crate::fit::{DeviceMetrics, device_metrics_up_to};

/// One view.
#[derive(Clone, Debug)]
pub struct ViewState {
    /// The id.
    pub id: ViewId,
    /// The breakpoint shown now; `None` after a free size.
    pub breakpoint: Option<Breakpoint>,
    /// The breakpoint of the view's cell.
    pub cell_breakpoint: Option<String>,
    /// The view's cell, by `layout.cells` index.
    pub cell_index: Option<usize>,
    /// The cell's zoom.
    pub zoom: Zoom,
    /// A zoom set with `view.zoom_in` / `view.zoom_out`, until `view.zoom_fit_all`.
    pub zoom_override: Option<f64>,
    /// The emulation the view should have: its breakpoint resolved, the live overrides on top.
    pub emulation: Emulation,
    /// The fields `emulation.set` put over the breakpoint, until `emulation.reset_view`
    /// (RB-010-05).
    pub overrides: EmulationFields,
    /// The view's rectangle as CEF laid it out (DIP, client area).
    pub measured: Option<CellRect>,
    /// The metrics last sent over CDP.
    pub applied: Option<DeviceMetrics>,
    /// The emulation last sent over CDP, headers as allowed then; `None` for a new session.
    pub applied_emulation: Option<Emulation>,
    /// Whether the view was reported hidden (zero size) to the browser.
    pub hidden: bool,
    /// A layout mode placed the view outside the grid area: it is not shown and is throttled
    /// like a view without area (RB-080-04).
    pub off_screen: bool,
    /// The emulated size (width, height) a layout mode last placed the view at; another size
    /// places the views again before its metrics go out (RB-080-54).
    pub placed_size: Option<(u32, u32)>,
    /// The CDP target, captured at creation.
    pub target_id: Option<String>,
    /// The flat CDP session, while attached.
    pub session: Option<String>,
    /// An attach is under way.
    pub attaching: bool,
    /// A URL to load once the session is attached.
    pub pending_url: Option<String>,
    /// The current URL.
    pub url: String,
    /// The page title.
    pub title: String,
    /// Whether a load is in progress.
    pub loading: bool,
    /// How many loads have ended: the SyncHub tells a load's end by it, even when the start
    /// and the end both happened before it looked (RB-020-02).
    pub loads_ended: u64,
    /// The session (cookie and storage context) the view's browser was created in; `None` is
    /// `default` (RB-020-07).
    pub context_session: Option<String>,
    /// The session was chosen with `session.assign`, so a config change does not move the view.
    pub session_chosen: bool,
    /// The origin the geolocation permission is granted for on this view's behalf (RB-080-10).
    pub geo_origin: Option<String>,
    /// The `browserContextId` of the view's target, read at the attach; the permission calls
    /// name it. `None` when the browser named none.
    pub browser_context: Option<String>,
    /// The view is in a window of its own (RB-080-14); `None` in the grid.
    pub detached: Option<Detached>,
}

/// A view popped out of the grid into a window of its own (RB-080-14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Detached {
    /// Its cell keeps a placeholder (`popOut.keepPlaceholder`); else the other views close the
    /// gap.
    pub placeholder: bool,
}

impl ViewState {
    /// A new view for `breakpoint`.
    pub fn new(id: ViewId, breakpoint: &Breakpoint, emulation: Emulation) -> Self {
        Self {
            id,
            breakpoint: Some(breakpoint.clone()),
            cell_breakpoint: Some(breakpoint.id.clone()),
            cell_index: None,
            zoom: Zoom::Fit,
            zoom_override: None,
            emulation,
            overrides: EmulationFields::default(),
            measured: None,
            applied: None,
            applied_emulation: None,
            hidden: false,
            off_screen: false,
            placed_size: None,
            target_id: None,
            session: None,
            attaching: false,
            pending_url: None,
            url: String::new(),
            title: String::new(),
            loading: true,
            loads_ended: 0,
            context_session: breakpoint.session.clone(),
            session_chosen: false,
            geo_origin: None,
            browser_context: None,
            detached: None,
        }
    }

    /// The session the view's browser lives in.
    pub fn context_session(&self) -> &str {
        self.context_session
            .as_deref()
            .unwrap_or(rb_config::DEFAULT_SESSION)
    }

    /// The zoom in effect: the override, else the cell's.
    pub fn effective_zoom(&self) -> Zoom {
        self.zoom_override.map_or(self.zoom, Zoom::Factor)
    }
}

/// Everything the host knows.
#[derive(Clone, Debug, Default)]
pub struct HostState {
    /// The project config in effect.
    pub config: ProjectConfig,
    /// Breakpoints opened that are not in the config (`open_view`).
    pub extra_breakpoints: Vec<Breakpoint>,
    /// The grid in effect, the cells of the hidden breakpoints included.
    pub layout: Layout,
    /// The hidden selection the grid was built with (RB-080-41): the breakpoints of
    /// `layout.hidden` that are known, none when every one would be hidden.
    pub hidden: Vec<String>,
    /// The views in grid order.
    pub order: Vec<ViewId>,
    /// The views.
    pub views: BTreeMap<ViewId, ViewState>,
    /// The next view id; ids are never reused.
    pub next_id: u32,
    /// The active view.
    pub active: Option<ViewId>,
    /// The grid area as CEF laid it out.
    pub grid_area: Option<CellRect>,
    /// The computed cells for `grid_area`.
    pub cells: Vec<GridCell>,
    /// The layout mode in effect (RB-080-04); `None` for grid and focus.
    pub mode: Option<crate::layout::ModeSpec>,
    /// The footers under the views (RB-080-44).
    pub footer: super::footer::FooterState,
    /// The main window is closing (RB-080-91): no view attaches again, its browser goes.
    pub closing: bool,
}

impl HostState {
    /// The grid without the hidden cells: what the cells are computed from (RB-080-41).
    pub fn visible_layout(&self) -> Layout {
        crate::layout_plan::visible_cells(&self.layout, &self.hidden)
    }

    /// The breakpoint `id`, from the config or opened later.
    pub fn breakpoint(&self, id: &str) -> Option<&Breakpoint> {
        self.config
            .breakpoints
            .iter()
            .chain(&self.extra_breakpoints)
            .find(|breakpoint| breakpoint.id == id)
    }

    /// A fresh view id.
    pub fn allocate_id(&mut self) -> ViewId {
        self.next_id += 1;
        ViewId(self.next_id)
    }

    /// The device metrics `view` should have now: its measured rectangle, else its computed
    /// cell. In a window of its own the page is at its real size (RB-080-14).
    pub fn metrics_of(&self, view: &ViewState) -> DeviceMetrics {
        let rect = self.rect_of(view).unwrap_or_default();
        if view.detached.is_some() {
            return crate::pop_out::metrics(&view.emulation, rect);
        }
        // A canvas view fills its place past its page's size (RB-080-94).
        let canvas = self
            .mode
            .as_ref()
            .is_some_and(|spec| spec.mode() == rb_config::LayoutMode::Canvas);
        let max = if canvas {
            crate::fit::MAX_CANVAS_SCALE
        } else {
            crate::fit::MAX_ZOOM
        };
        device_metrics_up_to(&view.emulation, rect, view.effective_zoom(), max)
    }

    /// Where `view` is: its measured rectangle, else its computed cell; `None` before the
    /// window laid it out.
    pub fn rect_of(&self, view: &ViewState) -> Option<CellRect> {
        view.measured.or_else(|| {
            view.cell_index
                .and_then(|index| self.cells.get(index))
                .map(|cell| cell.rect)
        })
    }

    /// What the shell and MCP see of `view`.
    pub fn info(&self, view: &ViewState) -> ViewInfo {
        let metrics = view.applied.unwrap_or_else(|| self.metrics_of(view));
        let label = match &view.breakpoint {
            Some(breakpoint) => breakpoint
                .label
                .clone()
                .unwrap_or_else(|| breakpoint.id.clone()),
            None => format!("{}x{}", view.emulation.width, view.emulation.height),
        };
        ViewInfo {
            id: view.id,
            label,
            breakpoint: view.breakpoint.as_ref().map(|b| b.id.clone()),
            cell_breakpoint: view.cell_breakpoint.clone(),
            engine: view
                .breakpoint
                .as_ref()
                .map_or(EngineKind::Chromium, |b| b.engine),
            width: metrics.width,
            height: metrics.height,
            dpr: view.emulation.dpr,
            scale: metrics.scale,
            url: view.url.clone(),
            title: view.title.clone(),
            loading: view.loading,
            session: view.context_session().to_owned(),
            cell: view
                .cell_index
                .and_then(|index| self.cells.get(index))
                .cloned(),
        }
    }
}

/// Which existing view each wanted cell keeps: per cell (in order) the first unused view whose
/// cell had the same breakpoint, else `None` (a new view). `existing` is in grid order.
pub fn match_views(existing: &[(ViewId, Option<String>)], wanted: &[&str]) -> Vec<Option<ViewId>> {
    let mut used = vec![false; existing.len()];
    wanted
        .iter()
        .map(|breakpoint| {
            let found = existing.iter().enumerate().position(|(index, (_, cell))| {
                !used[index] && cell.as_deref() == Some(*breakpoint)
            })?;
            used[found] = true;
            Some(existing[found].0)
        })
        .collect()
}

/// The emulation of `breakpoint`, resolved against the project `defaults` and the device presets
/// in effect (RB-010-05). One that does not resolve (an unknown device, a value a browser
/// refuses) keeps its size, DPR, touch and mobile, with a warning. A DPR of `0` keeps the
/// display's.
pub fn emulation_for(breakpoint: &Breakpoint, defaults: &EmulationFields) -> Emulation {
    let devices = crate::emulation::device_list();
    match rb_engine::resolve_emulation(breakpoint, defaults, &devices) {
        Ok(emulation) => emulation,
        Err(error) => {
            tracing::warn!(breakpoint = %breakpoint.id, %error, "the emulation is size and touch only");
            plain_emulation(breakpoint, defaults, &devices)
        }
    }
}

/// Size, DPR, touch and mobile of `breakpoint`, nothing else. The size is the breakpoint's
/// viewport (its own sides, else its device preset's, RB-010-12); one without (a preset that
/// went missing after the config was checked) keeps the engine's default size.
fn plain_emulation(
    breakpoint: &Breakpoint,
    defaults: &EmulationFields,
    devices: &[rb_config::DevicePreset],
) -> Emulation {
    let own = &breakpoint.emulation;
    let touch = own.touch.or(defaults.touch).unwrap_or(false);
    let dpr = own.dpr.or(defaults.dpr).unwrap_or(0.0);
    let fallback = Emulation::default();
    let (width, height) = breakpoint
        .viewport(devices)
        .map_or((fallback.width, fallback.height), |viewport| {
            (viewport.width, viewport.height)
        });
    Emulation {
        width,
        height,
        dpr: if dpr > 0.0 && dpr <= 10.0 { dpr } else { 0.0 },
        mobile: own.mobile.or(defaults.mobile).unwrap_or(touch),
        touch,
        ..Emulation::default()
    }
}

/// The layout when the config has breakpoints but no cells: one row, one cell per Chromium
/// breakpoint not hidden (RB-080-41), in config order.
pub fn layout_for(config: &ProjectConfig) -> Layout {
    if !config.layout.cells.is_empty() {
        return config.layout.clone();
    }
    let hidden = rb_config::hidden_selection(config);
    let cells: Vec<rb_config::LayoutCell> = config
        .breakpoints
        .iter()
        .filter(|breakpoint| {
            breakpoint.engine == EngineKind::Chromium && !hidden.contains(&breakpoint.id)
        })
        .enumerate()
        .map(|(index, breakpoint)| rb_config::LayoutCell {
            breakpoint: breakpoint.id.clone(),
            col: index as u32 + 1,
            row: 1,
            col_span: 1,
            row_span: 1,
            zoom: Zoom::Fit,
        })
        .collect();
    Layout {
        columns: (cells.len() as u32).max(1),
        rows: 1,
        gap: config.layout.gap,
        cells,
        ..Default::default()
    }
}

/// The layout with one more column holding `breakpoint` over every row (`open_view`).
pub fn layout_with_extra_column(layout: &Layout, breakpoint: &str) -> Layout {
    let mut extended = layout.clone();
    extended.columns += 1;
    extended.cells.push(rb_config::LayoutCell {
        breakpoint: breakpoint.to_owned(),
        col: extended.columns,
        row: 1,
        col_span: 1,
        row_span: extended.rows.max(1),
        zoom: Zoom::Fit,
    });
    extended
}

/// `layout` without the cells of `breakpoint`; the column [`layout_with_extra_column`] added
/// goes with them when it is the last one (`close_extra_view`, RB-080-41).
pub fn layout_without_extra_column(layout: &Layout, breakpoint: &str) -> Layout {
    let mut shrunk = layout.clone();
    let last_column = shrunk
        .cells
        .iter()
        .any(|cell| cell.breakpoint == breakpoint && cell.col == shrunk.columns);
    shrunk.cells.retain(|cell| cell.breakpoint != breakpoint);
    let in_last = shrunk
        .cells
        .iter()
        .any(|cell| cell.col + cell.col_span > shrunk.columns);
    if last_column && !in_last && shrunk.columns > 1 {
        shrunk.columns -= 1;
    }
    shrunk
}

/// The config when no project is open: desktop, tablet and mobile side by side on
/// `about:blank`.
pub fn default_config() -> ProjectConfig {
    let breakpoint = |id: &str, width: u32, height: u32, touch: bool| Breakpoint {
        id: id.to_owned(),
        label: None,
        width: Some(width),
        height: Some(height),
        device: None,
        engine: EngineKind::Chromium,
        session: None,
        sync_navigation: true,
        shortcut: None,
        color: None,
        emulation: EmulationFields {
            touch: Some(touch),
            ..EmulationFields::default()
        },
    };
    let mut config = ProjectConfig {
        version: rb_config::CURRENT_VERSION,
        start_url: Some("about:blank".to_owned()),
        breakpoints: vec![
            breakpoint("desktop", 1440, 900, false),
            breakpoint("tablet", 768, 1024, true),
            breakpoint("mobile", 393, 852, true),
        ],
        ..ProjectConfig::default()
    };
    config.layout = layout_for(&config);
    config
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
