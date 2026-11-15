use rb_visual::sitemap::RunBreakpoint;
use rb_visual::{DiffOptions, Rgba, encode_png};

use super::*;

fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    encode_png(&Rgba::new(width, height, color).expect("image")).expect("png")
}

/// `width` x 10 white, with the left `red` columns red.
fn striped(width: u32, red: u32) -> Vec<u8> {
    let mut image = Rgba::new(width, 10, [255, 255, 255, 255]).expect("image");
    image.fill(0, 0, red, 10, [255, 0, 0, 255]);
    encode_png(&image).expect("png")
}

fn options() -> DiffOptions {
    DiffOptions {
        threshold: 0.1,
        include_aa: false,
    }
}

#[test]
fn equal_shots_are_the_same_and_a_changed_one_differs_in_every_pair_it_is_in() {
    let white = png(20, 10, [255, 255, 255, 255]);
    let shots = vec![Some(white.clone()), Some(white), Some(striped(20, 5))];
    let found = diff_group(&shots, &[(0, 1), (0, 2), (1, 2)], &options(), 0.0);
    assert_eq!(found[0].status, PairStatus::Same);
    assert_eq!(found[0].ratio, 0.0);
    assert!(found[0].png.is_none(), "no diff image without a difference");
    for differing in &found[1..] {
        assert_eq!(differing.status, PairStatus::Differs);
        assert!((differing.ratio - 0.25).abs() < 1e-9, "{}", differing.ratio);
        assert_eq!(differing.different_pixels, 50);
        assert!(differing.png.is_some());
    }
}

#[test]
fn a_difference_within_the_tolerance_counts_as_the_same() {
    let shots = vec![Some(striped(20, 0)), Some(striped(20, 1))];
    let found = diff_group(&shots, &[(0, 1)], &options(), 0.06);
    assert_eq!(found[0].status, PairStatus::Same);
    assert!((found[0].ratio - 0.05).abs() < 1e-9);
}

#[test]
fn a_size_mismatch_and_a_missing_shot_are_reported() {
    let shots = vec![
        Some(png(20, 10, [0, 0, 0, 255])),
        Some(png(20, 30, [0, 0, 0, 255])),
        None,
    ];
    let found = diff_group(&shots, &[(0, 1), (0, 2)], &options(), 0.0);
    assert_eq!(found[0].status, PairStatus::SizeMismatch);
    assert_eq!(found[0].ratio, 1.0);
    assert_eq!(found[0].error.as_deref(), Some("20x10 against 20x30"));
    assert_eq!(found[1].status, PairStatus::Failed);
}

fn run() -> EnvRun {
    let environment = |id: &str, url: &str| RunEnvironment {
        id: id.to_owned(),
        url: url.to_owned(),
        color: None,
    };
    let breakpoint = |name: &str| RunBreakpoint {
        name: name.to_owned(),
        label: name.to_owned(),
        width: 100,
        height: 100,
        dpr: 1.0,
    };
    let mut run = EnvRun {
        run_id: "r1".to_owned(),
        environments: vec![
            environment("local", "http://localhost:3000"),
            environment("staging", "https://staging.test/"),
        ],
        breakpoints: vec![breakpoint("mobile"), breakpoint("desktop")],
        paths: vec![
            RunPath {
                path: "/".to_owned(),
                folder: "01-index".to_owned(),
            },
            RunPath {
                path: "/about".to_owned(),
                folder: "02-about".to_owned(),
            },
        ],
        ..EnvRun::default()
    };
    run.fill(&[(0, 1)], crate::compare::plan::url_in);
    run
}

#[test]
fn a_run_holds_every_shot_and_pair_in_index_order() {
    let run = run();
    assert_eq!(run.shots.len(), 8);
    assert_eq!(run.diffs.len(), 4);
    let shot = &run.shots[run.shot_index(1, 1, 0)];
    assert_eq!((shot.path, shot.environment, shot.breakpoint), (1, 1, 0));
    assert_eq!(shot.url, "https://staging.test/about");
    assert_eq!(
        run.shots[run.shot_index(0, 0, 1)].url,
        "http://localhost:3000/"
    );
}

#[test]
fn the_summary_names_each_pair_with_its_share_in_percent() {
    let mut run = run();
    run.shots[0].file = Some("01-index/a.png".to_owned());
    run.shots[1].error = Some("timeout".to_owned());
    run.diffs[3].status = PairStatus::Differs;
    run.diffs[3].ratio = 0.123_456;
    let summary = run.summary("/out/r1", None);
    assert_eq!(
        (summary.shots, summary.shots_done, summary.shots_failed),
        (8, 2, 1)
    );
    assert_eq!(summary.differing, 1);
    let pair = &summary.pairs[3];
    assert_eq!(
        (pair.path.as_str(), pair.a.as_str(), pair.b.as_str()),
        ("/about", "local", "staging")
    );
    assert_eq!(pair.breakpoint, "desktop");
    assert!((pair.percent - 12.35).abs() < 1e-9);
    let json = serde_json::to_value(&summary).expect("json");
    assert_eq!(json["pairs"][3]["status"], "differs");
}
