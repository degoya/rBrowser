//! The acceptance pages `tests/fixtures/site/fonts/` end to end without a browser: the collector's
//! answers (`tests/fixtures/{wide,narrow,broken}.json`, written and held fresh by
//! `web/src/fixture.test.ts`) served as the views' pages, Chromium's platform fonts as it draws
//! the pages, then the check run of each view, the inventory, MCP's `fonts_list`/`fonts_check`,
//! the export, and the findings snapshot `tests/snapshots/findings.txt`
//! (`FONTS_UPDATE_SNAPSHOTS=1 cargo test` writes it).

use std::collections::BTreeMap;

use fonts::Fonts;
use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{
    Capability, CheckContext, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost,
    ViewId, ViewInfo, codes,
};
use serde_json::{Value, json};

const WIDE: &str = include_str!("fixtures/wide.json");
const NARROW: &str = include_str!("fixtures/narrow.json");
const BROKEN: &str = include_str!("fixtures/broken.json");
const URL: &str = "http://localhost:8080/fonts/index.html";
const SNAPSHOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots/findings.txt");

/// A font Chromium drew: family, a web font, glyphs.
type Drawn = (&'static str, bool, u64);

/// What Chromium draws each sample with: `(selector, [font])`.
const DRAWN: &[(&str, &[Drawn])] = &[
    ("html > body > header > h1", &[("Inter", true, 14)]),
    ("html > body > main > p.lead", &[("Roboto", true, 80)]),
    (
        "html > body > main > p:nth-of-type(2)",
        &[("Inter", true, 23)],
    ),
    // legacy-serif.ttf is not on the server: Georgia draws it.
    ("html > body > main > p.legacy", &[("Georgia", false, 34)]),
    // The ASCII-only subset: the umlauts and the sharp s come from Arial.
    (
        "html > body > main > p.subset",
        &[("Inter", true, 64), ("Arial", false, 5)],
    ),
    // broken.html: Inter is missing, Arial draws the page.
    ("html > body > h1", &[("Arial", false, 16)]),
];

/// The views: 1 Desktop (wide), 2 Mobile (narrow), both on `URL`; or one Desktop on broken.html.
struct Fixture {
    broken: bool,
    settings: BTreeMap<String, String>,
    node: BTreeMap<u64, &'static str>,
    written: Vec<(String, Vec<u8>)>,
    reported: Vec<Finding>,
}

impl Fixture {
    fn new(broken: bool) -> Self {
        Self {
            broken,
            settings: BTreeMap::new(),
            node: BTreeMap::new(),
            written: Vec::new(),
            reported: Vec::new(),
        }
    }
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
        let list: &[(ViewId, &str, u32)] = if self.broken {
            &[(1, "Desktop", 1280)]
        } else {
            &[(1, "Desktop", 1280), (2, "Mobile", 375)]
        };
        let url = if self.broken {
            "http://localhost:8080/fonts/broken.html"
        } else {
            URL
        };
        Ok(list
            .iter()
            .map(|(id, label, width)| ViewInfo {
                id: *id,
                label: (*label).to_owned(),
                breakpoint: label.to_ascii_lowercase(),
                engine: "chromium".to_owned(),
                width: *width,
                height: 800,
                dpr: 1.0,
                url: url.to_owned(),
                title: String::new(),
            })
            .collect())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(Some(1))
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        if script.contains("FontsPage.mark(") {
            return Ok(json!({ "count": 2 }).to_string());
        }
        match (self.broken, view) {
            (true, 1) => Ok(BROKEN.to_owned()),
            (false, 1) => Ok(WIDE.to_owned()),
            (false, 2) => Ok(NARROW.to_owned()),
            _ => Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)),
        }
    }

    fn cdp_send(&mut self, _view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        let answer = match method {
            "Accessibility.getFullAXTree" => json!({ "nodes": [{ "backendDOMNodeId": 3 }] }),
            "DOM.pushNodesByBackendIdsToFrontend" => json!({ "nodeIds": [1] }),
            "DOM.querySelector" => {
                let selector = params["selector"].as_str().unwrap_or_default();
                let index = DRAWN.iter().position(|(known, _)| *known == selector);
                let id = index.map_or(0, |index| index as u64 + 10);
                if let Some(index) = index {
                    self.node.insert(id, DRAWN[index].0);
                }
                json!({ "nodeId": id })
            }
            "CSS.getPlatformFontsForNode" => {
                let id = params["nodeId"].as_u64().unwrap_or_default();
                let selector = self.node.get(&id).copied().unwrap_or_default();
                let drawn = DRAWN.iter().find(|(known, _)| *known == selector);
                let fonts: Vec<Value> = drawn
                    .map(|(_, fonts)| fonts.to_vec())
                    .unwrap_or_default()
                    .iter()
                    .map(|(family, custom, glyphs)| {
                        json!({ "familyName": family, "isCustomFont": custom, "glyphCount": glyphs })
                    })
                    .collect();
                json!({ "fonts": fonts })
            }
            other => {
                return Err(
                    Failure::coded(FailureKind::AccessDenied, codes::CDP_DOMAIN_DENIED)
                        .with_param("method", other),
                );
            }
        };
        Ok(answer.to_string())
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, _path: &str) -> Result<Vec<u8>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.written.push((path.to_owned(), bytes.to_vec()));
        Ok(())
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        self.reported.extend_from_slice(items);
        Ok(())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}

fn run(host: &mut Fixture, view: ViewId) -> Vec<Finding> {
    let ctx = CheckContext {
        view: Some(view),
        url: Some(URL.to_owned()),
        rules: Vec::new(),
        args: String::new(),
    };
    Fonts.run(host, &ctx).expect("check run")
}

fn line(finding: &Finding) -> String {
    let params: Vec<String> = finding
        .params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    format!(
        "view {} {} {} [{}] at {}",
        finding.view.unwrap_or_default(),
        finding.severity.as_str(),
        finding.rule,
        params.join(", "),
        finding.path.as_deref().unwrap_or("-"),
    )
}

fn tool(host: &mut Fixture, name: &str, args: &Value) -> Value {
    let answer = Fonts.call_tool(host, name, &args.to_string()).expect(name);
    serde_json::from_str(&answer).expect("json")
}

#[test]
fn the_findings_of_both_views_match_the_snapshot() {
    let mut host = Fixture::new(false);
    let mut lines: Vec<String> = run(&mut host, 1).iter().map(line).collect();
    lines.extend(run(&mut host, 2).iter().map(line));
    // Characters beyond ASCII as escapes, as in the committed fixtures.
    let text: String = lines
        .join("\n")
        .chars()
        .map(|c| {
            if c.is_ascii() {
                c.to_string()
            } else {
                c.escape_unicode().to_string()
            }
        })
        .collect::<String>()
        + "\n";
    if std::env::var_os("FONTS_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(SNAPSHOT, &text).expect("snapshot written");
    }
    let expected = std::fs::read_to_string(SNAPSHOT).expect("snapshot");
    assert_eq!(text, expected);
}

#[test]
fn acceptance_inventory_inter_and_roboto_with_counts_per_view() {
    let mut host = Fixture::new(false);
    let list = tool(&mut host, "list", &json!({ "target": "all" }));
    let families = list["families"].as_array().expect("families");
    let inter = families
        .iter()
        .find(|f| f["family"] == "Inter")
        .expect("Inter");
    assert_eq!(inter["provider"], "own");
    assert_eq!(inter["formats"], json!(["woff2"]));
    assert_eq!(inter["faces"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        inter["usage"],
        json!([
            { "view": 1, "elements": 8, "weights": [400, 600], "styles": ["normal"], "sizes": [11.0, 14.0, 16.0, 28.0, 32.0] },
            { "view": 2, "elements": 8, "weights": [400, 600], "styles": ["normal"], "sizes": [11.0, 14.0, 16.0, 20.0, 32.0] },
        ])
    );
    let roboto = families
        .iter()
        .find(|f| f["family"] == "Roboto")
        .expect("Roboto");
    assert_eq!(roboto["provider"], "google");
    assert_eq!(roboto["host"], "fonts.gstatic.com");
    assert_eq!(roboto["usage"][0]["elements"], 1);
    assert_eq!(roboto["usage"][1]["elements"], 1);
}

#[test]
fn acceptance_declared_inter_rendered_arial() {
    let mut host = Fixture::new(true);
    let found = run(&mut host, 1);
    let fallback: Vec<String> = found
        .iter()
        .filter(|f| f.rule == "rendered-fallback")
        .map(line)
        .collect();
    assert_eq!(
        fallback,
        [
            "view 1 error rendered-fallback [declared=Inter, rendered=Arial, text=Inter is missing] at html > body > h1"
        ]
    );
}

#[test]
fn acceptance_umlauts_from_the_fallback_are_marked() {
    let mut host = Fixture::new(false);
    let found = run(&mut host, 2);
    let glyphs: Vec<&Finding> = found
        .iter()
        .filter(|f| f.rule == "glyph-fallback")
        .collect();
    assert_eq!(glyphs.len(), 1);
    assert_eq!(
        glyphs[0].path.as_deref(),
        Some("html > body > main > p:nth-of-type(4)")
    );
    let chars = &glyphs[0]
        .params
        .iter()
        .find(|(k, _)| k == "chars")
        .expect("chars")
        .1;
    assert_eq!(chars, "\u{f6}\u{df}\u{dc}\u{e4}\u{c4}");
}

#[test]
fn acceptance_google_is_a_privacy_finding_until_the_host_is_allowed() {
    let mut host = Fixture::new(false);
    assert!(run(&mut host, 1).iter().any(|f| f.rule == "external-host"));
    host.settings.insert(
        "allowedHosts".to_owned(),
        r#"["fonts.gstatic.com"]"#.to_owned(),
    );
    assert!(!run(&mut host, 1).iter().any(|f| f.rule == "external-host"));
}

#[test]
fn acceptance_display_ttf_and_unused_face_each_give_a_performance_finding() {
    let mut host = Fixture::new(false);
    let found = run(&mut host, 1);
    for rule in ["font-display-missing", "format-not-woff2", "face-unused"] {
        assert_eq!(found.iter().filter(|f| f.rule == rule).count(), 1, "{rule}");
    }
}

#[test]
fn acceptance_a_14px_input_on_mobile_only() {
    let mut host = Fixture::new(false);
    assert!(
        run(&mut host, 2)
            .iter()
            .any(|f| f.rule == "input-too-small")
    );
    assert!(
        !run(&mut host, 1)
            .iter()
            .any(|f| f.rule == "input-too-small")
    );
}

#[test]
fn acceptance_an_h1_of_32px_everywhere_is_not_scaled_by_the_setting() {
    let mut host = Fixture::new(false);
    assert!(
        run(&mut host, 1)
            .iter()
            .any(|f| f.rule == "fluid-not-scaled")
    );
    host.settings
        .insert("fluidMinRatio".to_owned(), "1".to_owned());
    assert!(
        !run(&mut host, 1)
            .iter()
            .any(|f| f.rule == "fluid-not-scaled")
    );
}

#[test]
fn acceptance_list_check_and_export_answer_json_and_csv() {
    let mut host = Fixture::new(false);
    let check = tool(&mut host, "check", &json!({ "target": "all" }));
    assert!(check["findings"].as_array().is_some_and(|f| !f.is_empty()));
    assert!(check["summary"].is_object());
    let export = tool(&mut host, "export", &json!({}));
    assert_eq!(export["families"], 5);
    assert_eq!(host.written.len(), 2);
    let csv = String::from_utf8(host.written[1].1.clone()).expect("csv");
    assert_eq!(csv.lines().count(), 1 + 10);
    let mark = tool(&mut host, "mark", &json!({ "family": "Inter" }));
    assert_eq!(mark["marked"][1]["count"], 2);
    let command = Fonts
        .run_command(&mut host, "check", "{}")
        .expect("command");
    assert!(command.contains("\"findings\""));
    assert!(!host.reported.is_empty());
}
