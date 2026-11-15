//! The fixture over every view (RB-080-09): `tests/fixtures/site/errors/` in four views as the
//! host captures it, run through the plugin's native traits against `MemoryHost` - the host's
//! rules for the capabilities, the project paths and the out folder.

use std::collections::BTreeMap;

use error_report::ErrorReport;
use error_report::feed::FEED_KEY;
use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, CheckContext, Finding, ViewInfo};
use serde_json::{Value, json};

const INDEX: &str = include_str!("../../../tests/fixtures/site/errors/index.html");
const SRCSET: &str = include_str!("../../../tests/fixtures/site/errors/srcset.html");
const SERVER: &str = include_str!("../../../tests/fixtures/site/errors/server.py");
const ORIGIN: &str = "https://127.0.0.1:18766";
const VIEWS: [(u32, &str, u32); 4] = [
    (1, "mobile", 375),
    (2, "tablet", 768),
    (3, "laptop", 1280),
    (4, "desktop", 1600),
];

fn host(page: &str) -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
        .grant(Capability::Findings);
    host.views = VIEWS
        .iter()
        .map(|&(id, name, width)| ViewInfo {
            id,
            label: format!("View {width}"),
            breakpoint: name.to_owned(),
            engine: "chromium".to_owned(),
            width,
            height: 800,
            dpr: 1.0,
            url: format!("{ORIGIN}/errors/{page}"),
            title: String::new(),
        })
        .collect();
    host.now = 1_731_300_000;
    host
}

/// What errors/index.html reports in one view over https, as the host captures it.
fn index_view(id: u32) -> Value {
    let at = 1_731_300_000_000.0 + f64::from(id) * 10.0;
    let page = format!("{ORIGIN}/errors/index.html");
    json!({ "view": id,
        "console": [
            { "level": "error", "source": "network", "time": at + 2.0,
              "text": "Failed to load resource: the server responded with a status of 404 (Not Found)" },
            { "level": "error", "source": "security", "time": at + 3.0, "url": page, "line": 15,
              "text": "Refused to load the image 'https://csp-blocked.invalid/pixel.png' because it violates the following Content Security Policy directive: \"img-src 'self' data:\"." }
        ],
        "errors": [{ "text": format!("Uncaught Error: error-report fixture: boom\n    at {page}:31:34"),
                     "url": page, "line": 31, "column": 34, "time": at + 9.0 }],
        "network": [
            { "id": "1", "url": page, "method": "GET", "resourceType": "Document", "status": 200, "time": at },
            { "id": "2", "url": format!("{ORIGIN}/errors/missing.png"), "method": "GET",
              "resourceType": "Image", "status": 404, "time": at + 1.0 },
            { "id": "3", "url": "https://csp-blocked.invalid/pixel.png", "method": "GET",
              "resourceType": "Image", "failed": "net::ERR_BLOCKED_BY_CLIENT", "time": at + 1.0 },
            { "id": "4", "url": format!("{ORIGIN}/errors/boom.js"), "method": "GET",
              "resourceType": "Script", "status": 500, "time": at + 2.0 },
            { "id": "5", "url": "http://127.0.0.1:18766/errors/mixed.js", "method": "GET",
              "resourceType": "Script", "failed": "net::ERR_BLOCKED_BY_CLIENT", "time": at + 4.0 },
            { "id": "6", "url": page, "method": "GET", "resourceType": "XHR", "status": 200, "time": at + 5.0 }
        ],
        "issues": [
            { "code": "ContentSecurityPolicyIssue", "kind": "img-src", "detail": "kURLViolation",
              "url": "https://csp-blocked.invalid/pixel.png", "sourceUrl": page, "line": 15, "time": at + 3.0 },
            { "code": "MixedContentIssue", "kind": "MixedContentBlocked", "detail": "Script",
              "url": "http://127.0.0.1:18766/errors/mixed.js", "sourceUrl": page, "time": at + 4.0 },
            { "code": "DeprecationIssue", "kind": "XHRSynchronous", "sourceUrl": page, "line": 28,
              "column": 7, "time": at + 5.0 }
        ] })
}

fn capture(views: Vec<Value>) -> String {
    json!({ FEED_KEY: { "views": views } }).to_string()
}

/// `checks_run` over the four views, merged as the host's runner merges: by rule and path.
fn merged(host: &mut MemoryHost, args: &str) -> BTreeMap<(String, String), Vec<Finding>> {
    let rules: Vec<String> = ErrorReport
        .describe()
        .into_iter()
        .map(|rule| rule.id)
        .collect();
    let mut merged: BTreeMap<(String, String), Vec<Finding>> = BTreeMap::new();
    for &(id, _, _) in &VIEWS {
        let ctx = CheckContext {
            view: Some(id),
            url: None,
            rules: rules.clone(),
            args: args.to_owned(),
        };
        for finding in ErrorReport.run(host, &ctx).expect("runs") {
            let key = (
                finding.rule.clone(),
                finding.path.clone().unwrap_or_default(),
            );
            merged.entry(key).or_default().push(finding);
        }
    }
    merged
}

#[test]
fn the_fixture_has_one_case_per_category() {
    assert!(INDEX.contains("img-src 'self' data:"));
    assert!(INDEX.contains("src=\"missing.png\""));
    assert!(INDEX.contains("https://csp-blocked.invalid/pixel.png"));
    assert!(INDEX.contains("src=\"boom.js\""));
    assert!(INDEX.contains("'http://' + location.host + '/errors/mixed.js'"));
    assert!(INDEX.contains("xhr.open('GET', 'index.html', false)"));
    assert!(INDEX.contains("throw new Error('error-report fixture: boom')"));
    assert!(SRCSET.contains("missing-375.png 375w, wide.svg 1600w"));
    assert!(SERVER.contains("/errors/boom.js"));
}

#[test]
fn the_fixture_gives_exactly_six_findings_each_in_all_four_views() {
    let mut host = host("index.html");
    let args = capture(VIEWS.iter().map(|&(id, _, _)| index_view(id)).collect());
    let merged = merged(&mut host, &args);
    let snapshot: Vec<(String, String, usize, Option<String>)> = merged
        .iter()
        .map(|((rule, _), found)| {
            let first = &found[0];
            let only = first
                .params
                .iter()
                .find(|(name, _)| name == "views")
                .map(|(_, v)| v.clone());
            (
                rule.clone(),
                first.severity.as_str().to_owned(),
                found.len(),
                only,
            )
        })
        .collect();
    let expected: Vec<(String, String, usize, Option<String>)> = [
        ("csp-violation", "warning"),
        ("deprecation", "info"),
        ("http-not-found", "error"),
        ("http-server-error", "error"),
        ("js-exception", "error"),
        ("mixed-content-blocked", "error"),
    ]
    .iter()
    .map(|(rule, severity)| ((*rule).to_owned(), (*severity).to_owned(), 4, None))
    .collect();
    assert_eq!(snapshot, expected);
}

#[test]
fn the_srcset_404_of_the_mobile_view_is_one_finding_only_in_mobile() {
    let mut host = host("srcset.html");
    let views: Vec<Value> = VIEWS
        .iter()
        .map(|&(id, _, width)| {
            let candidate = if width <= 480 {
                ("missing-375.png", 404)
            } else {
                ("wide.svg", 200)
            };
            json!({ "view": id, "issues": [], "network": [
                { "url": format!("{ORIGIN}/errors/srcset.html"), "resourceType": "Document",
                  "status": 200, "time": 1.0 },
                { "url": format!("{ORIGIN}/errors/{}", candidate.0), "resourceType": "Image",
                  "status": candidate.1, "time": 2.0 }] })
        })
        .collect();
    let args = capture(views);
    let merged = merged(&mut host, &args);
    assert_eq!(merged.len(), 1);
    let found = merged.values().next().expect("one finding");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].view, Some(1));
    assert!(found[0].message.ends_with("http_not_found_only"));
    assert!(
        found[0]
            .params
            .contains(&("views".to_owned(), "mobile".to_owned()))
    );
}

#[test]
fn an_ignore_pattern_suppresses_the_matching_entry() {
    let mut host = host("index.html");
    host.settings.insert(
        "ignore".to_owned(),
        r#"["*csp-blocked.invalid*"]"#.to_owned(),
    );
    let args = capture(VIEWS.iter().map(|&(id, _, _)| index_view(id)).collect());
    let answer = ErrorReport
        .call_tool(&mut host, "get", &args)
        .expect("answers");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(answer["total"], 5);
    assert_eq!(answer["summary"]["csp-violation"], 0);
}

#[test]
fn get_answers_the_json_report_and_clear_empties_it() {
    let mut host = host("index.html");
    let args = capture(VIEWS.iter().map(|&(id, _, _)| index_view(id)).collect());
    let answer: Value = serde_json::from_str(
        &ErrorReport
            .call_tool(&mut host, "get", &args)
            .expect("answers"),
    )
    .expect("JSON");
    assert_eq!(answer["total"], 6);
    assert!(
        answer["entries"]
            .as_array()
            .is_some_and(|entries| entries.iter().all(|e| e["views"] == json!([1, 2, 3, 4])))
    );
    ErrorReport
        .call_tool(&mut host, "clear", &args)
        .expect("clears");
    // The host serves the out folder below `.rbrowser/`: the next call reads the mark back.
    let state = host
        .out
        .get("state.json")
        .cloned()
        .expect("the mark is written");
    host.project
        .insert(".rbrowser/out/error-report/state.json".to_owned(), state);
    let after: Value = serde_json::from_str(
        &ErrorReport
            .call_tool(&mut host, "get", &args)
            .expect("answers"),
    )
    .expect("JSON");
    assert_eq!(after["total"], 0);
}
