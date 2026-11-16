//! The views proposed for the merged boundaries, in the three modes of `analyzer.mode`
//! (RB-010-13):
//!
//! - `all`: one view at the lower edge of every range - the boundary itself - sorted by width,
//!   plus a mobile view below the smallest and a desktop view above the largest; with
//!   `analyzer.edgeViews` one more view one pixel below each boundary.
//! - `simplified`: the views of `all` in five groups (mobile, large mobile, tablet, laptop,
//!   desktop), one view per group at the group's average width.
//! - `edges`: two views at every boundary, on it and one pixel beside it - below it for
//!   `min-width`, above the last selected width for `max-width`.
//!
//! The height follows the width: portrait 9:16 below 768 px, portrait 3:4 up to 1023 px,
//! landscape 16:10 from 1024 px on.

use serde::Deserialize;

use crate::parser::Side;
use crate::suggest::Cluster;

/// `analyzer.mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// One view per breakpoint.
    #[default]
    All,
    /// Five groups.
    Simplified,
    /// Both sides of every boundary.
    Edges,
}

/// Why a view is proposed; the reason key and its parameters come from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    /// At a boundary that `count` features share.
    Boundary { count: u32 },
    /// Below every boundary.
    Mobile,
    /// Above every boundary.
    Desktop,
    /// One pixel beside the boundary at `boundary`.
    Edge { boundary: u32 },
    /// The average of the views between `from` and `to`.
    Group { from: u32, to: u32 },
    /// Declared in the project (`source`: the file and the name) but used by no media query of
    /// the analysed pages (RB-030-07).
    Declared { source: String },
}

/// One proposed view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub id: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub reason: Reason,
}

/// The groups of `simplified`: id and the widths it takes, `from..to`.
const GROUPS: [(&str, u32, u32); 5] = [
    ("mobile", 0, 480),
    ("mobile-large", 480, 768),
    ("tablet", 768, 1024),
    ("laptop", 1024, 1440),
    ("desktop", 1440, u32::MAX),
];

/// The height for a view `width` px wide.
#[must_use]
pub fn height_for(width: u32) -> u32 {
    let (numerator, denominator) = match width {
        0..768 => (16, 9),
        768..1024 => (4, 3),
        _ => (5, 8),
    };
    let width = u64::from(width);
    // Rounded half up; at most 16/9 of a u32 width, so it fits again after the division.
    u32::try_from((width * numerator * 2 + denominator) / (denominator * 2)).unwrap_or(u32::MAX)
}

/// The views of `clusters` (sorted by width, labelled) in `mode`.
#[must_use]
pub fn views(clusters: &[Cluster], mode: Mode, edge_views: bool) -> Vec<View> {
    let mut views = match mode {
        Mode::All => all(clusters, edge_views),
        Mode::Simplified => simplified(&all(clusters, false)),
        Mode::Edges => edges(clusters),
    };
    views.sort_by_key(|view| view.width);
    views.dedup_by_key(|view| view.width);
    views
}

/// Why the view at a cluster's boundary is proposed.
fn boundary_reason(cluster: &Cluster) -> Reason {
    match (&cluster.source, cluster.count) {
        (Some(source), 0) => Reason::Declared {
            source: source.clone(),
        },
        _ => Reason::Boundary {
            count: cluster.count,
        },
    }
}

fn view(id: String, label: String, width: u32, reason: Reason) -> View {
    View {
        id,
        label,
        width,
        height: height_for(width),
        reason,
    }
}

/// The id and label of a cluster's view: the framework's name, else the width.
fn names(cluster: &Cluster) -> (String, String) {
    match &cluster.label {
        Some(label) => (label.clone(), label.clone()),
        None => (
            format!("w{}", cluster.change),
            format!("{} px", cluster.change),
        ),
    }
}

fn all(clusters: &[Cluster], edge_views: bool) -> Vec<View> {
    let smallest = clusters.first().map_or(u32::MAX, |cluster| cluster.change);
    let largest = clusters.last().map_or(0, |cluster| cluster.change);
    let mobile = [375, 360, 320]
        .into_iter()
        .find(|width| *width < smallest)
        .unwrap_or_else(|| smallest.saturating_sub(1).max(1));
    let desktop = [1440, 1920]
        .into_iter()
        .find(|width| *width > largest)
        .unwrap_or(largest + 320);
    let mut views = vec![
        view("mobile".into(), "mobile".into(), mobile, Reason::Mobile),
        view("desktop".into(), "desktop".into(), desktop, Reason::Desktop),
    ];
    for cluster in clusters {
        let (id, label) = names(cluster);
        if edge_views && cluster.change > 1 {
            views.push(view(
                format!("{id}-1"),
                format!("{label} -1"),
                cluster.change - 1,
                Reason::Edge {
                    boundary: cluster.change,
                },
            ));
        }
        views.push(view(id, label, cluster.change, boundary_reason(cluster)));
    }
    views
}

fn simplified(views: &[View]) -> Vec<View> {
    GROUPS
        .iter()
        .filter_map(|(id, from, to)| {
            let members: Vec<u32> = views
                .iter()
                .map(|view| view.width)
                .filter(|width| (*from..*to).contains(width))
                .collect();
            let (first, last) = (*members.first()?, *members.last()?);
            let sum: u64 = members.iter().copied().map(u64::from).sum();
            let count = members.len() as u64;
            let average = u32::try_from((sum * 2 + count) / (count * 2)).unwrap_or(first);
            Some(view(
                (*id).to_owned(),
                (*id).to_owned(),
                average,
                Reason::Group {
                    from: first,
                    to: last,
                },
            ))
        })
        .collect()
}

fn edges(clusters: &[Cluster]) -> Vec<View> {
    let mut views = Vec::new();
    for cluster in clusters.iter().filter(|cluster| cluster.change > 1) {
        let (id, label) = names(cluster);
        let boundary = cluster.change;
        let (on, beside, suffix) = match cluster.side {
            Side::Min => (boundary, boundary - 1, "-1"),
            Side::Max => (boundary - 1, boundary, "+1"),
        };
        let edge = Reason::Edge { boundary };
        views.push(view(
            format!("{id}{suffix}"),
            format!("{label} {suffix}"),
            beside,
            edge,
        ));
        views.push(view(id, label, on, boundary_reason(cluster)));
    }
    views
}

#[cfg(test)]
#[path = "modes_tests.rs"]
mod tests;
