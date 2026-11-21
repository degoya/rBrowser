//! The acceptance page `tests/fixtures/site/outline/` end to end without a browser: the
//! collector's answers for a wide and a narrow view (`tests/fixtures/{wide,narrow}.json`, written
//! and held fresh by `web/src/fixture.test.ts`) served as the views' pages, the fixture server's
//! answers as HTTP, then the check run, the merged outline and the findings snapshot
//! `tests/snapshots/findings.txt` (`OUTLINE_UPDATE_SNAPSHOTS=1 cargo test` writes it).

use std::collections::BTreeMap;

use outline::Outline;
use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{
    Capability, CheckContext, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost,
    ViewId, ViewInfo, codes,
};
use serde_json::Value;

const WIDE: &str = include_str!("fixtures/wide.json");
const NARROW: &str = include_str!("fixtures/narrow.json");
const URL: &str = "http://localhost:8080/outline/index.html";
const SNAPSHOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots/findings.txt");

/// Two views of the fixture: 1 Desktop (wide), 2 Mobile (narrow); the server answers every
/// fixture page but `missing.html`.
#[derive(Default)]
struct Fixture {
    fetched: Vec<String>,
    settings: BTreeMap<String, String>,
}

impl PluginHost for Fixture {
    fn log(&mut self, _level: &str, _message: &str) {}

    fn now_unix_seconds(&mut self) -> u64 {
        0
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Ok([(1, "Desktop", 1280), (2, "Mobile", 375)]
            .into_iter()
            .map(|(id, label, width)| ViewInfo {
                id,
                label: label.to_owned(),
                breakpoint: label.to_ascii_lowercase(),
                engine: "chromium".to_owned(),
                width,
                height: 800,
                dpr: 1.0,
                url: URL.to_owned(),
                title: String::new(),
            })
            .collect())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(Some(1))
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        if script.starts_with("new Promise") {
            return Ok("null".to_owned());
        }
        match view {
            1 => Ok(WIDE.to_owned()),
            2 => Ok(NARROW.to_owned()),
            _ => Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)),
        }
    }

    fn cdp_send(&mut self, _view: ViewId, _method: &str, _params: &str) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure> {
        self.fetched.push(request.url.clone());
        let status = if request.url.ends_with("/missing.html") {
            404
        } else {
            200
        };
        Ok(HttpResponse {
            status,
            ..HttpResponse::default()
        })
    }

    fn project_read(&mut self, _path: &str) -> Result<Vec<u8>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn out_write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), Failure> {
        Err(Capability::OutWrite.denied())
    }

    fn report_findings(&mut self, _items: &[Finding]) -> Result<(), Failure> {
        Ok(())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}

fn fixture() -> Fixture {
    let mut host = Fixture::default();
    host.settings.insert(
        "customSelectors".to_owned(),
        r#"[{"label":"CMS blocks","selector":"[data-cms-block]"}]"#.to_owned(),
    );
    host
}

fn run(host: &mut Fixture, view: ViewId) -> Vec<Finding> {
    let ctx = CheckContext {
        view: Some(view),
        url: Some(URL.to_owned()),
        rules: Vec::new(),
        args: String::new(),
    };
    Outline.run(host, &ctx).expect("findings")
}

fn line(finding: &Finding) -> String {
    let params: Vec<String> = finding
        .params
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    format!(
        "view {} {} {} [{}] at {}",
        finding.view.unwrap_or_default(),
        finding.severity.as_str(),
        finding.rule,
        params.join(", "),
        finding.path.as_deref().unwrap_or("-")
    )
}

#[test]
fn the_findings_of_both_views_match_the_snapshot() {
    let mut host = fixture();
    let mut findings = run(&mut host, 1);
    findings.extend(run(&mut host, 2));
    let text: String = findings.iter().map(|f| format!("{}\n", line(f))).collect();
    if std::env::var("OUTLINE_UPDATE_SNAPSHOTS").as_deref() == Ok("1") {
        std::fs::write(SNAPSHOT, &text).expect("snapshot written");
    }
    let expected = std::fs::read_to_string(SNAPSHOT).expect("snapshot");
    assert_eq!(text, expected);
}

#[test]
fn the_heading_jump_is_a_finding_with_its_element_path() {
    let found = run(&mut fixture(), 1);
    let skip = found
        .iter()
        .find(|f| f.rule == "heading-skip")
        .expect("heading-skip");
    assert_eq!(
        skip.path.as_deref(),
        Some("html > body > main > section:nth-of-type(1) > h4")
    );
}

#[test]
fn the_404_link_is_broken_and_the_label_shares_its_id() {
    let mut host = fixture();
    let found = run(&mut host, 1);
    let broken: Vec<_> = found.iter().filter(|f| f.rule == "link-broken").collect();
    assert_eq!(broken.len(), 1);
    assert!(broken[0].params.contains(&(
        "url".to_owned(),
        "http://localhost:8080/outline/missing.html".to_owned()
    )));
    let label = found
        .iter()
        .find(|f| f.rule == "form-label-missing")
        .expect("label");
    assert_eq!(label.selector.as_deref(), Some("#nickname"));
    let described = Outline.describe();
    let rule = described
        .iter()
        .find(|r| r.id == "form-label-missing")
        .expect("rule");
    assert_eq!(rule.shared_id.as_deref(), Some("form-label-missing"));
    // The mobile view does not lead the page: its run requests nothing.
    let before = host.fetched.len();
    run(&mut host, 2);
    assert_eq!(host.fetched.len(), before);
}

#[test]
fn a_menu_link_only_on_the_desktop_is_one_finding_naming_the_breakpoint_missing_it() {
    // RB-094-02: the main navigation's links show only in the wide view; the merged findings
    // panel shows one finding per element, here the mobile view's.
    let mut host = fixture();
    let mut found = run(&mut host, 1);
    found.extend(run(&mut host, 2));
    let has = |finding: &Finding, key: &str, value: &str| {
        finding.params.contains(&(key.to_owned(), value.to_owned()))
    };
    let pricing: Vec<&Finding> = found
        .iter()
        .filter(|f| f.rule == "interactive-differs" && has(f, "text", "link \"Pricing\""))
        .collect();
    assert_eq!(pricing.len(), 1);
    assert_eq!(pricing[0].view, Some(2));
    assert!(has(pricing[0], "missing", "Mobile"));
    assert!(
        found.iter().all(|f| f.rule != "reading-order"),
        "the fixture's boxes are all alike: no reading order"
    );
}

fn get(host: &mut Fixture, args: &str) -> Value {
    let answer = Outline.call_tool(host, "get", args).expect("answer");
    serde_json::from_str(&answer).expect("json")
}

#[test]
fn the_closed_mobile_navigation_is_only_visible_on_desktop() {
    let answer = get(&mut fixture(), r#"{"sections":["landmarks","links"]}"#);
    let rows = answer["sections"]["landmarks"]["rows"]
        .as_array()
        .expect("rows");
    let main = rows
        .iter()
        .find(|row| row["item"]["name"] == "Main")
        .expect("main nav");
    assert_eq!(main["only"], serde_json::json!([1]));
    let docs = answer["sections"]["links"]["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["item"]["text"] == "Docs")
        .expect("docs link");
    assert_eq!(docs["only"], serde_json::json!([1]));
}

#[test]
fn the_custom_selector_lists_its_hits_per_view() {
    let answer = get(&mut fixture(), r#"{"sections":["custom"]}"#);
    let selector = &answer["sections"]["custom"]["selectors"][0];
    assert_eq!(selector["selector"], "[data-cms-block]");
    assert_eq!(selector["counts"], serde_json::json!({ "1": 3, "2": 3 }));
    let promo = selector["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["item"]["text"] == "Promotion")
        .expect("promo");
    assert_eq!(promo["only"], serde_json::json!([1]));
}

#[test]
fn outline_get_answers_each_view_with_the_chosen_sections() {
    let answer = get(
        &mut fixture(),
        r#"{"shape":"views","sections":["headings","links"]}"#,
    );
    let views = answer["views"].as_array().expect("views");
    assert_eq!(views.len(), 2);
    for view in views {
        let outline = view["outline"].as_object().expect("outline");
        assert!(outline["headings"].as_array().is_some_and(|h| h.len() == 5));
        assert!(outline["links"].is_array());
        assert!(!outline.contains_key("images"));
    }
}
