//! The sweep's plan, apart from any view: the coarse pass from the first width to the last in
//! steps, the refinement in 1-px steps only between two swept widths whose issues differ (the
//! boundaries of a break, never the whole range), and the issues' width ranges from what each
//! swept width showed.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// One problem the page shows at a width: its kind (`overflow`, `text-clipped`, `overlap`), the
/// element's short selector and full path, and for an overlap the other element's selector.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Issue {
    pub kind: String,
    pub selector: String,
    #[serde(default)]
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other: Option<String>,
}

/// What each swept width showed.
pub type Samples = BTreeMap<u32, BTreeSet<Issue>>;

/// The widths of a sweep, in CSS px: `from` to `to` (inclusive) in steps of `step`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub from: u32,
    pub to: u32,
    pub step: u32,
}

/// One issue over a run of widths: present at every swept width from `from` to `to`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Range {
    pub issue: Issue,
    pub from: u32,
    pub to: u32,
}

/// The coarse pass: `from`, every `step` after it, and `to` when the steps miss it.
#[must_use]
pub fn coarse(span: Span) -> Vec<u32> {
    let step = span.step.max(1);
    let mut widths: Vec<u32> = (span.from..=span.to).step_by(step as usize).collect();
    if widths.last() != Some(&span.to) && span.to >= span.from {
        widths.push(span.to);
    }
    widths
}

/// The widths the refinement probes: every pixel between two neighbouring swept widths whose
/// issues differ, in order, at most `limit` of them.
#[must_use]
pub fn refine(samples: &Samples, limit: usize) -> Vec<u32> {
    let swept: Vec<(&u32, &BTreeSet<Issue>)> = samples.iter().collect();
    swept
        .windows(2)
        .filter(|pair| pair[0].1 != pair[1].1)
        .flat_map(|pair| (*pair[0].0 + 1)..*pair[1].0)
        .take(limit)
        .collect()
}

/// Every issue's runs of neighbouring swept widths where it is present.
#[must_use]
pub fn ranges(samples: &Samples) -> Vec<Range> {
    let mut open: BTreeMap<&Issue, (u32, u32)> = BTreeMap::new();
    let mut found = Vec::new();
    for (&width, issues) in samples {
        // A run ends at the first swept width without its issue.
        let ended: Vec<&Issue> = open
            .keys()
            .filter(|issue| !issues.contains(**issue))
            .copied()
            .collect();
        for issue in ended {
            if let Some((from, to)) = open.remove(issue) {
                found.push(range(issue, from, to));
            }
        }
        for issue in issues {
            open.entry(issue)
                .and_modify(|run| run.1 = width)
                .or_insert((width, width));
        }
    }
    found.extend(
        open.into_iter()
            .map(|(issue, (from, to))| range(issue, from, to)),
    );
    found
}

fn range(issue: &Issue, from: u32, to: u32) -> Range {
    Range {
        issue: issue.clone(),
        from,
        to,
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
