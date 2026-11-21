//! The fixture of the export tests: a run of two plugins over two views with five known
//! findings, each in one breakpoint, one of them with markup in its parameters.

use std::collections::BTreeMap;

use rb_plugin_types::{
    CheckTarget, CheckedView, ElementRef, FINDINGS_VERSION, FindingMessage, FindingsFile,
    FindingsSummary, MergedFinding, Severity,
};

use super::ExportContext;

/// The page every finding is on.
pub const PAGE: &str = "http://localhost:5173/shop?q=1&x=<2>";

/// Markup a finding's parameter carries, which no export may let through.
pub const MARKUP: &str = "<script>alert(\"x\")</script>";

fn finding(
    rule: &str,
    severity: Severity,
    view: u32,
    breakpoint: &str,
    selector: Option<&str>,
) -> MergedFinding {
    let plugin = rule.split('.').next().unwrap_or(rule).to_owned();
    MergedFinding {
        id: format!("id-{rule}"),
        rule_id: rule.to_owned(),
        plugins: vec![plugin.clone()],
        severity,
        message: FindingMessage {
            key: format!("plugin.{plugin}.findings.{rule}"),
            params: BTreeMap::new(),
        },
        views: vec![view],
        breakpoints: vec![breakpoint.to_owned()],
        url: Some(PAGE.to_owned()),
        element: selector.map(|selector| ElementRef {
            selector: Some(selector.to_owned()),
            path: Some(format!("html>body>{selector}")),
        }),
        help_url: None,
        data: None,
        shared_id: None,
    }
}

/// Five findings: three in `mobile` (view 1), two in `desktop` (view 2).
pub fn fixture() -> FindingsFile {
    let mut meta = finding("seo.meta", Severity::Warning, 1, "mobile", None);
    meta.message
        .params
        .insert("name".to_owned(), MARKUP.to_owned());
    let mut images = finding(
        "seo.images",
        Severity::Warning,
        2,
        "desktop",
        Some("img.hero"),
    );
    images.help_url = Some("https://help.test/alt?a=1&b=2".to_owned());
    let mut alt = finding("a11y.alt", Severity::Info, 2, "desktop", Some("img.logo"));
    alt.help_url = Some("javascript:alert(1)".to_owned());
    let findings = vec![
        finding(
            "seo.title",
            Severity::Error,
            1,
            "mobile",
            Some("head>title"),
        ),
        images,
        finding(
            "a11y.contrast",
            Severity::Error,
            1,
            "mobile",
            Some("p.lead"),
        ),
        alt,
        meta,
    ];
    FindingsFile {
        version: FINDINGS_VERSION,
        run_id: "run-1700000000000".to_owned(),
        started_at: 1_700_000_000,
        target: CheckTarget::All,
        url: None,
        plugins: vec!["seo".to_owned(), "a11y".to_owned()],
        views: vec![
            CheckedView {
                id: 1,
                breakpoint: Some("mobile".to_owned()),
            },
            CheckedView {
                id: 2,
                breakpoint: Some("desktop".to_owned()),
            },
        ],
        summary: FindingsSummary::of(&findings),
        findings,
        failures: Vec::new(),
        skipped: Vec::new(),
    }
}

/// The texts of the fixture's message keys.
pub fn messages() -> BTreeMap<String, String> {
    [
        ("plugin.seo.findings.seo.title", "The page has no title"),
        ("plugin.seo.findings.seo.images", "An image is too large"),
        (
            "plugin.seo.findings.seo.meta",
            "The meta tag {name} is missing",
        ),
        (
            "plugin.a11y.findings.a11y.contrast",
            "The text contrast is too low",
        ),
        ("plugin.a11y.findings.a11y.alt", "An image has no alt text"),
    ]
    .into_iter()
    .map(|(key, text)| (key.to_owned(), text.to_owned()))
    .collect()
}

/// The engines of the fixture's views.
pub fn engines() -> BTreeMap<u32, String> {
    BTreeMap::from([(1, "chromium".to_owned()), (2, "firefox".to_owned())])
}

/// A report text as its key and arguments (`count(count=3)`), so tests see which text went where.
pub fn text(key: &str, args: &[(&str, &str)]) -> String {
    if args.is_empty() {
        return key.to_owned();
    }
    let args: Vec<String> = args
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    format!("{key}({})", args.join(","))
}

/// Runs `body` with the fixture's context.
pub fn with_context<R>(body: impl FnOnce(&ExportContext<'_>) -> R) -> R {
    let (messages, engines) = (messages(), engines());
    body(&ExportContext {
        messages: &messages,
        engines: &engines,
        text: &text,
    })
}
