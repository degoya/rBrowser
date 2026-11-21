//! The acceptance fixture of RB-093-02 end to end without a browser: the collector's answers for
//! three breakpoints at their densities (`tests/fixtures/{mobile,tablet,desktop}.json`, written
//! and held fresh by `web/src/fixture.test.ts`) served as the views' pages, then the check run of
//! `checks_run` and its findings, held to the snapshot `tests/snapshots/findings.txt`
//! (`RESPONSIVE_IMAGES_UPDATE_SNAPSHOTS=1 cargo test` writes it).

use rb_plugin_api::check::Check;
use rb_plugin_api::{
    Capability, CheckContext, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost,
    ViewId, ViewInfo, codes,
};
use responsive_images::ResponsiveImages;

const MOBILE: &str = include_str!("fixtures/mobile.json");
const TABLET: &str = include_str!("fixtures/tablet.json");
const DESKTOP: &str = include_str!("fixtures/desktop.json");
const URL: &str = "https://example.test/";
const SNAPSHOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots/findings.txt");

/// The breakpoints: id, label, width, density and the collector's answer.
const VIEWS: [(ViewId, &str, u32, f32, &str); 3] = [
    (1, "Mobile", 390, 3.0, MOBILE),
    (2, "Tablet", 768, 2.0, TABLET),
    (3, "Desktop", 1440, 1.0, DESKTOP),
];

#[derive(Default)]
struct Fixture {
    /// Every script evaluated, as `(view, script)`.
    evaluated: Vec<(ViewId, String)>,
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
        Ok(VIEWS
            .iter()
            .map(|(id, label, width, dpr, _)| ViewInfo {
                id: *id,
                label: (*label).to_owned(),
                breakpoint: label.to_ascii_lowercase(),
                engine: "chromium".to_owned(),
                width: *width,
                height: 800,
                dpr: *dpr,
                url: URL.to_owned(),
                title: String::new(),
            })
            .collect())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(Some(1))
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.evaluated.push((view, script.to_owned()));
        VIEWS
            .iter()
            .find(|(id, ..)| *id == view)
            .map(|(.., answer)| (*answer).to_owned())
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn cdp_send(&mut self, _view: ViewId, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Cdp.denied())
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

/// One line per finding: the view, the rule's message key below the plugin, the image and its
/// parameters.
fn line(finding: &Finding) -> String {
    let label = VIEWS
        .iter()
        .find(|(id, ..)| Some(*id) == finding.view)
        .map_or("?", |(_, label, ..)| *label);
    let message = finding
        .message
        .strip_prefix("plugin.responsive-images.findings.")
        .unwrap_or(&finding.message);
    let params: Vec<String> = finding
        .params
        .iter()
        .filter(|(key, _)| key != "image")
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    format!(
        "{label} {message} {} {}",
        finding.selector.as_deref().unwrap_or("-"),
        params.join(" ")
    )
}

fn run() -> (Vec<Finding>, Fixture) {
    let mut host = Fixture::default();
    let ctx = CheckContext {
        view: None,
        url: None,
        rules: Vec::new(),
        args: "{}".to_owned(),
    };
    let findings = ResponsiveImages.run(&mut host, &ctx).expect("run");
    (findings, host)
}

#[test]
fn each_view_is_asked_at_its_own_density() {
    let (_, host) = run();
    for (view, dpr) in [(1, "3.0"), (2, "2.0"), (3, "1.0")] {
        let (_, script) = host
            .evaluated
            .iter()
            .find(|(id, _)| *id == view)
            .expect("evaluated");
        assert!(script.contains(&format!("\"dpr\":{dpr}")), "view {view}");
    }
}

#[test]
fn every_breakpoint_has_exactly_the_four_problems_each_with_its_density() {
    let (findings, _) = run();
    for (view, _, _, dpr, _) in VIEWS {
        let mine: Vec<&Finding> = findings
            .iter()
            .filter(|finding| finding.view == Some(view))
            .collect();
        let rules: Vec<&str> = mine
            .iter()
            .map(|finding| finding.message.as_str())
            .collect();
        assert_eq!(
            rules,
            [
                "plugin.responsive-images.findings.lcp.lazy",
                "plugin.responsive-images.findings.oversized",
                "plugin.responsive-images.findings.undersized",
                "plugin.responsive-images.findings.sizes-missing.img",
            ],
            "view {view}"
        );
        for finding in mine {
            let density = finding
                .params
                .iter()
                .find(|(key, _)| key == "dpr")
                .map(|(_, value)| value.as_str());
            assert_eq!(density, Some(format!("{dpr}").as_str()), "{finding:?}");
        }
    }
    // The correctly sized image has no finding in any view, also at DPR 3.
    assert!(
        findings
            .iter()
            .all(|finding| finding.selector.as_deref() != Some("#right"))
    );
}

#[test]
fn the_findings_match_the_snapshot() {
    let (findings, _) = run();
    let mut text: String = findings
        .iter()
        .map(|finding| line(finding) + "\n")
        .collect();
    if text.is_empty() {
        text.push('\n');
    }
    if std::env::var_os("RESPONSIVE_IMAGES_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(SNAPSHOT, &text).expect("write the snapshot");
    }
    let expected = std::fs::read_to_string(SNAPSHOT).expect("the snapshot");
    assert_eq!(
        text, expected,
        "RESPONSIVE_IMAGES_UPDATE_SNAPSHOTS=1 writes it"
    );
}
