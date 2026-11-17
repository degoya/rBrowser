use super::{Cluster, Framework, adopt, label, merge, recognise};
use crate::parser::{Boundary, Side};
use crate::static_source::Declared;

fn min(change: u32) -> Boundary {
    Boundary {
        change,
        side: Side::Min,
    }
}

fn max(change: u32) -> Boundary {
    Boundary {
        change,
        side: Side::Max,
    }
}

fn changes(clusters: &[Cluster]) -> Vec<u32> {
    clusters.iter().map(|cluster| cluster.change).collect()
}

#[test]
fn values_within_the_tolerance_become_one_boundary() {
    let merged = merge(&[min(767), min(768), min(770)], 8);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].count, 3);
    // Every value once: the smallest wins the tie.
    assert_eq!(merged[0].change, 767);
}

#[test]
fn the_most_frequent_value_wins() {
    let merged = merge(&[min(767), min(768), min(768), max(770)], 8);
    assert_eq!(merged[0].change, 768);
    assert_eq!(merged[0].side, Side::Min);
    assert_eq!(merged[0].count, 4);
}

#[test]
fn values_further_apart_stay_apart() {
    assert_eq!(
        changes(&merge(&[min(640), min(649), min(768)], 8)),
        vec![640, 649, 768]
    );
    assert_eq!(
        changes(&merge(&[min(640), min(649), min(768)], 0)),
        vec![640, 649, 768]
    );
    assert_eq!(changes(&merge(&[min(640), min(648)], 8)), vec![640]);
}

#[test]
fn the_majority_decides_the_side() {
    let merged = merge(&[max(576), max(576), min(576)], 8);
    assert_eq!(merged[0].side, Side::Max);
    assert_eq!(merged[0].query_width(), 575);
    let tie = merge(&[max(576), min(576)], 8);
    assert_eq!(tie[0].side, Side::Min);
    assert_eq!(tie[0].query_width(), 576);
}

#[test]
fn frameworks_are_recognised_by_their_widths() {
    let tailwind = merge(&[min(640), min(768), min(1024), min(1280), min(1536)], 8);
    assert_eq!(recognise(&tailwind), Some(Framework::Tailwind));
    let bootstrap = merge(&[min(576), max(576), min(768), min(992), min(1200)], 8);
    assert_eq!(recognise(&bootstrap), Some(Framework::Bootstrap));
    let custom = merge(&[min(600), min(768), min(900)], 8);
    assert_eq!(recognise(&custom), None);
}

#[test]
fn labels_follow_the_framework() {
    let mut clusters = merge(&[min(640), min(768), min(1024), min(900)], 8);
    label(&mut clusters, Some(Framework::Tailwind));
    let labels: Vec<Option<&str>> = clusters
        .iter()
        .map(|cluster| cluster.label.as_deref())
        .collect();
    assert_eq!(labels, vec![Some("sm"), Some("md"), None, Some("lg")]);
    let mut unknown = merge(&[min(640)], 8);
    label(&mut unknown, None);
    assert_eq!(unknown[0].label, None);
}

fn declared(width: u32, label: &str, path: &str, heuristic: bool) -> Declared {
    Declared {
        width,
        label: label.to_owned(),
        path: path.to_owned(),
        framework: (!heuristic).then_some(Framework::Tailwind),
        heuristic,
    }
}

fn summary(clusters: &[Cluster]) -> Vec<(u32, u32, Option<&str>, Option<&str>)> {
    clusters
        .iter()
        .map(|cluster| {
            (
                cluster.change,
                cluster.count,
                cluster.label.as_deref(),
                cluster.source.as_deref(),
            )
        })
        .collect()
}

#[test]
fn static_declared_widths_name_the_clusters_within_the_tolerance() {
    let mut clusters = merge(&[min(640), min(770), min(1024)], 8);
    let config = "tailwind.config.js";
    adopt(
        &mut clusters,
        &[
            declared(640, "sm", config, false),
            declared(768, "tablet", config, false),
            declared(1920, "3xl", config, false),
        ],
        8,
    );
    assert_eq!(
        summary(&clusters),
        vec![
            (640, 1, Some("sm"), Some("tailwind.config.js: sm")),
            (770, 1, Some("tablet"), Some("tailwind.config.js: tablet")),
            (1024, 1, None, None),
            (1920, 0, Some("3xl"), Some("tailwind.config.js: 3xl")),
        ]
    );
    assert_eq!(clusters[3].side, Side::Min);
}

#[test]
fn static_the_project_s_name_wins_over_the_recognised_framework_s() {
    let mut clusters = merge(&[min(640), min(768), min(1024), min(1280)], 8);
    label(&mut clusters, Some(Framework::Tailwind));
    // The project moved `md` to 900: 768 loses the name, 900 is new.
    adopt(&mut clusters, &[declared(900, "md", "app.css", false)], 8);
    assert_eq!(
        summary(&clusters),
        vec![
            (640, 1, Some("sm"), None),
            (768, 1, None, None),
            (900, 0, Some("md"), Some("app.css: md")),
            (1024, 1, Some("lg"), None),
            (1280, 1, Some("xl"), None),
        ]
    );
}

#[test]
fn static_a_heuristic_name_never_replaces_another() {
    let mut clusters = merge(&[min(768), min(1024)], 8);
    label(&mut clusters, Some(Framework::Bootstrap));
    adopt(
        &mut clusters,
        &[
            declared(768, "tablet", "_vars.scss", true),
            declared(1024, "md", "_vars.scss", true),
            declared(1500, "wide", "_vars.scss", true),
        ],
        8,
    );
    assert_eq!(
        summary(&clusters),
        vec![
            (768, 1, Some("md"), Some("_vars.scss: tablet")),
            (1024, 1, None, Some("_vars.scss: md")),
            (1500, 0, Some("wide"), Some("_vars.scss: wide")),
        ]
    );
    assert!(clusters.iter().all(|cluster| cluster.heuristic));
}

#[test]
fn static_unusable_and_taken_names_stay_off() {
    let mut clusters = Vec::new();
    adopt(
        &mut clusters,
        &[
            declared(500, "mobile", "a.js", false),
            declared(600, "with space", "a.js", false),
            declared(700, "md", "a.js", false),
            declared(800, "md", "b.js", false),
        ],
        8,
    );
    let labels: Vec<Option<&str>> = clusters
        .iter()
        .map(|cluster| cluster.label.as_deref())
        .collect();
    assert_eq!(labels, vec![None, None, Some("md"), None]);
    assert!(clusters.iter().all(|cluster| cluster.count == 0));
}
