// RB-090-73: the Lighthouse bar's words - which run a view shows, its four scores with the
// colour and shape of Lighthouse's thresholds, the tooltip and the form that fits the strip.

use serde_json::json;

use super::*;
use crate::view_footer::model::contrast;

const RUN: &str = "20261117-143200";

fn summary(entries: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "version": 1, "runId": RUN, "startedAt": 1_794_925_920_u64,
        "url": null, "breakpoints": entries, "cancelled": false,
    }))
    .expect("json")
}

fn entry(breakpoint: &str, url: &str, scores: serde_json::Value) -> serde_json::Value {
    json!({
        "breakpoint": breakpoint, "view": 1, "url": url, "finalUrl": url,
        "scores": scores,
        "report": format!(".rbrowser/out/lighthouse/{RUN}/{breakpoint}.html"),
        "json": format!(".rbrowser/out/lighthouse/{RUN}/{breakpoint}.json"),
    })
}

fn all_scores() -> serde_json::Value {
    json!({ "performance": 72, "accessibility": 96, "best-practices": 81, "seo": 100 })
}

fn measured(run: &str, breakpoint: &str, document: &str) -> Measured {
    Measured {
        run: run.to_owned(),
        started_at: Some(1_794_925_920),
        breakpoint: breakpoint.to_owned(),
        documents: vec![document.to_owned()],
        gauges: vec![Gauge {
            category: Category::Performance,
            score: Some(72),
        }],
    }
}

#[test]
fn the_thresholds_are_lighthouses() {
    assert_eq!(rating(0), Rating::Poor);
    assert_eq!(rating(49), Rating::Poor);
    assert_eq!(rating(50), Rating::NeedsImprovement);
    assert_eq!(rating(89), Rating::NeedsImprovement);
    assert_eq!(rating(90), Rating::Good);
    assert_eq!(rating(100), Rating::Good);
}

#[test]
fn a_run_summary_gives_each_breakpoint_its_four_scores_in_lighthouses_order() {
    let runs = parse_run(
        RUN,
        &summary(json!([
            entry("md", "https://example.test/shop#top", all_scores()),
            entry(
                "sm",
                "https://example.test/",
                json!({ "seo": 90, "performance": null })
            ),
        ])),
    );
    assert_eq!(runs.len(), 2);
    let md = &runs[0];
    assert_eq!(md.run, RUN);
    assert_eq!(md.breakpoint, "md");
    assert_eq!(md.started_at, Some(1_794_925_920));
    assert_eq!(md.documents, ["https://example.test/shop"], "no fragment");
    let scores: Vec<(Category, Option<u8>)> = md
        .gauges
        .iter()
        .map(|gauge| (gauge.category, gauge.score))
        .collect();
    assert_eq!(
        scores,
        [
            (Category::Performance, Some(72)),
            (Category::Accessibility, Some(96)),
            (Category::BestPractices, Some(81)),
            (Category::Seo, Some(100)),
        ]
    );
    // A run of two categories shows two; a category Lighthouse gave no score keeps its place.
    let sm: Vec<(Category, Option<u8>)> = runs[1]
        .gauges
        .iter()
        .map(|gauge| (gauge.category, gauge.score))
        .collect();
    assert_eq!(
        sm,
        [(Category::Performance, None), (Category::Seo, Some(90))]
    );
}

#[test]
fn a_summary_that_does_not_read_or_points_elsewhere_gives_nothing() {
    assert!(parse_run(RUN, b"not json").is_empty());
    assert!(parse_run(RUN, &summary(json!("no list"))).is_empty());
    // A report outside the run's own folder, a score past 100, no score at all, no name.
    let mut elsewhere = entry("md", "https://example.test/", all_scores());
    elsewhere["report"] = json!(".rbrowser/out/lighthouse/20250101-000000/md.html");
    let mut not_html = entry("md", "https://example.test/", all_scores());
    not_html["report"] = json!(format!(".rbrowser/out/lighthouse/{RUN}/md.json"));
    let runs = parse_run(
        RUN,
        &summary(json!([
            elsewhere,
            not_html,
            entry("lg", "https://example.test/", json!({ "performance": 101 })),
            entry("xl", "https://example.test/", json!({})),
            entry("", "https://example.test/", all_scores()),
        ])),
    );
    assert!(runs.is_empty(), "{runs:?}");
}

#[test]
fn a_view_shows_the_newest_run_of_its_breakpoint_and_its_page() {
    let runs = vec![
        measured("20261117-100000", "md", "https://example.test/"),
        measured("20261117-120000", "md", "https://example.test/"),
        measured("20261117-130000", "md", "https://example.test/other"),
        measured("20261117-140000", "sm", "https://example.test/"),
    ];
    let found = latest(&runs, "https://example.test/#hash", "md").map(|run| run.run.as_str());
    assert_eq!(
        found,
        Some("20261117-120000"),
        "the newest of md on that page"
    );
    assert_eq!(
        latest(&runs, "https://example.test/unseen", "md"),
        None,
        "a run of another page is not shown"
    );
    assert_eq!(latest(&runs, "https://example.test/", "lg"), None);
    assert_eq!(latest(&runs, "not a url", "md"), None);
}

#[test]
fn a_page_matches_by_its_address_normalised() {
    assert_eq!(
        normalize("HTTPS://Example.TEST:443/a?b=1#c").as_deref(),
        Some("https://example.test/a?b=1")
    );
    assert_eq!(
        normalize("http://example.test").as_deref(),
        Some("http://example.test/")
    );
    assert_eq!(normalize("  "), None);
    // Lighthouse measured the address after its redirect: both count.
    let mut run = measured(RUN, "md", "https://example.test/");
    run.documents.push("https://www.example.test/".to_owned());
    let runs = [run];
    assert!(latest(&runs, "https://www.example.test/", "md").is_some());
}

#[test]
fn a_free_size_is_named_as_the_run_names_it() {
    assert_eq!(breakpoint_name(ViewId(7), Some("md")), "md");
    assert_eq!(breakpoint_name(ViewId(7), None), "view-7");
    assert_eq!(breakpoint_name(ViewId(7), Some("")), "view-7");
}

#[test]
fn a_gauge_shows_the_shape_of_its_rating_and_its_score() {
    let gauge = |score| Gauge {
        category: Category::Performance,
        score,
    };
    assert_eq!(
        gauge_text(&gauge(Some(72)), Form::Bare),
        format!("{} 72", crate::view_footer::vitals::SHAPE_NEEDS_IMPROVEMENT)
    );
    assert_eq!(
        gauge_text(&gauge(Some(95)), Form::Short),
        format!("{} 95 Perf", crate::view_footer::vitals::SHAPE_GOOD)
    );
    assert_eq!(
        gauge_text(&gauge(Some(12)), Form::Named),
        format!("{} 12 Performance", crate::view_footer::vitals::SHAPE_POOR)
    );
    assert_eq!(
        gauge_text(&gauge(None), Form::Named),
        "\u{2013} Performance"
    );
    assert_eq!(gauge_text(&gauge(None), Form::Bare), "\u{2013}");
}

#[test]
fn the_names_go_first_then_the_short_names_when_the_strip_is_narrow() {
    let gauges: Vec<Gauge> = Category::ALL
        .iter()
        .map(|category| Gauge {
            category: *category,
            score: Some(100),
        })
        .collect();
    let width = |form| -> u32 {
        gauges
            .iter()
            .map(|gauge| text_width(&gauge_text(gauge, form)))
            .sum()
    };
    assert_eq!(form_for(&gauges, None), Form::Named, "not laid out yet");
    assert_eq!(form_for(&gauges, Some(width(Form::Named))), Form::Named);
    assert_eq!(form_for(&gauges, Some(width(Form::Named) - 1)), Form::Short);
    assert_eq!(form_for(&gauges, Some(width(Form::Short) - 1)), Form::Bare);
    assert_eq!(form_for(&gauges, Some(10)), Form::Bare);
}

#[test]
fn the_colours_are_lighthouses_and_read_on_both_grounds() {
    use crate::app_theme::palette::{DARK, LIGHT};
    for ground in [LIGHT.ground, DARK.ground] {
        let colors: Vec<u32> = [10, 70, 95]
            .iter()
            .map(|score| gauge_color(Some(*score), ground).expect("a colour"))
            .collect();
        assert_eq!(colors.len(), 3);
        assert!(colors[0] != colors[1] && colors[1] != colors[2]);
        for color in colors {
            assert!(
                contrast(color, ground) >= 4.5,
                "{color:06x} on {ground:06x}: {}",
                contrast(color, ground)
            );
        }
    }
    assert_eq!(gauge_color(Some(95), LIGHT.ground), Some(0x0000_8800));
    assert_eq!(gauge_color(Some(95), DARK.ground), Some(0x000c_ce6b));
    assert_eq!(
        gauge_color(None, LIGHT.ground),
        None,
        "no score: the theme's text"
    );
}

#[test]
fn the_tooltip_names_the_run_time_every_score_in_words_and_the_click() {
    let run = parse_run(
        RUN,
        &summary(json!([entry(
            "md",
            "https://example.test/",
            json!({ "performance": 72, "accessibility": 96, "best-practices": 30, "seo": null })
        )])),
    );
    // 2026-11-17 14:32 UTC, an hour east of it.
    let bar = bar(&run[0], 3600);
    assert_eq!(bar.run, RUN);
    assert_eq!(bar.breakpoint, "md");
    assert_eq!(bar.gauges.len(), 4);
    let lines: Vec<&str> = bar.tooltip.lines().collect();
    assert_eq!(
        lines,
        [
            "Lighthouse run of 2026-11-17 15:32",
            "Performance: 72 (needs improvement)",
            "Accessibility: 96 (good)",
            "Best Practices: 30 (poor)",
            "SEO: no score",
            "Click: open the report",
        ]
    );
}

#[test]
fn the_run_time_is_the_local_time() {
    assert_eq!(run_time(0, 0), "1970-01-01 00:00");
    assert_eq!(run_time(0, -3600), "1969-12-31 23:00");
    assert_eq!(run_time(1_794_925_920, 3600), "2026-11-17 15:32");
}

#[test]
fn a_gauge_is_spoken_with_its_name_its_score_and_its_rating() {
    let gauge = Gauge {
        category: Category::BestPractices,
        score: Some(81),
    };
    assert_eq!(spoken(&gauge), "Best Practices: 81 (needs improvement)");
}
