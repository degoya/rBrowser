//! The live comparison (`env.compare.live`): a temporary grid of the active view's breakpoint
//! once per environment, side by side, each view on the page the active view shows under its
//! own environment. Sync stays on: navigation sync already follows a leader into each
//! follower's own environment (`rb_sync::nav_url::follow_url`: path, query and hash kept, the
//! base swapped; a link to a foreign host is followed as it is, never rewritten). The window's
//! environment frame is cleared while the comparison shows, since it would name one
//! environment for all views. Run again (or `enabled: false`), the grid of the current
//! selection comes back in the window's environment.
//!
//! The grid uses the existing layout API (`ViewHost::set_layout`, a grid of one row); a split
//! mode (RB-080-04) and a per-cell environment frame are left to the layout modes.

use std::sync::Mutex;

use rb_config::{Environment, Layout, LayoutCell, Zoom};
use rb_engine::{Target, ViewHost as _, ViewId};
use rb_util::url::is_web_url;

use super::plan::{self, PlanError, url_in, view_path};
use super::runs::RunError;
use super::tool::{LiveAnswer, LiveArgs, LiveView};
use crate::cdp::lock;
use crate::control::Core;
use crate::presets::actions;

/// Whether the live comparison shows (one window per process).
static LIVE: Mutex<bool> = Mutex::new(false);

/// The grid of the live comparison: `breakpoint` in `count` columns of one row.
pub fn live_layout(breakpoint: &str, count: usize, gap: u32, zoom: Zoom) -> Layout {
    let columns = u32::try_from(count.max(1)).unwrap_or(1);
    Layout {
        columns,
        rows: 1,
        gap,
        cells: (1..=columns)
            .map(|col| LayoutCell {
                breakpoint: breakpoint.to_owned(),
                col,
                row: 1,
                col_span: 1,
                row_span: 1,
                zoom,
            })
            .collect(),
        ..Default::default()
    }
}

/// The URL each of `environments` shows: `shown`'s path under it (`known` are every
/// environment of the project, to find the one `shown` lies under), its base for a page
/// outside http(s).
pub fn live_urls(shown: &str, environments: &[Environment], known: &[Environment]) -> Vec<String> {
    environments
        .iter()
        .map(|environment| {
            if is_web_url(shown) {
                url_in(&environment.url, &view_path(shown, known))
            } else {
                environment.url.clone()
            }
        })
        .collect()
}

/// Whether the live comparison shows now.
pub fn is_live() -> bool {
    *lock(&LIVE)
}

fn engine(error: impl std::fmt::Display) -> RunError {
    RunError::Failed(error.to_string())
}

/// `env.compare.live` and `env_compare_live`: shows (or, toggled, leaves) the comparison.
///
/// # Errors
///
/// Fewer than two environments, no view to compare, or the grid refused.
pub fn live(core: &Core, args: &LiveArgs) -> Result<LiveAnswer, RunError> {
    let wanted = args.enabled.unwrap_or(!is_live());
    if !wanted {
        if is_live() {
            leave(core)?;
        }
        return Ok(LiveAnswer {
            enabled: false,
            views: Vec::new(),
        });
    }
    let host = core.host();
    let config = host.config();
    let environments = plan::environments(
        &config.environments,
        &args.environments,
        &config.env_compare.environments,
    )?;
    let views = host.views();
    let chosen = args
        .view
        .map(ViewId)
        .or_else(|| host.active_view())
        .or_else(|| views.first().map(|view| view.id));
    let view = views
        .iter()
        .find(|view| Some(view.id) == chosen)
        .ok_or_else(|| RunError::NotFound("view".to_owned()))?;
    let breakpoint = view
        .cell_breakpoint
        .clone()
        .or_else(|| view.breakpoint.clone())
        .ok_or_else(|| PlanError::Breakpoints("the view shows no breakpoint".to_owned()))?;
    let urls = live_urls(&view.url, &environments, &config.environments);
    let zoom = actions::default_zoom(&core.user_settings());
    host.set_layout(&live_layout(
        &breakpoint,
        environments.len(),
        config.layout.gap,
        zoom,
    ))
    .map_err(engine)?;
    let mut shown = Vec::new();
    for ((view, environment), url) in host.views().iter().zip(&environments).zip(urls) {
        host.navigate(Target::View(view.id), &url).map_err(engine)?;
        shown.push(LiveView {
            view: view.id.0,
            environment: environment.id.clone(),
            url,
        });
    }
    core.ui().set_frame_color(None);
    *lock(&LIVE) = true;
    Ok(LiveAnswer {
        enabled: true,
        views: shown,
    })
}

/// The grid of the current selection again, every view in the window's environment.
fn leave(core: &Core) -> Result<(), RunError> {
    let host = core.host();
    actions::apply(core, &actions::current_selection(host), false).map_err(engine)?;
    *lock(&LIVE) = false;
    if let Some(environment) = host.shell_state().environment {
        actions::select_environment(core, Some(&environment)).map_err(engine)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
