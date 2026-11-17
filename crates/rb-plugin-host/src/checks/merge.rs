//! Many reports, one list: the same rule (or shared id) at the same element of the same page is
//! one finding, whichever views and plugins reported it.

use std::collections::{BTreeMap, HashMap};

use rb_plugin_types::{
    ElementRef, Finding, FindingMessage, MergedFinding, Severity, ViewId, fingerprint,
    normalize_path,
};

/// One finding as one plugin reported it in one view, after the rule overrides.
#[derive(Clone, Debug)]
pub struct Report {
    pub plugin: String,
    /// `<plugin>.<rule>`.
    pub rule_id: String,
    pub shared_id: Option<String>,
    pub help_url: Option<String>,
    /// After `checks.rules`.
    pub severity: Severity,
    pub view: Option<ViewId>,
    pub breakpoint: Option<String>,
    /// As reported; its `url` filled with the run's when it had none.
    pub finding: Finding,
}

impl Report {
    /// What the id is made of: the shared id when there is one, else the rule id.
    fn key(&self) -> String {
        self.shared_id
            .as_ref()
            .map_or_else(|| self.rule_id.clone(), |shared| format!("shared:{shared}"))
    }

    fn id(&self) -> String {
        let element = self
            .finding
            .path
            .as_deref()
            .or(self.finding.selector.as_deref());
        fingerprint(&self.key(), self.finding.url.as_deref(), element)
    }
}

fn start(id: String, report: Report) -> MergedFinding {
    let Report {
        plugin,
        rule_id,
        shared_id,
        help_url,
        severity,
        finding,
        ..
    } = report;
    let element = (finding.selector.is_some() || finding.path.is_some()).then(|| ElementRef {
        selector: finding.selector,
        path: finding.path.as_deref().map(normalize_path),
    });
    MergedFinding {
        id,
        rule_id,
        plugins: vec![plugin],
        severity,
        message: FindingMessage {
            key: finding.message,
            params: finding.params.into_iter().collect(),
        },
        views: Vec::new(),
        breakpoints: Vec::new(),
        url: finding.url,
        element,
        help_url,
        data: finding
            .data
            .and_then(|data| serde_json::from_str(&data).ok()),
        shared_id,
    }
}

/// Merges `reports`, given in a fixed order (plugin, view, position): the first report of a
/// finding keeps its message, rule and details; later ones add their view, their plugin and
/// raise the severity. The list is ordered by severity (highest first), rule id and id.
#[must_use]
pub fn merge(reports: Vec<Report>) -> Vec<MergedFinding> {
    let mut merged: Vec<MergedFinding> = Vec::new();
    let mut seen_in: Vec<BTreeMap<ViewId, Option<String>>> = Vec::new();
    let mut by_id: HashMap<String, usize> = HashMap::new();
    for report in reports {
        let id = report.id();
        let view = report.view.map(|view| (view, report.breakpoint.clone()));
        let at = match by_id.get(&id) {
            Some(&at) => {
                let finding = &mut merged[at];
                finding.severity = finding.severity.max(report.severity);
                if !finding.plugins.contains(&report.plugin) {
                    finding.plugins.push(report.plugin);
                }
                at
            }
            None => {
                by_id.insert(id.clone(), merged.len());
                merged.push(start(id, report));
                seen_in.push(BTreeMap::new());
                merged.len() - 1
            }
        };
        if let Some((view, breakpoint)) = view {
            seen_in[at].entry(view).or_insert(breakpoint);
        }
    }
    for (finding, views) in merged.iter_mut().zip(seen_in) {
        for (view, breakpoint) in views {
            finding.views.push(view);
            if let Some(breakpoint) = breakpoint
                && !finding.breakpoints.contains(&breakpoint)
            {
                finding.breakpoints.push(breakpoint);
            }
        }
    }
    merged.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.rule_id.cmp(&b.rule_id))
            .then_with(|| a.id.cmp(&b.id))
    });
    merged
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
