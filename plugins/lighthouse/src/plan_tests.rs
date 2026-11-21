use serde_json::json;

use super::*;
use crate::fake::FakeHost;
use crate::project::{BreakpointExtras, Project};

fn views() -> Vec<ViewInfo> {
    let mut host =
        FakeHost::with_views(&[(1, "mobile", 375), (2, "desktop", 1440), (3, "mobile", 375)]);
    let mut firefox = host.views[1].clone();
    firefox.id = 4;
    firefox.breakpoint = "firefox-desktop".to_owned();
    firefox.engine = "firefox".to_owned();
    host.views.push(firefox);
    host.views
}

#[test]
fn one_target_per_chromium_breakpoint_with_its_emulation() {
    let (targets, skipped) = targets(&views(), &Project::default(), &Selection::default());
    assert_eq!(targets.len(), 2, "view 3 shows mobile again");
    let mobile = &targets[0];
    assert_eq!(
        (
            mobile.view,
            mobile.breakpoint.as_str(),
            mobile.width,
            mobile.height
        ),
        (1, "mobile", 375, 800)
    );
    assert!((mobile.dpr - 3.0).abs() < f64::EPSILON);
    assert!(mobile.mobile);
    assert!(!targets[1].mobile);
    assert_eq!(
        skipped,
        [Skipped {
            breakpoint: "firefox-desktop".to_owned(),
            view: Some(4),
            engine: Some("firefox".to_owned()),
            reason: "engine",
        }]
    );
}

#[test]
fn the_project_breakpoint_decides_mobile_and_the_user_agent() {
    let mut project = Project::default();
    project.breakpoints.insert(
        "desktop".to_owned(),
        BreakpointExtras {
            touch: Some(true),
            mobile: None,
            user_agent: Some("Tablet UA".to_owned()),
        },
    );
    project.breakpoints.insert(
        "mobile".to_owned(),
        BreakpointExtras {
            touch: Some(true),
            mobile: Some(false),
            user_agent: None,
        },
    );
    let (targets, _) = targets(&views(), &project, &Selection::default());
    assert!(!targets[0].mobile, "mobile: false wins over touch");
    assert!(targets[1].mobile, "touch without mobile");
    assert_eq!(targets[1].user_agent.as_deref(), Some("Tablet UA"));
}

#[test]
fn a_selection_narrows_the_run_and_names_unknown_breakpoints() {
    let only = vec!["desktop".to_owned(), "watch".to_owned()];
    let selection = Selection {
        only: Some(&only),
        ..Selection::default()
    };
    let (targets, skipped) = targets(&views(), &Project::default(), &selection);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].breakpoint, "desktop");
    assert_eq!(skipped[0].breakpoint, "watch");
    assert_eq!(skipped[0].reason, "not_found");

    let one_view = Selection {
        view: Some(3),
        url: Some("https://example.com/pricing"),
        ..Selection::default()
    };
    let (targets, skipped) = super::targets(&views(), &Project::default(), &one_view);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].view, 3);
    assert_eq!(targets[0].url, "https://example.com/pricing");
    assert!(skipped.is_empty());
}

#[test]
fn a_view_without_a_web_page_is_skipped() {
    let mut views = views();
    views[0].url = "about:blank".to_owned();
    views[2].url = "about:blank".to_owned();
    let (targets, skipped) = targets(&views, &Project::default(), &Selection::default());
    assert_eq!(targets.len(), 1);
    assert_eq!(skipped[0].reason, "url");
    assert!(is_web_url("HTTPS://example.com"));
    assert!(!is_web_url("file:///C:/site/index.html"));
    // RB-094-10: the app's one check - a host, nothing trimmed.
    assert!(!is_web_url(" https://example.com"));
    assert!(!is_web_url("https://"));
}

#[test]
fn categories_come_from_the_call_then_the_project_then_all() {
    let mut project = Project::default();
    assert_eq!(categories(None, &project).len(), 4);
    project.categories = vec!["seo".to_owned(), "performance".to_owned()];
    assert_eq!(categories(None, &project), ["performance", "seo"]);
    let asked = vec!["accessibility".to_owned()];
    assert_eq!(categories(Some(&asked), &project), ["accessibility"]);
    assert_eq!(categories(Some(&[]), &project), ["performance", "seo"]);
}

#[test]
fn the_settings_name_the_breakpoint_emulation() {
    let (targets, _) = targets(&views(), &Project::default(), &Selection::default());
    let categories = vec!["performance".to_owned()];
    assert_eq!(
        targets[0].settings("simulate", &categories),
        json!({
            "formFactor": "mobile",
            "screenEmulation": { "width": 375, "height": 800, "deviceScaleFactor": 3.0, "mobile": true },
            "throttlingMethod": "simulate",
            "onlyCategories": ["performance"],
        })
    );
    let mut with_agent = targets[1].clone();
    with_agent.user_agent = Some("UA".to_owned());
    assert_eq!(
        with_agent.settings("devtools", &categories)["emulatedUserAgent"],
        "UA"
    );
    assert_eq!(
        with_agent.settings("devtools", &categories)["formFactor"],
        "desktop"
    );
}

#[test]
fn file_names_are_safe() {
    let (targets, _) = targets(&views(), &Project::default(), &Selection::default());
    let mut odd = targets[0].clone();
    odd.breakpoint = "../iPhone 15 Pro".to_owned();
    assert_eq!(odd.file_name(), "-iPhone-15-Pro");
    odd.breakpoint = "..".to_owned();
    assert_eq!(odd.file_name(), "view-1");
    assert_eq!(targets[0].file_name(), "mobile");
}
