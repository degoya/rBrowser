//! The acceptance fixtures of RB-060-09 (`tests/fixtures/site/seo/hreflang-*/`, `parity/`): each
//! language set served through a `MemoryHost` at `https://fixture.test/<folder>/<lang>/` with the
//! headers of its `headers.json`, its `robots.txt` and sitemap; checked as `checks_run` would
//! with `{"scope": "hreflang"}`, its findings held to `tests/snapshots/<folder>.txt` (K8).
//! `RB_UPDATE_SNAPSHOTS=1 cargo test` writes the snapshots anew (review the diff).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{
    Capability, CheckContext, Failure, Finding, HttpRequest, HttpResponse, PluginHost, Severity,
    ViewId, ViewInfo,
};
use seo_plugin::{Seo, i18n};
use serde_json::{Value, json};

const ORIGIN: &str = "https://fixture.test";

fn fixture(path: &str) -> String {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/site/seo")
        .join(path);
    std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()))
}

fn answer(kind: &str, body: String, extra: &[(String, String)]) -> HttpResponse {
    let mut headers = vec![("content-type".to_owned(), kind.to_owned())];
    headers.extend(extra.iter().cloned());
    HttpResponse {
        status: 200,
        headers,
        body: body.into_bytes(),
    }
}

/// The set in `folder`: one page per language folder, its `robots.txt` as the origin's, its
/// sitemap; view 1 shows the first page.
fn site(folder: &str, languages: &[&str]) -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Http)
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::OutWrite)
        .grant(Capability::Findings);
    let headers: BTreeMap<String, BTreeMap<String, String>> =
        serde_json::from_str(&fixture(&format!("{folder}/headers.json"))).expect("headers.json");
    host.responses.insert(
        format!("{ORIGIN}/robots.txt"),
        answer("text/plain", fixture(&format!("{folder}/robots.txt")), &[]),
    );
    host.responses.insert(
        format!("{ORIGIN}/{folder}/sitemap.xml"),
        answer(
            "application/xml",
            fixture(&format!("{folder}/sitemap.xml")),
            &[],
        ),
    );
    for lang in languages {
        let extra: Vec<(String, String)> = headers
            .get(&format!("{lang}/"))
            .map(|map| map.clone().into_iter().collect())
            .unwrap_or_default();
        host.responses.insert(
            format!("{ORIGIN}/{folder}/{lang}/"),
            answer(
                "text/html; charset=utf-8",
                fixture(&format!("{folder}/{lang}/index.html")),
                &extra,
            ),
        );
    }
    let first = format!("{ORIGIN}/{folder}/{}/", languages[0]);
    host.views.push(ViewInfo {
        id: 1,
        url: first.clone(),
        engine: "chromium".to_owned(),
        ..ViewInfo::default()
    });
    host.evaluate_answer = json!({
        "url": first,
        "html": fixture(&format!("{folder}/{}/index.html", languages[0])),
    })
    .to_string();
    host
}

fn check(host: &mut dyn PluginHost, url: &str, args: Value, rules: &[&str]) -> Vec<Finding> {
    let ctx = CheckContext {
        view: Some(1),
        url: Some(url.to_owned()),
        rules: rules.iter().map(|rule| (*rule).to_owned()).collect(),
        args: args.to_string(),
    };
    Seo.run(host, &ctx).expect("the check runs")
}

/// One line per finding: severity, rule, message, the page's path and the parameters.
fn lines(findings: &[Finding]) -> String {
    let mut out = String::new();
    for finding in findings {
        let mut params: Vec<String> = finding
            .params
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        params.sort();
        let line = format!(
            "{} {} {} {} {}",
            finding.severity.as_str(),
            finding.rule,
            finding
                .message
                .strip_prefix("plugin.seo.findings.")
                .unwrap_or(&finding.message),
            finding
                .url
                .as_deref()
                .unwrap_or("-")
                .strip_prefix(ORIGIN)
                .unwrap_or("-"),
            params.join(" | "),
        );
        let _ = writeln!(out, "{}", line.trim_end());
    }
    out
}

fn snapshot(name: &str, findings: &[Finding]) {
    for finding in findings {
        let text = i18n::text(&finding.message, &finding.params);
        assert!(
            !text.starts_with("plugin."),
            "no text for {}",
            finding.message
        );
        assert!(!text.contains('{'), "a placeholder left in {text}");
    }
    let actual = lines(findings);
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.txt"));
    if std::env::var_os("RB_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&file, &actual).expect("snapshot written");
        return;
    }
    let expected = std::fs::read_to_string(&file).unwrap_or_default();
    assert_eq!(actual, expected, "snapshot {name}");
}

const BROKEN: &[&str] = &["en", "uk", "de", "fr"];

#[test]
fn k8_the_broken_set_gives_exactly_the_expected_findings() {
    let mut host = site("hreflang-broken", BROKEN);
    let findings = check(
        &mut host,
        &format!("{ORIGIN}/hreflang-broken/en/"),
        json!({"scope": "hreflang"}),
        &[],
    );
    snapshot("hreflang-broken", &findings);
    let fetched: Vec<&str> = host.fetched.iter().map(|r| r.url.as_str()).collect();
    assert!(fetched.contains(&"https://fixture.test/hreflang-broken/es/"));
}

#[test]
fn k8_the_correct_set_has_no_finding() {
    let mut host = site("hreflang-ok", &["en", "de", "fr"]);
    host.settings
        .insert("locales".to_owned(), r#"["en", "de", "fr"]"#.to_owned());
    host.settings.insert(
        "hreflang.checkContentLanguage".to_owned(),
        "true".to_owned(),
    );
    let findings = check(
        &mut host,
        &format!("{ORIGIN}/hreflang-ok/en/"),
        json!({"scope": "hreflang"}),
        &[],
    );
    assert!(
        findings.iter().all(|f| f.severity != Severity::Error),
        "{}",
        lines(&findings)
    );
    snapshot("hreflang-ok", &findings);
    assert!(findings.is_empty());
}

#[test]
fn the_tool_writes_the_matrix_with_exactly_the_missing_pairs() {
    let mut host = site("hreflang-broken", BROKEN);
    let answer = Seo
        .run_command(
            &mut host,
            "hreflang",
            &json!({"url": format!("{ORIGIN}/hreflang-broken/en/")}).to_string(),
        )
        .expect("the command runs");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(answer["scope"], "hreflang");
    assert_eq!(answer["hreflang"]["pages"], 5);
    assert_eq!(answer["hreflang"]["missing"], 1);
    let html = host
        .out
        .iter()
        .find(|(path, _)| path.ends_with("report.html"))
        .map(|(_, bytes)| String::from_utf8_lossy(bytes).into_owned())
        .expect("report.html");
    assert_eq!(html.matches("<td class=\"missing\">").count(), 1);
    assert!(html.contains(
        "<li><code>https://fixture.test/hreflang-broken/fr/</code> does not link to \
         <code>https://fixture.test/hreflang-broken/uk/</code></li>"
    ));
    let report = host
        .out
        .iter()
        .find(|(path, _)| path.ends_with("report.json"))
        .map(|(_, bytes)| serde_json::from_slice::<Value>(bytes).expect("json"))
        .expect("report.json");
    assert_eq!(
        report["hreflang"]["missing"].as_array().map(Vec::len),
        Some(1)
    );
    assert!(!host.findings.is_empty(), "reported to the findings panel");
}

#[test]
fn a_disallowed_alternate_is_never_fetched() {
    let mut host = site("hreflang-ok", &["en", "de", "fr"]);
    host.responses.insert(
        format!("{ORIGIN}/robots.txt"),
        answer(
            "text/plain",
            "User-agent: *\nDisallow: /hreflang-ok/fr/\n".to_owned(),
            &[],
        ),
    );
    let findings = check(
        &mut host,
        &format!("{ORIGIN}/hreflang-ok/en/"),
        json!({"scope": "hreflang"}),
        &["hreflang.target_unchecked"],
    );
    assert!(host.fetched.iter().all(|r| !r.url.ends_with("/fr/")));
    assert_eq!(
        lines(&findings),
        "info hreflang.target_unchecked hreflang.unchecked_robots /hreflang-ok/fr/\n"
    );
}

/// A `MemoryHost` whose `evaluate` answers per view.
struct ViewsHost {
    inner: MemoryHost,
    answers: BTreeMap<ViewId, String>,
}

impl PluginHost for ViewsHost {
    fn log(&mut self, level: &str, message: &str) {
        self.inner.log(level, message);
    }
    fn now_unix_seconds(&mut self) -> u64 {
        0
    }
    fn setting(&mut self, key: &str) -> Option<String> {
        self.inner.setting(key)
    }
    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        self.inner.views()
    }
    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        self.inner.active_view()
    }
    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.inner.evaluate(view, script)?;
        Ok(self.answers.get(&view).cloned().unwrap_or_default())
    }
    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        self.inner.cdp_send(view, method, params)
    }
    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure> {
        self.inner.fetch(request)
    }
    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.inner.project_read(path)
    }
    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        self.inner.project_list(dir)
    }
    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.inner.out_write(path, bytes)
    }
    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        self.inner.report_findings(items)
    }
    fn sidecar_call(&mut self, method: &str, params: &str) -> Result<String, Failure> {
        self.inner.sidecar_call(method, params)
    }
}

#[test]
fn parity_finds_the_h1_only_on_desktop_and_the_json_ld_only_on_mobile() {
    let url = format!("{ORIGIN}/parity/");
    let mut inner = MemoryHost::new()
        .grant(Capability::Http)
        .grant(Capability::Views)
        .grant(Capability::Evaluate);
    for (id, breakpoint, width) in [(1, "desktop", 1280), (2, "mobile", 375)] {
        inner.views.push(ViewInfo {
            id,
            label: breakpoint.to_owned(),
            breakpoint: breakpoint.to_owned(),
            engine: "chromium".to_owned(),
            width,
            url: url.clone(),
            ..ViewInfo::default()
        });
    }
    let collected = |file: &str| {
        let html = fixture(&format!("parity/{file}"));
        json!({ "url": url, "html": html, "text": "Our red bicycles are made for the beach and the city." })
            .to_string()
    };
    let mut host = ViewsHost {
        inner,
        answers: BTreeMap::from([
            (1, collected("desktop.html")),
            (2, collected("mobile.html")),
        ]),
    };
    let rules = [
        "parity.title",
        "parity.h1",
        "parity.canonical",
        "parity.hreflang",
        "parity.structured_data",
        "parity.text",
    ];
    let findings = check(&mut host, &url, json!({"parity": true}), &rules);
    assert_eq!(
        lines(&findings),
        "warning parity.h1 parity.differs /parity/ breakpoint=desktop | field=h1 | reference=mobile | reference_value=- | value=Red bicycles\n\
         warning parity.structured_data parity.differs /parity/ breakpoint=desktop | field=structured data | reference=mobile | reference_value=Product | value=-\n"
    );
    assert!(findings.iter().all(|f| f.view == Some(1)));
}
