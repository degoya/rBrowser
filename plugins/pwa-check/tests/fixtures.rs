//! The acceptance pages `tests/fixtures/site/pwa-ok/` and `pwa-broken/` end to end without a
//! browser: their files served as HTTP at `http://localhost:8080/<folder>/` (an unknown file
//! answers 404), what the collector reports for each page (`tests/fixtures/<folder>.json`), no
//! installability error from Chromium, then the MCP tool `pwa_check` and the check run of
//! `checks_run`, the findings held to the snapshots `tests/snapshots/<folder>.txt`
//! (`PWA_UPDATE_SNAPSHOTS=1 cargo test` writes them).

use std::path::PathBuf;

use pwa_check::PwaCheck;
use pwa_check::collect::COLLECTOR;
use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::{
    CheckContext, Failure, Finding, HttpRequest, HttpResponse, PluginHost, ViewId, ViewInfo,
};
use serde_json::Value;

const ORIGIN: &str = "http://localhost:8080/";

fn site() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/site")
}

/// Two Chromium views of one fixture page.
struct Fixture {
    folder: &'static str,
    fetched: Vec<String>,
    reported: Vec<Finding>,
}

impl Fixture {
    fn new(folder: &'static str) -> Self {
        Self {
            folder,
            fetched: Vec::new(),
            reported: Vec::new(),
        }
    }

    fn url(&self) -> String {
        format!("{ORIGIN}{}/", self.folder)
    }
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("webmanifest") => "application/manifest+json",
        Some("json") => "application/json",
        Some("js") => "text/javascript",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

impl PluginHost for Fixture {
    fn log(&mut self, _level: &str, _message: &str) {}

    fn now_unix_seconds(&mut self) -> u64 {
        0
    }

    fn setting(&mut self, _key: &str) -> Option<String> {
        None
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        let url = self.url();
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
                url: url.clone(),
                title: String::new(),
            })
            .collect())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(Some(1))
    }

    fn evaluate(&mut self, _view: ViewId, script: &str) -> Result<String, Failure> {
        if script == COLLECTOR {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("tests/fixtures/{}.json", self.folder));
            Ok(std::fs::read_to_string(path).expect("collector answer"))
        } else {
            // The offline probe: the fixture's service worker caches its start URL.
            Ok("true".to_owned())
        }
    }

    fn cdp_send(&mut self, _view: ViewId, method: &str, _params: &str) -> Result<String, Failure> {
        assert_eq!(method, "Page.getInstallabilityErrors");
        Ok(r#"{"installabilityErrors":[]}"#.to_owned())
    }

    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure> {
        assert_eq!(request.method, "GET");
        self.fetched.push(request.url.clone());
        let relative = request
            .url
            .strip_prefix(ORIGIN)
            .expect("the fixture origin");
        let mut file = site().join(relative.split('?').next().unwrap_or_default());
        if relative.ends_with('/') {
            file = file.join("index.html");
        }
        Ok(match std::fs::read(&file) {
            Ok(body) => HttpResponse {
                status: 200,
                headers: vec![(
                    "Content-Type".to_owned(),
                    content_type(&file.to_string_lossy()).to_owned(),
                )],
                body,
            },
            Err(_) => HttpResponse {
                status: 404,
                headers: Vec::new(),
                body: b"not found".to_vec(),
            },
        })
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        panic!("no project read: {path}");
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        panic!("no project list: {dir}");
    }

    fn out_write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), Failure> {
        Ok(())
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        self.reported.extend_from_slice(items);
        Ok(())
    }

    fn sidecar_call(&mut self, method: &str, _params: &str) -> Result<String, Failure> {
        panic!("no sidecar: {method}");
    }
}

/// One line per finding: severity, rule, place and parameters.
fn lines(findings: &[Value]) -> String {
    findings
        .iter()
        .map(|finding| {
            let params: Vec<String> = finding["params"]
                .as_object()
                .map(|params| {
                    params
                        .iter()
                        .map(|(key, value)| format!("{key}={}", value.as_str().unwrap_or_default()))
                        .collect()
                })
                .unwrap_or_default();
            let line = format!(
                "{} {} {} {}",
                finding["severity"].as_str().unwrap_or_default(),
                finding["rule"].as_str().unwrap_or_default(),
                finding["path"].as_str().unwrap_or_default(),
                params.join(" ")
            );
            format!("{}\n", line.trim_end())
        })
        .collect()
}

fn snapshot(folder: &str, actual: &str) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/snapshots/{folder}.txt"));
    if std::env::var_os("PWA_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, actual).expect("snapshot written");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        actual, expected,
        "{folder}: PWA_UPDATE_SNAPSHOTS=1 rewrites the snapshot"
    );
}

fn tool(host: &mut Fixture) -> Value {
    let answer = PwaCheck
        .call_tool(host, "check", r#"{"target":"all"}"#)
        .expect("pwa_check");
    serde_json::from_str(&answer).expect("json")
}

#[test]
fn pwa_ok_has_no_error_and_every_icon_loads() {
    let mut host = Fixture::new("pwa-ok");
    let answer = tool(&mut host);
    assert_eq!(answer["summary"]["error"], 0, "{answer}");
    assert_eq!(answer["summary"]["warning"], 0, "{answer}");
    let page = &answer["pages"][0];
    assert_eq!(page["views"], serde_json::json!([1, 2]));
    let icons = page["icons"].as_array().expect("icons");
    assert_eq!(icons.len(), 3);
    assert!(icons.iter().all(|icon| icon["state"] == "ok"), "{icons:?}");
    assert_eq!(icons[2]["purpose"], serde_json::json!(["maskable"]));
    assert_eq!(page["serviceWorker"]["fetchHandler"], true);
    assert_eq!(page["offline"]["cached"], true);
    assert_eq!(page["manifest"]["startUrlStatus"], "200");
    snapshot(
        "pwa-ok",
        &lines(answer["findings"].as_array().expect("findings")),
    );
}

#[test]
fn pwa_broken_gives_exactly_the_expected_findings() {
    let mut host = Fixture::new("pwa-broken");
    let answer = tool(&mut host);
    let findings = answer["findings"].as_array().expect("findings");
    let rules: Vec<&str> = findings.iter().filter_map(|f| f["rule"].as_str()).collect();
    for expected in [
        "manifest.icon_size",
        "icons.unreachable",
        "start_url.out_of_scope",
        "sw.missing",
    ] {
        assert!(rules.contains(&expected), "{expected} in {rules:?}");
    }
    snapshot("pwa-broken", &lines(findings));
}

#[test]
fn the_check_run_gives_the_same_findings_in_every_view() {
    let mut host = Fixture::new("pwa-broken");
    let ctx = CheckContext {
        view: None,
        url: None,
        rules: Vec::new(),
        args: "{}".to_owned(),
    };
    let findings = PwaCheck.run(&mut host, &ctx).expect("findings");
    let in_view = |view| findings.iter().filter(|f| f.view == Some(view)).count();
    assert!(in_view(1) > 0);
    assert_eq!(in_view(1), in_view(2));
    let mut host = Fixture::new("pwa-broken");
    let answer = PwaCheck
        .run_command(&mut host, "check", "{}")
        .expect("command");
    assert!(answer.contains("\"reported\":true"));
    assert_eq!(host.reported.len(), findings.len());
}
