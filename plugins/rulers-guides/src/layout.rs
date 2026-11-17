//! What one view draws: the effective state resolved against its page's width and breakpoint.

use rb_plugin_api::{PluginHost, ViewId};
use serde::{Deserialize, Serialize};

use crate::colors::Colors;
use crate::columns::{self, ColumnLayout};
use crate::guides::{self, Placed};
use crate::model::{Baseline, Invalid, MAX_LENGTH, State, Visible};

/// The page's widths in CSS px, as its page script measures them.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    /// `document.documentElement.clientWidth`: the page without its scroll bar.
    pub client_width: f64,
    /// `window.innerWidth`: the viewport the media queries see.
    pub inner_width: f64,
}

impl Geometry {
    /// # Errors
    ///
    /// A width that is no finite length.
    pub fn check(&self) -> Result<(), Invalid> {
        for width in [self.client_width, self.inner_width] {
            if !(width.is_finite() && (0.0..=MAX_LENGTH).contains(&width)) {
                return Err("geometry");
            }
        }
        Ok(())
    }
}

/// One view's drawing.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Layout {
    pub breakpoint: Option<String>,
    pub visible: Visible,
    pub guides: Vec<Placed>,
    pub columns: Option<ColumnLayout>,
    pub baseline: Baseline,
    /// The colours to draw in (RB-090-58).
    pub colors: Colors,
}

/// The breakpoint of view `view`, from `views.list`; none when the views are not served or the
/// view is unknown.
pub fn breakpoint_of(host: &mut dyn PluginHost, view: ViewId) -> Option<String> {
    host.views()
        .ok()?
        .into_iter()
        .find(|info| info.id == view)
        .map(|info| info.breakpoint)
        .filter(|breakpoint| !breakpoint.is_empty())
}

/// The breakpoints of the open views, each once, for the panel.
pub fn breakpoints(host: &mut dyn PluginHost) -> Vec<String> {
    let mut found: Vec<String> = host
        .views()
        .unwrap_or_default()
        .into_iter()
        .map(|info| info.breakpoint)
        .filter(|breakpoint| crate::model::is_breakpoint_key(breakpoint))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// `state` drawn in a page of `geometry` showing `breakpoint`.
#[must_use]
pub fn layout(state: &State, breakpoint: Option<String>, geometry: Geometry) -> Layout {
    let columns = columns::spec_for(&state.columns, breakpoint.as_deref()).map(|spec| {
        columns::layout(
            &columns::resolve(spec, geometry.inner_width),
            geometry.client_width,
        )
    });
    Layout {
        visible: state.visible,
        guides: guides::place(&state.guides, geometry.client_width),
        columns,
        baseline: state.baseline,
        colors: state.colors.clone(),
        breakpoint,
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
