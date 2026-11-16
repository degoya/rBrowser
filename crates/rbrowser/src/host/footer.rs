//! The host's half of the view footer (RB-080-44, `crate::view_footer`): which footer each view
//! shows - written from its breakpoint, its size, the config's breakpoints and the analyser's
//! last boundaries - the 18 DIP a view gives each of its lines (two in a narrow view,
//! RB-080-53, `footer_lines.rs`), and keeping the strips current. A view whose
//! chosen fields all have no value has no footer and keeps its height. The colour is read from
//! the config in effect at every write, so `breakpoints[].color` holds as soon as it is saved.
//!
//! The footer takes its room from the view's own rectangle in every layout, so no layout mode's
//! geometry changes; a view's device metrics follow the rectangle the window reports. A view in
//! a window of its own has its strip there: the grid builds none for it, so every grid sent is
//! followed by that window's footer. The host starts without footers: the core hands it
//! `ui.viewFooter` (`crate::view_footer::on_start`).
//!
//! What changes a footer and who says so: a view's size, breakpoint or emulation (the task
//! here, on `Updated`, for that view only), a rebuilt grid or a new config (`with_footers`,
//! `Host::replace_config`), a new analysis (`Host::footer_analysed`) and a view that left the
//! analysed document (the task, on `Navigated`: its queries go until the next analysis).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rb_config::{FooterField, Layout, ViewFooterSettings};
use rb_engine::{CellRect, GridCell, ViewEvent, ViewId, compute_grid};
use rb_ipc::DetectedBreakpoint;
use tokio::sync::broadcast::error::RecvError;

use super::state::HostState;
use super::ui::{UiCell, UiGrid};
use super::{Host, label_height};
use crate::cdp::lock;
use crate::view_footer::auto::AutoState;
use crate::view_footer::results::{choose, own_counts};
use crate::view_footer::{
    CellFooter, FOOTER_HEIGHT, FooterInput, Range, Results, above_footer, background, document_of,
    foreground, range, text,
};

#[path = "footer_lines.rs"]
mod lines;
#[path = "footer_own.rs"]
mod own;
pub use own::ReadStart;

/// The tooltip's line about stylesheets the footer's read could not open.
const SKIPPED_KEY: &str = "footer.skipped";
/// The tooltip's line about a page with more media lists than one read takes.
const CAPPED_KEY: &str = "footer.capped";

/// What the host knows of the footers.
#[derive(Clone, Debug, Default)]
pub struct FooterState {
    /// `ui.viewFooter`; `None` until the core hands it over: no footers.
    pub settings: Option<ViewFooterSettings>,
    /// The footer the window shows for each view; a view without one has no strip.
    pub sent: BTreeMap<ViewId, CellFooter>,
    /// The widths of the config's breakpoints, until the config or the grid changes.
    widths: Option<Vec<u32>>,
    /// The document each view showed last (its URL without the fragment).
    documents: BTreeMap<ViewId, String>,
    /// The views that left the document the last analysis saw: no queries until the next one.
    left: BTreeSet<ViewId>,
    /// What the footer read from the documents the views show (`ui.viewFooter.analyze`,
    /// RB-080-47), beside what Analyze Breakpoints found (`view_footer::results::choose`).
    own: Results,
    /// The waits, the switch and the reads under way of those reads.
    auto: AutoState,
    /// Counts what makes a read under way worthless: the setting switched off, a config that
    /// changed what an analysis says. A read started under another number is not kept.
    generation: u64,
    /// The mark of the config the kept results were analysed with.
    config_mark: Option<u64>,
    /// A changed config dropped the results: the views are to be read again.
    stale: bool,
    /// What each footer was fitted to: a layout change fits again only where a width changed.
    fits: BTreeMap<ViewId, lines::Fit>,
    /// How many footer texts were written; the tests count the work a change costs.
    #[cfg(test)]
    pub built: usize,
}

impl FooterState {
    /// The DIP `view` gives its footer, a line height per line; 0 for a view without one.
    pub fn height_of(&self, view: ViewId) -> u32 {
        self.sent.get(&view).map_or(0, CellFooter::height)
    }

    /// Notes that `view` shows `url`; `true` when that is another document than before. A view
    /// that left a document is not on the ground of the last Analyze Breakpoints any more; the
    /// footer's own result of the new document, when one is kept, counts at once.
    fn navigated(&mut self, view: ViewId, url: &str) -> bool {
        let document = document_of(url);
        match self.documents.get(&view) {
            Some(known) if known == document => false,
            known => {
                if known.is_some() {
                    self.left.insert(view);
                }
                self.documents.insert(view, document.to_owned());
                true
            }
        }
    }
}

/// Whether the window builds a footer strip in cell `index` of `grid`: the grid has footers,
/// the view has one, and the view is in its cell - a popped-out view's cell holds a placeholder
/// (or nothing), its strip is in its window.
pub fn cell_has_strip(grid: &UiGrid, index: usize) -> bool {
    grid.footer_height > 0
        && grid
            .cells
            .get(index)
            .is_some_and(|cell| cell.footer.is_some() && grid.detached(cell.view).is_none())
}

/// The cells of `layout` in `area`, each view above its footer; `state.order` names the view
/// of each cell.
pub(super) fn grid_cells(state: &HostState, layout: &Layout, area: CellRect) -> Vec<GridCell> {
    let mut cells = compute_grid(layout, area, label_height()).unwrap_or_default();
    for (cell, view) in cells.iter_mut().zip(&state.order) {
        cell.rect = above_footer(cell.rect, state.footer.height_of(*view));
    }
    cells
}

/// The widths of the config's breakpoints (a device preset's where the breakpoint names none),
/// kept until [`FooterState::widths`] is cleared.
fn config_widths(state: &mut HostState) -> Vec<u32> {
    if let Some(widths) = &state.footer.widths {
        return widths.clone();
    }
    let breakpoints = &state.config.breakpoints;
    // The device presets only when a breakpoint takes its width from one.
    let devices = if breakpoints
        .iter()
        .any(|breakpoint| breakpoint.width.is_none())
    {
        crate::emulation::device_list()
    } else {
        Vec::new()
    };
    let widths: Vec<u32> = breakpoints
        .iter()
        .filter_map(|breakpoint| breakpoint.viewport(&devices))
        .map(|viewport| viewport.width)
        .collect();
    state.footer.widths = Some(widths.clone());
    widths
}

/// The footer of every view of `state` - of `only` that view - that has something to say, its
/// lines fitted to the room `strips` names for its view; none without footers.
pub(super) fn footers(
    state: &mut HostState,
    detected: &[DetectedBreakpoint],
    only: Option<ViewId>,
    strips: &BTreeMap<ViewId, lines::Room>,
) -> BTreeMap<ViewId, CellFooter> {
    if !state
        .footer
        .settings
        .as_ref()
        .is_some_and(ViewFooterSettings::shown)
    {
        return BTreeMap::new();
    }
    let widths = config_widths(state);
    // The results the footers name are the ones used last.
    let shown: Vec<String> = state.footer.documents.values().cloned().collect();
    for document in &shown {
        state.footer.own.touch(document);
    }
    // The site Analyze Breakpoints reads: `startUrl` and `analyzer.urls`.
    let origins: BTreeSet<String> = crate::analyzer::run::pages(&state.config, &[], false)
        .iter()
        .filter_map(|page| rb_util::url::origin_of(page))
        .collect();
    #[cfg(test)]
    {
        let count = state
            .order
            .iter()
            .filter(|id| only.is_none_or(|only| only == **id))
            .count();
        state.footer.built += count;
    }
    let written: Vec<(ViewId, CellFooter, lines::Fit)> = {
        let state = &*state;
        let Some(settings) = state.footer.settings.as_ref() else {
            return BTreeMap::new();
        };
        let known = || {
            state
                .config
                .breakpoints
                .iter()
                .chain(&state.extra_breakpoints)
        };
        state
            .order
            .iter()
            .filter(|id| only.is_none_or(|only| only == **id))
            .filter_map(|id| state.views.get(id))
            .filter_map(|entry| {
                let shown = entry.breakpoint.as_ref();
                // The colour is the config's, not the copy the view was opened with: a colour set
                // in the settings holds at once (RB-080-47). A free size keeps the colour of its
                // cell's breakpoint.
                let source = shown
                    .and_then(|shown| known().find(|known| known.id == shown.id))
                    .or_else(|| {
                        let cell = entry.cell_breakpoint.as_deref()?;
                        state.breakpoint(cell)
                    });
                let index = source
                    .and_then(|source| known().position(|known| known.id == source.id))
                    .or(entry.cell_index)
                    .unwrap_or(0);
                let background = background(source.and_then(|b| b.color.as_deref()), index);
                // The footer's own read of the view's document beside the result of Analyze
                // Breakpoints. That one counts for a view on the analysed ground - or, beside an
                // own read, for a view on a page of the analysed site.
                let document = state.footer.documents.get(&entry.id);
                let own = document.and_then(|document| state.footer.own.get(document));
                let on_ground = !state.footer.left.contains(&entry.id);
                let of_site = || {
                    document
                        .and_then(|document| rb_util::url::origin_of(document))
                        .is_some_and(|origin| origins.contains(&origin))
                };
                let manual = (on_ground || (own.is_some() && of_site())).then_some(detected);
                let chosen = choose(own, manual);
                let mut text = text(&FooterInput {
                    breakpoint: shown.map(|shown| shown.id.as_str()),
                    width: entry.emulation.width,
                    height: entry.emulation.height,
                    dpr: entry.emulation.dpr,
                    config_widths: &widths,
                    detected: &chosen,
                    fields: &settings.fields,
                });
                // What the own read left out, when it is the one named.
                if let Some(own) = own.filter(|_| own_counts(own, manual))
                    && settings.fields.contains(&FooterField::Queries)
                {
                    let skipped = (own.skipped > 0).then(|| {
                        rb_i18n::t_with(SKIPPED_KEY, &[("count", &own.skipped.to_string())])
                    });
                    let capped = own.capped.then(|| rb_i18n::t(CAPPED_KEY));
                    for note in skipped.into_iter().chain(capped) {
                        if !text.tooltip.is_empty() {
                            text.tooltip.push('\n');
                        }
                        text.tooltip.push_str(&note);
                    }
                }
                // Nothing to say: no strip, and the view keeps its height.
                (!text.text.is_empty()).then(|| {
                    let mut footer = CellFooter {
                        text: text.text,
                        tooltip: text.tooltip,
                        background,
                        foreground: foreground(background),
                        ..CellFooter::default()
                    };
                    let fit = lines::Fit {
                        room: strips.get(&entry.id).copied(),
                        tooltip: footer.tooltip.clone(),
                    };
                    lines::shape(&mut footer, &fit, settings);
                    (entry.id, footer, fit)
                })
            })
            .collect()
    };
    let mut footers = BTreeMap::new();
    for (view, footer, fit) in written {
        state.footer.fits.insert(view, fit);
        footers.insert(view, footer);
    }
    footers
}

impl Host {
    /// `ui.viewFooter` changed (or is handed over at the start): the strips appear, go or are
    /// written again, and a view takes or gives back its footer's height.
    pub fn set_footer_settings(&self, settings: ViewFooterSettings) {
        {
            let mut state = self.state();
            if state.footer.settings.as_ref() == Some(&settings) {
                return;
            }
            state.footer.settings = Some(settings);
        }
        self.refresh_footers();
    }

    /// The DIP `view` gives its footer now; 0 for a view without one.
    pub fn footer_height(&self, view: ViewId) -> u32 {
        self.state().footer.height_of(view)
    }

    /// The range of widths `view` stands for among the config's breakpoints (`views_list`);
    /// `None` for an unknown view.
    pub fn footer_range(&self, view: ViewId) -> Option<Range> {
        let mut state = self.state();
        let width = state.views.get(&view)?.emulation.width;
        Some(range(width, &config_widths(&mut state)))
    }

    /// The analyser ran: every view is on analysed ground again, and the footers name the
    /// queries it found.
    pub fn footer_analysed(&self) {
        self.state().footer.left.clear();
        self.refresh_footers();
    }

    /// The config changed without a new grid (`Host::replace_config`): its breakpoints may have.
    pub(super) fn footer_config_changed(&self) {
        {
            let mut state = self.state();
            state.footer.widths = None;
            own::note_config(&mut state);
        }
        self.refresh_footers();
    }

    /// Gives every cell of `cells` its footer, notes what the window shows and computes the
    /// grid's cells for it; answers the grid's footer height (0 without a footer). Before the
    /// places of a layout mode are computed: they follow what is noted here. The windows of the
    /// popped-out views, which the grid does not build, get their footers at once.
    pub(super) fn with_footers(&self, cells: &mut [UiCell]) -> u32 {
        let detected = lock(&self.shell).detected.clone();
        let (height, windows) = {
            let mut state = self.state();
            // A new grid: the config behind it may be new too.
            state.footer.widths = None;
            own::note_config(&mut state);
            // Fitted to the cells of this grid, not the window's report of the one before.
            let widths = lines::layout_widths(&state);
            let footers = footers(&mut state, &detected, None, &widths);
            for cell in cells {
                cell.footer = footers.get(&cell.view).and_then(CellFooter::shown).cloned();
            }
            let shown = footers.values().any(|footer| !footer.folded);
            let height = if shown {
                crate::text_scale::dip(FOOTER_HEIGHT)
            } else {
                0
            };
            state
                .footer
                .fits
                .retain(|view, _| footers.contains_key(view));
            state.footer.sent = footers;
            let views: BTreeSet<ViewId> = state.views.keys().copied().collect();
            state
                .footer
                .documents
                .retain(|view, _| views.contains(view));
            state.footer.left.retain(|view| views.contains(view));
            if state.mode.is_none()
                && let Some(area) = state.grid_area
            {
                state.cells = grid_cells(&state, &state.visible_layout(), area);
            }
            let windows: Vec<(ViewId, Option<CellFooter>)> = if state.footer.settings.is_some() {
                state
                    .views
                    .values()
                    .filter(|entry| entry.detached.is_some())
                    .map(|entry| {
                        let footer = state.footer.sent.get(&entry.id);
                        (entry.id, footer.and_then(CellFooter::shown).cloned())
                    })
                    .collect()
            } else {
                Vec::new()
            };
            (height, windows)
        };
        for (view, footer) in windows {
            self.ui.set_cell_footer(view, footer);
        }
        height
    }

    /// Writes the footers that changed since the window got them.
    pub fn refresh_footers(&self) {
        self.refresh_footer(None);
    }

    /// Writes the footer of `only` (every view's without it) when it changed; a view whose
    /// footer appeared, went or changed its number of lines gets the grid again.
    fn refresh_footer(&self, only: Option<ViewId>) {
        let detected = lock(&self.shell).detected.clone();
        let changed: Option<Vec<(ViewId, CellFooter)>> = {
            let mut state = self.state();
            if state.order.is_empty() {
                return;
            }
            let widths = lines::cell_widths(&state);
            let next = footers(&mut state, &detected, only, &widths);
            // A footer that appeared, went or got or lost a line: the grid again.
            let reshaped = state
                .order
                .iter()
                .filter(|view| only.is_none_or(|only| only == **view))
                .any(|view| lines::reshapes(*view, state.footer.sent.get(view), next.get(view)));
            if reshaped {
                None
            } else {
                let changed: Vec<(ViewId, CellFooter)> = next
                    .into_iter()
                    .filter(|(view, footer)| state.footer.sent.get(view) != Some(footer))
                    .collect();
                for (view, footer) in &changed {
                    state.footer.sent.insert(*view, footer.clone());
                }
                Some(changed)
            }
        };
        match changed {
            Some(changed) => {
                for (view, footer) in changed {
                    self.ui.set_cell_footer(view, footer.shown().cloned());
                }
            }
            None => self.footer_reshaped(),
        }
    }

    /// A footer appeared, went or got or lost a line: the grid again with its strips as they
    /// are now, and new metrics for the views, whose rectangles changed.
    fn footer_reshaped(&self) {
        self.regrid();
        let views = self.state().order.clone();
        for view in views {
            self.spawn_apply(view, false);
        }
        self.emit(ViewEvent::LayoutChanged);
    }

    /// `view` committed `url`: on another document than before its footer names that
    /// document's queries when the footer read them, else none until the next analysis.
    fn footer_navigated(&self, view: ViewId, url: &str) {
        if self.state().footer.navigated(view, url) {
            self.refresh_footer(Some(view));
        }
    }
}

/// Starts the task that keeps the footers of `host` current.
pub(super) fn watch(host: &Arc<Host>) {
    let mut events = host.events.subscribe();
    let weak = Arc::downgrade(host);
    host.runtime.spawn(async move {
        loop {
            let event = events.recv().await;
            let Some(host) = weak.upgrade() else {
                return;
            };
            match event {
                // A size, a breakpoint, an emulation: that view's footer only.
                Ok(ViewEvent::Updated(view)) => host.refresh_footer(Some(view)),
                Ok(ViewEvent::Navigated { view, url }) => host.footer_navigated(view, &url),
                // A new grid area or new places: the footers of the views whose width changed
                // are fitted again; nothing else is written (RB-080-53).
                Ok(ViewEvent::LayoutChanged) => host.refit_footers(),
                // A grid carries its footers, a config change and an analysis say so
                // themselves, and the active view changes none.
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => host.refresh_footers(),
                Err(RecvError::Closed) => return,
            }
        }
    });
}

#[cfg(test)]
#[path = "footer_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "footer_windows_tests.rs"]
mod window_tests;

#[cfg(test)]
#[path = "footer_color_tests.rs"]
mod color_tests;

#[cfg(test)]
#[path = "footer_own_tests.rs"]
mod own_tests;

#[cfg(test)]
#[path = "footer_wrap_tests.rs"]
mod wrap_tests;
