//! What the fixture tests share: the fixture site `https://fixture.test` served through a
//! `MemoryHost` - its `robots.txt` and sitemap, its `llms.txt` and `security.txt`, its share
//! image (1200 x 630) - the page of a folder, a check run as `checks_run` runs it, and the
//! findings held to a snapshot in `tests/snapshots/` (`RB_UPDATE_SNAPSHOTS=1 cargo test` writes
//! them anew; review the diff).

#![allow(dead_code, reason = "each test crate uses a part of the helpers")]

use std::fmt::Write as _;
use std::path::PathBuf;

use rb_plugin_api::check::Check;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, CheckContext, Finding, HttpResponse, ViewInfo};
use seo_plugin::{Seo, i18n};
use serde_json::json;

pub const ORIGIN: &str = "https://fixture.test";
/// The fixtures' clock: 2026-11-12T00:00:00Z (`security.txt` expires 2027-06-30).
pub const NOW: u64 = 1_794_441_600;

pub fn fixture(path: &str) -> String {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/site/seo")
        .join(path);
    std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()))
}

/// An answer of type `kind`.
pub fn answer(kind: &str, body: Vec<u8>) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: vec![("content-type".to_owned(), kind.to_owned())],
        body,
    }
}

pub fn html(body: String, headers: &[(&str, &str)]) -> HttpResponse {
    let mut page = answer("text/html; charset=utf-8", body.into_bytes());
    page.headers.extend(
        headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
    );
    page
}

/// A PNG header of `width` x `height` (signature and IHDR; enough to sniff).
pub fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

/// A JPEG header of `width` x `height`: SOI, an APP0 segment, SOF0.
pub fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08,
    ];
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes
}

/// The fixture site with the page at `folder`, shown by view 1.
pub fn site(folder: &str, page: HttpResponse) -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Http)
        .grant(Capability::Views)
        .grant(Capability::Evaluate);
    host.now = NOW;
    let text = |path: &str| answer("text/plain", fixture(path).into_bytes());
    host.responses
        .insert(format!("{ORIGIN}/robots.txt"), text("clean/robots.txt"));
    host.responses.insert(
        format!("{ORIGIN}/sitemap.xml"),
        HttpResponse {
            status: 200,
            ..HttpResponse::default()
        },
    );
    host.responses
        .insert(format!("{ORIGIN}/llms.txt"), text("clean/llms.txt"));
    host.responses.insert(
        format!("{ORIGIN}/.well-known/security.txt"),
        text("clean/security.txt"),
    );
    host.responses.insert(
        format!("{ORIGIN}/img/share.jpg"),
        answer("image/jpeg", jpeg(1200, 630)),
    );
    host.responses.insert(
        format!("{ORIGIN}/clean/"),
        html(fixture("clean/index.html"), &[]),
    );
    host.responses.insert(format!("{ORIGIN}/{folder}/"), page);
    host.views.push(ViewInfo {
        id: 1,
        url: format!("{ORIGIN}/{folder}/"),
        ..ViewInfo::default()
    });
    host.active = Some(1);
    host
}

/// What `evaluate` answers for a view showing `html` at `url`.
pub fn rendered(url: &str, html: &str, backgrounds: serde_json::Value) -> String {
    json!({ "url": url, "html": html, "truncated": false, "images": [], "backgrounds": backgrounds })
        .to_string()
}

/// The page at `folder` checked as `checks_run` checks it, its rendered DOM its raw HTML.
pub fn check_page(host: &mut MemoryHost, folder: &str, page: &str) -> Vec<Finding> {
    host.evaluate_answer = rendered(&format!("{ORIGIN}/{folder}/"), page, json!([]));
    check(host, folder)
}

pub fn check(host: &mut MemoryHost, folder: &str) -> Vec<Finding> {
    let ctx = CheckContext {
        view: Some(1),
        url: Some(format!("{ORIGIN}/{folder}/")),
        rules: Vec::new(),
        args: "{}".to_owned(),
    };
    Seo.run(host, &ctx).expect("the check runs")
}

/// One line per finding: severity, rule, message and element path.
pub fn lines(findings: &[Finding]) -> String {
    let mut out = String::new();
    for finding in findings {
        let _ = writeln!(
            out,
            "{} {} {} {}",
            finding.severity.as_str(),
            finding.rule,
            finding
                .message
                .strip_prefix("plugin.seo.findings.")
                .unwrap_or(&finding.message),
            finding.path.as_deref().unwrap_or("-"),
        );
    }
    out
}

/// `findings` held to `tests/snapshots/<name>.txt`, each with its English text.
pub fn snapshot(name: &str, findings: &[Finding]) {
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
