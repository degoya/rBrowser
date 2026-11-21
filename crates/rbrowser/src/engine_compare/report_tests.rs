use rb_config::{EngineCompareConfig, EngineKind};
use rb_visual::sitemap::RunBreakpoint;
use rb_visual::{Rgba, encode_png};

use super::*;
use crate::compare::report::diff_group;
use crate::engine_compare::plan::{Overrides, effective};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

fn breakpoint() -> RunBreakpoint {
    RunBreakpoint {
        name: "desktop".to_owned(),
        label: "Desktop".to_owned(),
        width: 60,
        height: 30,
        dpr: 1.0,
    }
}

fn all() -> [EngineKind; 3] {
    [
        EngineKind::Chromium,
        EngineKind::Firefox,
        EngineKind::Webkit,
    ]
}

/// The fixture page: 60 x 30 white with one dark element (x 10..30, y 5..15).
fn page(element: [u8; 4]) -> Rgba {
    let mut image = Rgba::new(60, 30, WHITE).expect("image");
    image.fill(10, 5, 20, 10, element);
    image
}

fn shot(image: &Rgba) -> Option<Vec<u8>> {
    Some(encode_png(image).expect("png"))
}

fn engine_pairs() -> Vec<(usize, usize)> {
    crate::compare::plan::pairs(3)
}

#[test]
fn the_identical_fixture_shows_no_difference_over_the_threshold() {
    let chromium = page(BLACK);
    // Firefox draws the element a shade lighter: within the colour tolerance.
    let firefox = page([8, 8, 8, 255]);
    // WebKit smooths the element's right edge: a grey column of anti-aliasing.
    let mut webkit = page(BLACK);
    for y in 7..13 {
        webkit.put(30, y, [128, 128, 128, 255]);
    }
    let effective = effective(&EngineCompareConfig::default(), 0.1, &Overrides::default());
    let found = diff_group(
        &[shot(&chromium), shot(&firefox), shot(&webkit)],
        &engine_pairs(),
        &effective.options,
        effective.max_ratio,
    );
    for pair in &found {
        assert_eq!(pair.status, PairStatus::Same, "{} / {}", pair.a, pair.b);
        assert_eq!(pair.different_pixels, 0, "{} / {}", pair.a, pair.b);
        assert!(pair.regions.is_empty());
    }
}

#[test]
fn the_element_that_wraps_only_in_firefox_is_exactly_the_difference_marked() {
    let chromium = page(BLACK);
    let webkit = page(BLACK);
    // In Firefox the element wraps: a second line below the first (x 10..30, y 18..26).
    let mut firefox = page(BLACK);
    firefox.fill(10, 18, 20, 8, BLACK);
    let effective = effective(&EngineCompareConfig::default(), 0.1, &Overrides::default());
    let found = diff_group(
        &[shot(&chromium), shot(&firefox), shot(&webkit)],
        &engine_pairs(),
        &effective.options,
        effective.max_ratio,
    );
    for pair in &found {
        if pair.a == 1 || pair.b == 1 {
            assert_eq!(pair.status, PairStatus::Differs, "{} / {}", pair.a, pair.b);
            assert_eq!(pair.regions.len(), 1, "one area: the second line");
            let area = pair.regions[0];
            assert_eq!((area.x, area.y, area.width, area.height), (10, 18, 20, 8));
            assert_eq!(area.pixels, 160);
            assert_eq!(pair.different_pixels, 160);
        } else {
            assert_eq!(pair.status, PairStatus::Same, "Chromium and WebKit agree");
            assert!(pair.regions.is_empty());
        }
    }
}

#[test]
fn a_new_run_has_the_engines_as_environments_and_one_shot_each() {
    let skipped = vec![SkippedEngine {
        engine: "webkit".to_owned(),
        reason: "WebKit is switched off".to_owned(),
    }];
    let run = EngineRun::new(
        "https://example.test/a",
        breakpoint(),
        &[EngineKind::Chromium, EngineKind::Firefox],
        skipped.clone(),
        0.01,
    );
    let ids: Vec<&str> = run.run.environments.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["chromium", "firefox"]);
    assert_eq!(run.run.shots.len(), 2);
    assert!(
        run.run
            .shots
            .iter()
            .all(|shot| shot.url == "https://example.test/a"),
        "every engine loads the same page"
    );
    assert_eq!(run.run.diffs.len(), 1);
    assert_eq!((run.run.diffs[0].a, run.run.diffs[0].b), (0, 1));
    assert_eq!(run.run.paths[0].folder, PAGE_FOLDER);
    assert_eq!(run.skipped, skipped);
    let json = serde_json::to_value(&run).expect("serializes");
    assert_eq!(json["url"], "https://example.test/a");
    assert_eq!(
        json["environments"][1]["id"], "firefox",
        "flattened: {json}"
    );
    let back: EngineRun = serde_json::from_value(json).expect("reads back");
    assert_eq!(back, run);
}

#[test]
fn the_summary_names_each_file_as_a_path_and_counts_the_differing_pairs() {
    let mut run = EngineRun::new(
        "https://example.test/",
        breakpoint(),
        &all(),
        Vec::new(),
        0.0,
    );
    run.run.run_id = "r1".to_owned();
    run.run.shots[0].file = Some("page/1-chromium--1-desktop.png".to_owned());
    run.run.shots[2].error = Some("navigate: the browser is not available".to_owned());
    run.run.diffs[0].status = PairStatus::Differs;
    run.run.diffs[0].ratio = 0.123_456;
    run.run.diffs[0].image = Some("page/diff-1-2--1-desktop.png".to_owned());
    run.run.diffs[1].status = PairStatus::Failed;
    let summary = run.summary("/p/.rbrowser/out/engine-compare/r1", None);
    assert_eq!(summary.breakpoint, "Desktop");
    let expected = std::path::Path::new("/p/.rbrowser/out/engine-compare/r1")
        .join("page/1-chromium--1-desktop.png")
        .display()
        .to_string();
    assert_eq!(summary.shots[0].file.as_deref(), Some(expected.as_str()));
    assert_eq!(summary.shots[0].engine, "chromium");
    assert_eq!(summary.shots[2].engine, "webkit");
    assert!(summary.shots[2].error.is_some());
    assert_eq!(
        (summary.pairs[0].a.as_str(), summary.pairs[0].b.as_str()),
        ("chromium", "firefox")
    );
    assert!((summary.pairs[0].percent - 12.35).abs() < 1e-9);
    assert!(
        summary.pairs[0]
            .image
            .as_deref()
            .is_some_and(|image| image.ends_with("diff-1-2--1-desktop.png"))
    );
    assert_eq!(summary.differing, 1, "a failed pair is no difference");
}

#[test]
fn the_page_names_the_engines_left_out_escaped() {
    let run = EngineRun::new(
        "https://example.test/",
        breakpoint(),
        &[EngineKind::Chromium],
        vec![SkippedEngine {
            engine: "firefox".to_owned(),
            reason: "not <installed>".to_owned(),
        }],
        0.0,
    );
    let page = render_html(&run, &|key| format!("[{key}]"));
    assert!(
        page.contains("</h1><p>[skipped]</p><ul><li>firefox: not &lt;installed&gt;</li></ul>"),
        "{page}"
    );
    let none = EngineRun::new(
        "https://example.test/",
        breakpoint(),
        &all(),
        Vec::new(),
        0.0,
    );
    assert!(!render_html(&none, &|key| format!("[{key}]")).contains("[skipped]"));
}
