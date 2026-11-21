//! The analysis `containers_analyze` answers: each view's report (what `assets/analyze.js` finds,
//! `web/src/collect.ts`) bounded, then merged over the views.
//!
//! - `conditions`: every `@container` condition of the views once (`name` and `query`), with its
//!   `bounds` (`axis`, `op`, `value` as written, `px`) and `supported`;
//! - `containers`: every size container once (by its `path`), its `conditions` (ids into the list
//!   above), per view (ordered by the view's width) its `inline` and `block` `size` with the next
//!   threshold `below` and `above` it, the conditions `matching` there and whether rBrowser
//!   `resized` it, and the `switches` - between two neighbouring views, the conditions `gained`
//!   and `lost`, so the breakpoints at which the container changes;
//! - `views`: each view with its count of containers, its unreadable stylesheets, whether a
//!   limit cut its report, or the code of its `error`.
//!
//! A report comes from the page, so every list and text is bounded here before it reaches an
//! answer.

use rb_plugin_api::{ViewId, ViewInfo};
use serde::{Deserialize, Serialize};

/// The most containers and conditions a report and the analysis keep.
pub const MAX_CONTAINERS: usize = 300;
pub const MAX_CONDITIONS: usize = 300;
/// The longest text a report keeps, in characters.
pub const MAX_TEXT: usize = 300;
const MAX_NAMES: usize = 8;
const MAX_BOUNDS: usize = 16;
const MAX_THRESHOLDS: usize = 64;

/// One threshold of a condition.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bound {
    /// `inline` or `block`.
    pub axis: String,
    /// `<`, `<=`, `>`, `>=` or `=`.
    pub op: String,
    /// The value as written (`50rem`).
    pub value: String,
    /// In px; none for a unit the page script does not know.
    pub px: Option<f64>,
}

/// One `@container` condition.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Condition {
    pub name: String,
    pub query: String,
    pub bounds: Vec<Bound>,
    pub supported: bool,
}

/// The px of a container's bounds, per axis.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Thresholds {
    pub inline: Vec<f64>,
    pub block: Vec<f64>,
}

/// One container of a view's report.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Container {
    pub path: String,
    pub selector: String,
    pub names: Vec<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub inline: f64,
    pub block: f64,
    /// Indices into the report's conditions.
    pub conditions: Vec<usize>,
    pub matching: Vec<usize>,
    pub thresholds: Thresholds,
    pub resized: bool,
}

/// One view's report.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ViewReport {
    pub containers: Vec<Container>,
    pub conditions: Vec<Condition>,
    pub unreadable_sheets: u32,
    pub truncated: bool,
}

fn cut(text: &mut String) {
    if text.chars().count() > MAX_TEXT {
        *text = text.chars().take(MAX_TEXT).collect();
    }
}

fn size(value: f64) -> f64 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
}

impl ViewReport {
    /// The report the analysis answered (JSON text), bounded; none for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        serde_json::from_str::<Self>(text.trim())
            .ok()
            .map(Self::bounded)
    }

    fn bounded(mut self) -> Self {
        if self.containers.len() > MAX_CONTAINERS || self.conditions.len() > MAX_CONDITIONS {
            self.truncated = true;
        }
        self.containers.truncate(MAX_CONTAINERS);
        self.conditions.truncate(MAX_CONDITIONS);
        for condition in &mut self.conditions {
            cut(&mut condition.name);
            cut(&mut condition.query);
            condition.bounds.truncate(MAX_BOUNDS);
            for bound in &mut condition.bounds {
                cut(&mut bound.axis);
                cut(&mut bound.op);
                cut(&mut bound.value);
                bound.px = bound.px.filter(|px| px.is_finite());
            }
        }
        let count = self.conditions.len();
        for container in &mut self.containers {
            cut(&mut container.path);
            cut(&mut container.selector);
            cut(&mut container.kind);
            container.names.truncate(MAX_NAMES);
            container.names.iter_mut().for_each(cut);
            container.inline = size(container.inline);
            container.block = size(container.block);
            container.conditions.retain(|index| *index < count);
            container.conditions.truncate(MAX_CONDITIONS);
            let queried = container.conditions.clone();
            container.matching.retain(|index| queried.contains(index));
            for list in [
                &mut container.thresholds.inline,
                &mut container.thresholds.block,
            ] {
                list.retain(|px| px.is_finite());
                list.truncate(MAX_THRESHOLDS);
            }
        }
        self
    }
}

/// A container's size on one axis, with the nearest thresholds around it.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct AxisSize {
    pub size: f64,
    /// The largest threshold at or under the size: shrinking past it switches a condition.
    pub below: Option<f64>,
    /// The smallest threshold over the size: growing to it switches a condition.
    pub above: Option<f64>,
}

/// A container in one view.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ContainerView {
    pub view: ViewId,
    pub label: String,
    pub breakpoint: String,
    pub inline: AxisSize,
    pub block: AxisSize,
    /// Ids of the analysis' conditions.
    pub matching: Vec<usize>,
    pub resized: bool,
}

/// Where a container changes between two neighbouring views.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Switch {
    pub from: ViewId,
    pub to: ViewId,
    pub gained: Vec<usize>,
    pub lost: Vec<usize>,
}

/// A container over the views.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct MergedContainer {
    pub path: String,
    pub selector: String,
    pub names: Vec<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub conditions: Vec<usize>,
    pub views: Vec<ContainerView>,
    pub switches: Vec<Switch>,
}

/// A view of the analysis.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewSummary {
    pub view: ViewId,
    pub label: String,
    pub breakpoint: String,
    pub width: u32,
    pub containers: usize,
    pub unreadable_sheets: u32,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The analysis of every view.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Analysis {
    pub conditions: Vec<Condition>,
    pub containers: Vec<MergedContainer>,
    pub views: Vec<ViewSummary>,
}

/// The nearest thresholds at or under `size` and over it.
#[must_use]
pub fn next_bounds(size: f64, thresholds: &[f64]) -> (Option<f64>, Option<f64>) {
    let below = thresholds
        .iter()
        .copied()
        .filter(|px| *px <= size)
        .reduce(f64::max);
    let above = thresholds
        .iter()
        .copied()
        .filter(|px| *px > size)
        .reduce(f64::min);
    (below, above)
}

fn axis(size: f64, thresholds: &[f64]) -> AxisSize {
    let (below, above) = next_bounds(size, thresholds);
    AxisSize { size, below, above }
}

/// The id of `condition` in `conditions`, added when new; none past the limit.
fn condition_id(conditions: &mut Vec<Condition>, condition: &Condition) -> Option<usize> {
    if let Some(id) = conditions
        .iter()
        .position(|known| known.name == condition.name && known.query == condition.query)
    {
        return Some(id);
    }
    if conditions.len() >= MAX_CONDITIONS {
        return None;
    }
    conditions.push(condition.clone());
    Some(conditions.len() - 1)
}

fn switches(views: &[ContainerView]) -> Vec<Switch> {
    views
        .windows(2)
        .filter_map(|pair| {
            let [from, to] = pair else {
                return None;
            };
            let gained: Vec<usize> = to
                .matching
                .iter()
                .filter(|id| !from.matching.contains(id))
                .copied()
                .collect();
            let lost: Vec<usize> = from
                .matching
                .iter()
                .filter(|id| !to.matching.contains(id))
                .copied()
                .collect();
            if gained.is_empty() && lost.is_empty() {
                return None;
            }
            Some(Switch {
                from: from.view,
                to: to.view,
                gained,
                lost,
            })
        })
        .collect()
}

/// Merges the views' reports, or the code each failed with, ordered by the views' width.
#[must_use]
pub fn merge(mut views: Vec<(ViewInfo, Result<ViewReport, String>)>) -> Analysis {
    views.sort_by_key(|(info, _)| (info.width, info.id));
    let mut analysis = Analysis::default();
    for (info, result) in views {
        let mut summary = ViewSummary {
            view: info.id,
            label: info.label.clone(),
            breakpoint: info.breakpoint.clone(),
            width: info.width,
            ..ViewSummary::default()
        };
        let report = match result {
            Ok(report) => report,
            Err(code) => {
                summary.error = Some(code);
                analysis.views.push(summary);
                continue;
            }
        };
        let ids: Vec<Option<usize>> = report
            .conditions
            .iter()
            .map(|condition| condition_id(&mut analysis.conditions, condition))
            .collect();
        let merged_ids = |locals: &[usize]| -> Vec<usize> {
            let mut merged: Vec<usize> = locals
                .iter()
                .filter_map(|local| ids.get(*local).copied().flatten())
                .collect();
            merged.sort_unstable();
            merged.dedup();
            merged
        };
        summary.containers = report.containers.len();
        summary.unreadable_sheets = report.unreadable_sheets;
        summary.truncated = report.truncated;
        for container in &report.containers {
            let index = match analysis
                .containers
                .iter()
                .position(|known| known.path == container.path)
            {
                Some(index) => index,
                None if analysis.containers.len() < MAX_CONTAINERS => {
                    analysis.containers.push(MergedContainer {
                        path: container.path.clone(),
                        selector: container.selector.clone(),
                        names: container.names.clone(),
                        kind: container.kind.clone(),
                        ..MergedContainer::default()
                    });
                    analysis.containers.len() - 1
                }
                None => {
                    summary.truncated = true;
                    continue;
                }
            };
            let Some(merged) = analysis.containers.get_mut(index) else {
                continue;
            };
            for id in merged_ids(&container.conditions) {
                if !merged.conditions.contains(&id) {
                    merged.conditions.push(id);
                }
            }
            merged.views.push(ContainerView {
                view: info.id,
                label: info.label.clone(),
                breakpoint: info.breakpoint.clone(),
                inline: axis(container.inline, &container.thresholds.inline),
                block: axis(container.block, &container.thresholds.block),
                matching: merged_ids(&container.matching),
                resized: container.resized,
            });
        }
        analysis.views.push(summary);
    }
    for container in &mut analysis.containers {
        container.conditions.sort_unstable();
        container.switches = switches(&container.views);
    }
    analysis
}

#[cfg(test)]
#[path = "analysis_tests.rs"]
pub(crate) mod tests;
