use super::*;
use crate::fake::{fixture, view_fonts};
use crate::run::Rendered;

fn views() -> (ViewFonts, ViewFonts) {
    (
        view_fonts(1, "Desktop", 1280, fixture("wide"), Rendered::Skipped),
        view_fonts(2, "Mobile", 375, fixture("narrow"), Rendered::Skipped),
    )
}

#[test]
fn an_h1_of_the_same_size_on_every_breakpoint_is_not_scaled() {
    let (wide, narrow) = views();
    let found = findings(&[], 1, &[&wide, &narrow], &Settings::default());
    assert_eq!(found.len(), 1);
    let finding = &found[0];
    assert_eq!(finding.rule, "fluid-not-scaled");
    assert_eq!(finding.view, Some(1));
    assert_eq!(finding.path.as_deref(), Some("html > body > header > h1"));
    let params: Vec<String> = finding
        .params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    assert_eq!(
        params,
        [
            "role=h1",
            "narrowSize=32",
            "narrow=375",
            "wideSize=32",
            "wide=1280",
            "ratio=1.15"
        ]
    );
}

#[test]
fn the_threshold_comes_from_the_settings() {
    let (wide, narrow) = views();
    let strict = Settings {
        fluid_min_ratio: 1.5,
        ..Settings::default()
    };
    // h2 grows 20 -> 28 (1.4): enough by default, not for 1.5.
    let found = findings(&[], 1, &[&wide, &narrow], &strict);
    let roles: Vec<&str> = found.iter().map(|f| f.params[0].1.as_str()).collect();
    assert_eq!(roles, ["h1", "h2"]);
    let lenient = Settings {
        fluid_min_ratio: 1.0,
        ..Settings::default()
    };
    assert!(findings(&[], 1, &[&wide, &narrow], &lenient).is_empty());
}

#[test]
fn views_too_close_in_width_or_alone_are_not_judged() {
    let (wide, _) = views();
    let close = view_fonts(3, "Laptop", 1024, fixture("wide"), Rendered::Skipped);
    assert!(findings(&[], 1, &[&wide, &close], &Settings::default()).is_empty());
    assert!(findings(&[], 1, &[&wide], &Settings::default()).is_empty());
    assert!(findings(&[], 9, &[&wide], &Settings::default()).is_empty());
}

#[test]
fn the_series_are_ordered_by_width_and_mark_jumps() {
    let (wide, narrow) = views();
    let json = json(&[&wide, &narrow], &Settings::default());
    let h2 = json.iter().find(|r| r["role"] == "h2").expect("h2");
    assert_eq!(h2["views"][0]["view"], 2);
    assert_eq!(h2["growth"], 1.4);
    assert_eq!(h2["scaled"], true);
    assert_eq!(h2["jump"], false);
    let h1 = json.iter().find(|r| r["role"] == "h1").expect("h1");
    assert_eq!(h1["scaled"], false);
    let mut big = fixture("wide");
    big.roles[0].max = 64.0;
    let big = view_fonts(1, "Desktop", 1280, big, Rendered::Skipped);
    let series = series(&[&big, &narrow]);
    assert!(series.iter().find(|s| s.role == "h1").expect("h1").jump());
}
