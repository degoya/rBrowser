use super::{Mode, Reason, height_for, views};
use crate::parser::{Boundary, Side};
use crate::suggest::{Cluster, Framework, label, merge};

fn tailwind() -> Vec<Cluster> {
    let boundaries: Vec<Boundary> = [640, 768, 1024, 1280, 1536]
        .into_iter()
        .map(|change| Boundary {
            change,
            side: Side::Min,
        })
        .collect();
    let mut clusters = merge(&boundaries, 8);
    label(&mut clusters, Some(Framework::Tailwind));
    clusters
}

fn summary(mode: Mode, edge_views: bool, clusters: &[Cluster]) -> Vec<(String, u32, u32)> {
    views(clusters, mode, edge_views)
        .into_iter()
        .map(|view| (view.id, view.width, view.height))
        .collect()
}

fn owned(expected: &[(&str, u32, u32)]) -> Vec<(String, u32, u32)> {
    expected
        .iter()
        .map(|(id, width, height)| ((*id).to_owned(), *width, *height))
        .collect()
}

#[test]
fn heights_follow_the_width() {
    assert_eq!(height_for(375), 667);
    assert_eq!(height_for(360), 640);
    assert_eq!(height_for(768), 1024);
    assert_eq!(height_for(1024), 640);
    assert_eq!(height_for(1440), 900);
    assert_eq!(height_for(1920), 1200);
}

#[test]
fn all_has_a_view_per_boundary_plus_mobile_and_desktop() {
    assert_eq!(
        summary(Mode::All, false, &tailwind()),
        owned(&[
            ("mobile", 375, 667),
            ("sm", 640, 1138),
            ("md", 768, 1024),
            ("lg", 1024, 640),
            ("xl", 1280, 800),
            ("2xl", 1536, 960),
            ("desktop", 1920, 1200),
        ])
    );
}

#[test]
fn edge_views_add_one_pixel_below_each_boundary() {
    let ids: Vec<String> = summary(Mode::All, true, &tailwind())
        .into_iter()
        .map(|(id, width, _)| format!("{id}@{width}"))
        .collect();
    assert_eq!(
        ids,
        vec![
            "mobile@375",
            "sm-1@639",
            "sm@640",
            "md-1@767",
            "md@768",
            "lg-1@1023",
            "lg@1024",
            "xl-1@1279",
            "xl@1280",
            "2xl-1@1535",
            "2xl@1536",
            "desktop@1920"
        ]
    );
}

#[test]
fn simplified_averages_five_groups() {
    assert_eq!(
        summary(Mode::Simplified, false, &tailwind()),
        owned(&[
            ("mobile", 375, 667),
            ("mobile-large", 640, 1138),
            ("tablet", 768, 1024),
            ("laptop", 1152, 720),
            ("desktop", 1728, 1080),
        ])
    );
    let grouped = views(&tailwind(), Mode::Simplified, false);
    assert_eq!(
        grouped[3].reason,
        Reason::Group {
            from: 1024,
            to: 1280
        }
    );
}

#[test]
fn edges_take_the_side_of_the_query() {
    let mut clusters = merge(
        &[
            Boundary {
                change: 600,
                side: Side::Min,
            },
            Boundary {
                change: 900,
                side: Side::Max,
            },
        ],
        8,
    );
    label(&mut clusters, None);
    assert_eq!(
        summary(Mode::Edges, false, &clusters),
        owned(&[
            ("w600-1", 599, 1065),
            ("w600", 600, 1067),
            ("w900", 899, 1199),
            ("w900+1", 900, 1200),
        ])
    );
}

#[test]
fn no_boundary_still_gives_mobile_and_desktop() {
    assert_eq!(
        summary(Mode::All, true, &[]),
        owned(&[("mobile", 375, 667), ("desktop", 1440, 900)])
    );
    assert!(views(&[], Mode::Edges, false).is_empty());
}

#[test]
fn a_tiny_first_boundary_moves_the_mobile_view_below_it() {
    let clusters = merge(
        &[Boundary {
            change: 300,
            side: Side::Min,
        }],
        8,
    );
    let all = views(&clusters, Mode::All, false);
    assert_eq!(all[0].id, "mobile");
    assert_eq!(all[0].width, 299);
}
