//! The acceptance page `tests/fixtures/site/coverage/` end to end without a browser: its files
//! served as four views' pages (desktop 1280, laptop 1024, tablet 768, mobile 375), Chromium's
//! rule usage as it matches them (the `@media (min-width: 1024px)` rule at 1024 px and wider),
//! V8's coverage of `app.js` before and after a click; then a load run, the check run, an
//! interaction session and the findings snapshot `tests/snapshots/findings.txt`
//! (`COVERAGE_UPDATE_SNAPSHOTS=1 cargo test` writes it).

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{CheckContext, Finding};
use serde_json::{Value, json};

use crate::Coverage;
use crate::css_rules::{self, units};
use crate::fake::{FakeHost, Sheet};

const SITE: &str = include_str!("../../../tests/fixtures/site/coverage/site.css");
const UNUSED: &str = include_str!("../../../tests/fixtures/site/coverage/unused.css");
const APP: &str = include_str!("../../../tests/fixtures/site/coverage/app.js");
const INLINE: &str =
    "\n.inline-used { font-weight: 600; }\n.inline-unused { font-style: italic; }\n";
const SNAPSHOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots/findings.txt");
const BASE: &str = "http://localhost:8080/coverage/";

/// The rules of `text` whose prelude is one of `preludes`, the n-th match of each as listed.
fn ranges(text: &str, wanted: &[(&str, usize)]) -> Vec<(u32, u32)> {
    let rules = css_rules::rules(text);
    wanted
        .iter()
        .filter_map(|(prelude, nth)| {
            rules
                .iter()
                .filter(|rule| rule.prelude == *prelude)
                .nth(*nth)
                .map(|rule| (rule.start, rule.end))
        })
        .collect()
}

fn sheets(width: u32) -> Vec<Sheet> {
    let mut site = vec![("body", 0), (".layout", 0), (".lead", 0)];
    if width >= 1024 {
        site.push((".layout", 1));
    }
    vec![
        Sheet {
            id: "1",
            href: Some("http://localhost:8080/coverage/site.css"),
            text: SITE.to_owned(),
            used: ranges(SITE, &site),
        },
        Sheet {
            id: "2",
            href: Some("http://localhost:8080/coverage/unused.css"),
            text: UNUSED.to_owned(),
            used: Vec::new(),
        },
        Sheet {
            id: "3",
            href: None,
            text: INLINE.to_owned(),
            used: ranges(INLINE, &[(".inline-used", 0)]),
        },
    ]
}

/// V8's coverage of app.js: the top level ran; `countClick` after a click only.
fn app(id: &str, clicked: bool) -> Value {
    let at = |name: &str| u32::try_from(APP.find(name).expect("in app.js")).expect("small");
    let end = |from: u32| {
        let rest = &APP[from as usize..];
        from + u32::try_from(rest.find("\n}\n").expect("closed") + 2).expect("small")
    };
    let click = at("function countClick");
    let never = at("function neverCalled");
    json!({ "scriptId": id, "url": format!("{BASE}app.js"), "functions": [
        { "functionName": "", "ranges": [{ "startOffset": 0, "endOffset": units(APP), "count": 1 }] },
        { "functionName": "countClick", "ranges": [{ "startOffset": click, "endOffset": end(click), "count": u64::from(clicked) }] },
        { "functionName": "neverCalled", "ranges": [{ "startOffset": never, "endOffset": end(never), "count": 0 }] }
    ] })
}

fn host() -> FakeHost {
    let mut host = FakeHost::new();
    for (id, breakpoint, width) in [
        (1, "desktop", 1280),
        (2, "laptop", 1024),
        (3, "tablet", 768),
        (4, "mobile", 375),
    ] {
        host = host.view(id, breakpoint, width, sheets(width)).scripts(
            id,
            json!([app("1", false)]),
            json!([app("2", false)]),
        );
    }
    host
}

fn param<'a>(finding: &'a Finding, name: &str) -> &'a str {
    finding
        .params
        .iter()
        .find(|(key, _)| key == name)
        .map_or("", |(_, value)| value.as_str())
}

fn snapshot(found: &[Finding]) -> String {
    let mut lines: Vec<String> = found
        .iter()
        .map(|finding| {
            let params: Vec<String> = finding
                .params
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect();
            format!(
                "view {} {} {} {} [{}]",
                finding.view.unwrap_or_default(),
                finding.rule,
                finding.severity.as_str(),
                finding.path.as_deref().unwrap_or_default(),
                params.join("; ")
            )
        })
        .collect();
    lines.sort();
    lines.join("\n") + "\n"
}

#[test]
fn the_wide_rule_is_used_only_at_1024_px_and_wider() {
    let mut host = host();
    let answer: Value = serde_json::from_str(
        &Coverage
            .call_tool(&mut host, "get", r#"{"mode":"load"}"#)
            .expect("measured"),
    )
    .expect("JSON");
    assert!(!host.tracking());
    let ctx = |view| CheckContext {
        view: Some(view),
        url: None,
        rules: Vec::new(),
        args: "{}".to_owned(),
    };
    let mut found = Vec::new();
    for view in 1..=4 {
        found.extend(Coverage.run(&mut host, &ctx(view)).expect("check run"));
    }
    let partial: Vec<&Finding> = found.iter().filter(|f| f.rule == "rule-partial").collect();
    assert_eq!(partial.len(), 2, "{partial:?}");
    assert!(
        partial
            .iter()
            .all(|f| param(f, "breakpoints") == "desktop, laptop")
    );
    assert!(
        partial
            .iter()
            .all(|f| param(f, "unused") == "tablet, mobile")
    );
    assert!(partial.iter().all(|f| param(f, "selector") == ".layout"));
    let unused: Vec<&Finding> = found.iter().filter(|f| f.rule == "unused-file").collect();
    assert_eq!(unused.len(), 4, "every view warns about unused.css");
    assert!(unused.iter().all(|f| param(f, "percent") == "100"));
    assert!(
        unused
            .iter()
            .all(|f| f.path.as_deref() == Some("http://localhost:8080/coverage/unused.css"))
    );
    // The byte sums: unused.css whole, app.js all but the two functions that did not run.
    let unused_css = answer["files"]
        .as_array()
        .and_then(|files| {
            files
                .iter()
                .position(|f| f["url"] == format!("{BASE}unused.css"))
        })
        .expect("listed");
    let cell = &answer["views"][0]["files"][unused_css];
    assert_eq!(cell["total"], units(UNUSED));
    assert_eq!(cell["used"], 0);

    let text = snapshot(&found);
    if std::env::var_os("COVERAGE_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(SNAPSHOT, &text).expect("snapshot written");
    }
    let stored = std::fs::read_to_string(SNAPSHOT).unwrap_or_default();
    assert_eq!(
        text, stored,
        "COVERAGE_UPDATE_SNAPSHOTS=1 cargo test updates it"
    );
}

#[test]
fn a_click_during_an_interaction_marks_its_function_executed() {
    let mut host = host();
    Coverage
        .run_command(&mut host, "start", "{}")
        .expect("started");
    // The person clicks the button in the desktop view.
    host.views[0].scripts_before = json!([app("1", true)]);
    for view in &mut host.views[1..] {
        view.scripts_before = json!([app("1", false)]);
    }
    Coverage
        .run_command(&mut host, "stop", "{}")
        .expect("stopped");
    assert!(!host.tracking());
    let detail: Value = serde_json::from_str(
        &Coverage
            .call_tool(
                &mut host,
                "file",
                &json!({ "url": format!("{BASE}app.js") }).to_string(),
            )
            .expect("detail"),
    )
    .expect("JSON");
    let functions = detail["functions"].as_array().expect("functions");
    let ran = |name: &str| {
        functions
            .iter()
            .find(|f| f["name"] == name)
            .map(|f| f["executedIn"].clone())
    };
    assert_eq!(ran("countClick"), Some(json!([1])));
    assert_eq!(ran("neverCalled"), Some(json!([])));
    let never: Vec<&Finding> = host
        .reported
        .iter()
        .filter(|f| f.rule == "function-unused")
        .collect();
    assert!(never.iter().all(|f| param(f, "name") == "neverCalled"));
    assert_eq!(never.len(), 4);
}
