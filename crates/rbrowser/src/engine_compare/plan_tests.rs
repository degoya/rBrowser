use std::time::Duration;

use rb_config::{Breakpoint, EmulationFields, EngineCompareConfig, EngineKind};
use rb_engine::EngineHealth;

use super::*;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

fn breakpoint(json: &str) -> Breakpoint {
    serde_json::from_str(json).expect("a breakpoint")
}

#[test]
fn no_engine_named_compares_all_three_in_a_fixed_order() {
    assert_eq!(
        engines(&[]).expect("all"),
        [
            EngineKind::Chromium,
            EngineKind::Firefox,
            EngineKind::Webkit
        ]
    );
    assert_eq!(
        engines(&names(&["webkit", "Chromium", "webkit"])).expect("two"),
        [EngineKind::Chromium, EngineKind::Webkit],
        "case-insensitive, once each, in the fixed order"
    );
}

#[test]
fn an_unknown_engine_is_refused_with_its_code() {
    let error = engines(&names(&["safari"])).expect_err("not an engine");
    assert_eq!(error.code(), "engine_compare.error.unknown_engine");
    assert!(error.to_string().contains("safari"), "{error}");
}

#[test]
fn an_engine_that_cannot_run_is_reported_not_an_error() {
    let health = |kind: EngineKind| match kind {
        EngineKind::Chromium => EngineHealth::Running,
        EngineKind::Firefox => EngineHealth::Unavailable("Firefox is not installed".to_owned()),
        EngineKind::Webkit => EngineHealth::Stopped,
        EngineKind::Android => EngineHealth::Stopped,
        EngineKind::Safari | EngineKind::Ios => {
            EngineHealth::Unavailable("not compared".to_owned())
        }
    };
    let (run, skipped) = partition(&engines(&[]).expect("all"), health);
    assert_eq!(
        run,
        [EngineKind::Chromium, EngineKind::Webkit],
        "a stopped engine starts on its first view"
    );
    assert_eq!(
        skipped,
        [SkippedEngine {
            engine: "firefox".to_owned(),
            reason: "Firefox is not installed".to_owned(),
        }]
    );
}

#[test]
fn the_breakpoint_is_the_calls_else_the_views() {
    assert_eq!(
        breakpoint_id(Some("mobile"), Some("desktop")).expect("the call's"),
        "mobile"
    );
    assert_eq!(
        breakpoint_id(None, Some("desktop")).expect("the view's"),
        "desktop"
    );
    let error = breakpoint_id(None, None).expect_err("none");
    assert_eq!(error.code(), "engine_compare.error.no_breakpoint");
}

#[test]
fn the_page_is_the_calls_else_the_views_and_only_http_https_or_file() {
    assert_eq!(
        page_url(Some("https://example.test/a"), Some("http://other.test/")).expect("the call's"),
        "https://example.test/a"
    );
    assert_eq!(
        page_url(None, Some("file:///C:/site/index.html")).expect("the view's"),
        "file:///C:/site/index.html"
    );
    for refused in [
        "javascript:alert(1)",
        "about:blank",
        "",
        "chrome://settings",
    ] {
        let error = page_url(Some(refused), None).expect_err(refused);
        assert_eq!(error.code(), "engine_compare.error.bad_url", "{refused}");
    }
    assert_eq!(
        page_url(None, None).expect_err("no page").code(),
        "engine_compare.error.bad_url"
    );
}

#[test]
fn the_shot_spec_keeps_size_and_dpr_and_lets_each_engine_be_itself() {
    let mobile = breakpoint(
        r#"{ "id": "mobile", "label": "Phone", "width": 390, "height": 844, "dpr": 3,
             "engine": "firefox", "userAgent": "Chrome on a phone", "platform": "iPhone" }"#,
    );
    let (run, spec) = shot_spec(&mobile, &EmulationFields::default(), &[], true).expect("resolves");
    assert_eq!((run.width, run.height), (390, 844));
    assert!((run.dpr - 3.0).abs() < f64::EPSILON);
    assert_eq!(run.label, "Phone");
    assert_eq!(run.name, "mobile");
    assert_eq!(spec.breakpoint, "mobile");
    assert_eq!(spec.session, "default");
    assert_eq!(spec.emulation.user_agent, None, "each engine sends its own");
    assert_eq!(spec.emulation.platform, None);
}

#[test]
fn an_untrusted_project_sends_no_extra_headers() {
    let desktop = breakpoint(
        r#"{ "id": "desktop", "width": 1280, "height": 800, "headers": { "X-Test": "1" } }"#,
    );
    let (_, trusted) = shot_spec(&desktop, &EmulationFields::default(), &[], true).expect("ok");
    let (_, untrusted) = shot_spec(&desktop, &EmulationFields::default(), &[], false).expect("ok");
    assert_eq!(trusted.emulation.headers.len(), 1);
    assert!(untrusted.emulation.headers.is_empty());
}

#[test]
fn the_effective_options_take_the_call_over_the_section_over_visual() {
    let section = EngineCompareConfig {
        threshold: Some(0.2),
        max_diff_percent: 2.0,
        wait_ms: 500,
    };
    let none = Overrides::default();
    let taken = effective(&section, 0.1, &none);
    assert!((taken.options.threshold - 0.2).abs() < f64::EPSILON);
    assert!(!taken.options.include_aa, "font smoothing never counts");
    assert!((taken.max_ratio - 0.02).abs() < f64::EPSILON);
    assert_eq!(taken.batch.wait.delay, Duration::from_millis(500));
    assert!(
        !taken.batch.full_page,
        "the viewport: the same size in every engine"
    );
    assert_eq!(taken.batch.parallel, 1, "one view per engine");
    assert_eq!(taken.batch.host_interval, Duration::ZERO);
    let call = Overrides {
        threshold: Some(0.05),
        max_diff_percent: Some(0.0),
    };
    let taken = effective(&section, 0.1, &call);
    assert!((taken.options.threshold - 0.05).abs() < f64::EPSILON);
    assert!(taken.max_ratio.abs() < f64::EPSILON);
    let unset = EngineCompareConfig::default();
    assert!((effective(&unset, 0.15, &none).options.threshold - 0.15).abs() < f64::EPSILON);
}

#[test]
fn the_effective_options_hold_their_bounds() {
    let wild = EngineCompareConfig {
        threshold: Some(7.0),
        max_diff_percent: 900.0,
        wait_ms: 999_999,
    };
    let taken = effective(&wild, 0.1, &Overrides::default());
    assert!(
        (taken.options.threshold - 0.1).abs() < f64::EPSILON,
        "out of range: visual.threshold"
    );
    assert!((taken.max_ratio - 1.0).abs() < f64::EPSILON);
    assert_eq!(
        taken.batch.wait.delay,
        Duration::from_millis(u64::from(MAX_WAIT_MS))
    );
    let nan = EngineCompareConfig {
        max_diff_percent: f64::NAN,
        ..EngineCompareConfig::default()
    };
    assert!(effective(&nan, 0.1, &Overrides::default()).max_ratio.abs() < f64::EPSILON);
}

/// Security review w28: an agent compares http(s) pages and the project's own files only, as
/// MCP's `navigate` loads them; a person's command keeps every file.
#[test]
fn an_agent_may_compare_http_and_the_projects_own_files_only() {
    let project = tempfile::tempdir().expect("temp");
    let page = project.path().join("index.html");
    std::fs::write(&page, "<h1>x</h1>").expect("page");
    let inside = format!("file://{}", page.display());
    assert!(agent_url(&inside, Some(project.path())).is_ok(), "{inside}");
    for url in ["https://example.test/", "http://localhost:3000/a"] {
        assert_eq!(agent_url(url, Some(project.path())).as_deref(), Ok(url));
    }
    for refused in [
        "file:///C:/Windows/win.ini",
        "file:///etc/passwd",
        "file:///C:/Users/someone/.aws/credentials",
    ] {
        let error = agent_url(refused, Some(project.path())).expect_err(refused);
        assert_eq!(
            error.code(),
            "engine_compare.error.url_refused",
            "{refused}"
        );
    }
    assert!(agent_url(&inside, None).is_err(), "no project, no file");
}
