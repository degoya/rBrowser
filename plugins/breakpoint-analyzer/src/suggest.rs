//! From boundaries to breakpoints (RB-010-13): equal and near boundaries merged within the
//! tolerance and weighted by how often they occur, the framework they come from recognised for
//! the labels, and the breakpoints the project declares adopted (RB-030-07, [`adopt`]).

use std::collections::BTreeMap;

use crate::parser::{Boundary, Side};
use crate::static_source::Declared;

/// One boundary after merging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cluster {
    /// The first width at which the layout differs: the most frequent value of the cluster, the
    /// smaller one on a tie.
    pub change: u32,
    /// The side most of its queries select (`min-width` on a tie).
    pub side: Side,
    /// How many media query features it merges.
    pub count: u32,
    /// The framework's name for it (`md`), or the project's own.
    pub label: Option<String>,
    /// The project file that declares it, with the name there (`tailwind.config.js: md`).
    pub source: Option<String>,
    /// Declared only by a variable's name (`$bp-md`): the weaker, heuristic source.
    pub heuristic: bool,
}

impl Cluster {
    /// The width a person reads in the query: the `min-width` itself, or the last width a
    /// `max-width` query still selects.
    #[must_use]
    pub fn query_width(&self) -> u32 {
        match self.side {
            Side::Min => self.change,
            Side::Max => self.change - 1,
        }
    }
}

/// A CSS framework whose default breakpoints are recognised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Framework {
    /// Tailwind CSS v3 and v4 (`sm` 640 to `2xl` 1536; v4 writes them in `rem`).
    Tailwind,
    /// Bootstrap 4 and 5 (`sm` 576 to `xxl` 1400).
    Bootstrap,
}

impl Framework {
    /// The framework's breakpoints, narrowest first.
    #[must_use]
    pub fn breakpoints(self) -> &'static [(u32, &'static str)] {
        match self {
            Self::Tailwind => &[
                (640, "sm"),
                (768, "md"),
                (1024, "lg"),
                (1280, "xl"),
                (1536, "2xl"),
            ],
            Self::Bootstrap => &[
                (576, "sm"),
                (768, "md"),
                (992, "lg"),
                (1200, "xl"),
                (1400, "xxl"),
            ],
        }
    }

    /// The id in answers.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Tailwind => "tailwind",
            Self::Bootstrap => "bootstrap",
        }
    }
}

/// How many of a framework's breakpoints must be present before its labels are used.
const RECOGNISED_AT: usize = 3;

/// Merges `boundaries`: sorted by width, a value within `tolerance` px of its cluster's first
/// value joins it.
#[must_use]
pub fn merge(boundaries: &[Boundary], tolerance: u32) -> Vec<Cluster> {
    // Per width: how many `min` and how many `max` features.
    let mut counts: BTreeMap<u32, (u32, u32)> = BTreeMap::new();
    for boundary in boundaries {
        let entry = counts.entry(boundary.change).or_default();
        match boundary.side {
            Side::Min => entry.0 += 1,
            Side::Max => entry.1 += 1,
        }
    }
    let mut groups: Vec<Vec<(u32, (u32, u32))>> = Vec::new();
    for (change, sides) in counts {
        match groups.last_mut() {
            Some(group) if change - group[0].0 <= tolerance => group.push((change, sides)),
            _ => groups.push(vec![(change, sides)]),
        }
    }
    groups.into_iter().map(|group| cluster_of(&group)).collect()
}

fn cluster_of(group: &[(u32, (u32, u32))]) -> Cluster {
    let mut best = group[0];
    for &entry in &group[1..] {
        if entry.1.0 + entry.1.1 > best.1.0 + best.1.1 {
            best = entry;
        }
    }
    let (min, max) = group.iter().fold((0, 0), |(min, max), (_, sides)| {
        (min + sides.0, max + sides.1)
    });
    Cluster {
        change: best.0,
        side: if max > min { Side::Max } else { Side::Min },
        count: min + max,
        label: None,
        source: None,
        heuristic: false,
    }
}

/// The framework most of `clusters` belong to, if at least [`RECOGNISED_AT`] of its breakpoints
/// are there and no other framework has as many.
#[must_use]
pub fn recognise(clusters: &[Cluster]) -> Option<Framework> {
    let hits = |framework: Framework| {
        framework
            .breakpoints()
            .iter()
            .filter(|(width, _)| clusters.iter().any(|cluster| cluster.change == *width))
            .count()
    };
    let tailwind = hits(Framework::Tailwind);
    let bootstrap = hits(Framework::Bootstrap);
    if tailwind >= RECOGNISED_AT && tailwind > bootstrap {
        Some(Framework::Tailwind)
    } else if bootstrap >= RECOGNISED_AT && bootstrap > tailwind {
        Some(Framework::Bootstrap)
    } else {
        None
    }
}

/// Gives every cluster at one of `framework`'s widths its name.
pub fn label(clusters: &mut [Cluster], framework: Option<Framework>) {
    let Some(framework) = framework else {
        return;
    };
    for cluster in clusters {
        cluster.label = framework
            .breakpoints()
            .iter()
            .find(|(width, _)| *width == cluster.change)
            .map(|(_, name)| (*name).to_owned());
    }
}

/// The ids the views of mode `all` take themselves.
const RESERVED_LABELS: [&str; 2] = ["mobile", "desktop"];

/// Whether a declared name can be a breakpoint id: short, `[A-Za-z0-9_-]`, not reserved.
fn usable_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 32
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !RESERVED_LABELS.contains(&label)
}

/// Adopts the breakpoints the project declares (RB-030-07) into `clusters`: a declared width
/// within `tolerance` px of a cluster's gives that cluster its source and name; one beside every
/// cluster becomes a cluster of its own (no media query uses it yet: `count` 0, `min-width`).
///
/// A framework's declaration names the cluster over the recognised framework's name (which
/// another cluster then loses); a heuristic one names only a cluster without a name. A name that
/// another declaration took, or that is no usable id, stays off. Sorted by width again.
pub fn adopt(clusters: &mut Vec<Cluster>, declared: &[Declared], tolerance: u32) {
    for heuristic in [false, true] {
        for item in declared.iter().filter(|item| item.heuristic == heuristic) {
            let nearest = clusters
                .iter()
                .enumerate()
                .filter(|(_, cluster)| cluster.change.abs_diff(item.width) <= tolerance)
                .min_by_key(|(_, cluster)| cluster.change.abs_diff(item.width))
                .map(|(index, _)| index);
            let index = nearest.unwrap_or_else(|| {
                clusters.push(Cluster {
                    change: item.width,
                    side: Side::Min,
                    count: 0,
                    label: None,
                    source: None,
                    heuristic,
                });
                clusters.len() - 1
            });
            if clusters[index].source.is_none() {
                clusters[index].source = Some(format!("{}: {}", item.path, item.label));
                clusters[index].heuristic = heuristic;
            }
            if !usable_label(&item.label) || (heuristic && clusters[index].label.is_some()) {
                continue;
            }
            let label = Some(item.label.clone());
            let taken = |cluster: &Cluster| cluster.label == label;
            let declared_elsewhere = clusters.iter().enumerate().any(|(other, cluster)| {
                other != index && taken(cluster) && (heuristic || cluster.source.is_some())
            });
            if declared_elsewhere {
                continue;
            }
            for (other, cluster) in clusters.iter_mut().enumerate() {
                if other != index && cluster.label == label {
                    cluster.label = None;
                }
            }
            clusters[index].label = label;
        }
    }
    clusters.sort_by_key(|cluster| cluster.change);
}

#[cfg(test)]
#[path = "suggest_tests.rs"]
mod tests;
