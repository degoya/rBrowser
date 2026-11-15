use rb_plugin_api::Severity;

use super::*;
use crate::fake::fixture;

fn mobile() -> Report {
    parse(&fixture("lhr-mobile.json")).expect("the stored LHR reads")
}

#[test]
fn reads_version_urls_emulation_and_scores() {
    let report = mobile();
    assert_eq!(report.lighthouse_version, "13.6.0");
    assert_eq!(report.requested_url, "http://localhost:5173/");
    assert_eq!(report.form_factor.as_deref(), Some("mobile"));
    assert_eq!(report.width, Some(375));
    assert_eq!(report.runtime_error, None);
    assert_eq!(report.scores["performance"], Some(100));
    assert_eq!(report.scores["accessibility"], Some(62));
    assert_eq!(report.scores["best-practices"], Some(96));
    assert_eq!(report.scores["seo"], Some(75));
}

#[test]
fn lists_the_scored_audits_below_the_pass_mark() {
    let report = mobile();
    let ids: Vec<&str> = report
        .failed
        .iter()
        .map(|audit| audit.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "charset",
            "color-contrast",
            "html-has-lang",
            "image-alt",
            "landmark-one-main",
            "meta-description",
            "robots-txt"
        ]
    );
}

#[test]
fn keeps_the_elements_and_the_help_link_of_an_audit() {
    let report = mobile();
    let image_alt = report
        .failed
        .iter()
        .find(|audit| audit.id == "image-alt")
        .expect("image-alt failed");
    assert_eq!(image_alt.categories, ["accessibility", "seo"]);
    assert!(image_alt.weight > 0.0);
    assert_eq!(
        image_alt.help_url.as_deref(),
        Some("https://dequeuniversity.com/rules/axe/4.14/image-alt")
    );
    assert_eq!(image_alt.elements.len(), 1);
    let element = &image_alt.elements[0];
    assert_eq!(element.selector.as_deref(), Some("body > img"));
    assert_eq!(element.path.as_deref(), Some("1,HTML,1,BODY,1,IMG"));
}

#[test]
fn makes_one_finding_per_element_and_one_per_audit_without() {
    let report = mobile();
    let findings = findings(&report, 3, "http://localhost:5173/", &|_| true);
    let contrast = findings
        .iter()
        .find(|finding| finding.rule == "color-contrast")
        .expect("a contrast finding");
    assert_eq!(contrast.view, Some(3));
    assert_eq!(contrast.url.as_deref(), Some("http://localhost:5173/"));
    assert_eq!(contrast.selector.as_deref(), Some("body > p"));
    assert_eq!(contrast.message, "plugin.lighthouse.findings.element");
    assert_eq!(contrast.severity, Severity::Error);
    assert!(
        contrast
            .params
            .contains(&("element".to_owned(), "low contrast".to_owned()))
    );
    let data: serde_json::Value =
        serde_json::from_str(contrast.data.as_deref().expect("data")).expect("data is JSON");
    assert_eq!(data["score"], 0.0);
    assert!(
        data["explanation"]
            .as_str()
            .expect("an explanation")
            .contains("contrast")
    );
    let robots = findings
        .iter()
        .find(|finding| finding.rule == "robots-txt")
        .expect("a robots.txt finding");
    assert_eq!(robots.selector, None);
    assert_eq!(robots.message, "plugin.lighthouse.findings.audit_value");
    assert!(
        robots
            .params
            .contains(&("value".to_owned(), "2 errors found".to_owned()))
    );
    let charset = findings
        .iter()
        .find(|finding| finding.rule == "charset")
        .expect("a charset finding");
    assert_eq!(charset.message, "plugin.lighthouse.findings.audit");
}

#[test]
fn leaves_out_the_rules_switched_off() {
    let report = mobile();
    let only = findings(&report, 1, "http://localhost:5173/", &|rule| {
        rule == "image-alt"
    });
    assert_eq!(only.len(), 1);
    assert_eq!(only[0].rule, "image-alt");
}

#[test]
fn severity_follows_score_and_weight() {
    assert_eq!(severity(0.0, 10.0), Severity::Error);
    assert_eq!(severity(0.7, 25.0), Severity::Warning);
    assert_eq!(severity(0.0, 0.0), Severity::Warning);
    assert_eq!(severity(0.5, 0.0), Severity::Info);
}

#[test]
fn caps_the_elements_of_an_audit() {
    let items: Vec<serde_json::Value> = (0..25)
        .map(|i| json!({ "node": { "type": "node", "selector": format!("li:nth-child({i})"), "path": format!("1,HTML,1,BODY,{i},LI") } }))
        .collect();
    let lhr = json!({
        "lighthouseVersion": "13.6.0",
        "categories": { "accessibility": { "score": 0.5, "auditRefs": [{ "id": "listitem", "weight": 7 }] } },
        "audits": { "listitem": {
            "id": "listitem", "title": "List items are not in lists", "score": 0,
            "scoreDisplayMode": "binary", "details": { "type": "table", "items": items } } }
    });
    let report = parse(&lhr.to_string()).expect("reads");
    assert_eq!(report.failed[0].elements.len(), MAX_ELEMENTS);
    assert_eq!(report.failed[0].more_elements, 5);
}

#[test]
fn reads_a_run_that_could_not_measure_the_page() {
    let lhr = json!({
        "lighthouseVersion": "13.6.0",
        "requestedUrl": "http://localhost:9/",
        "runtimeError": { "code": "ERRORED_DOCUMENT_REQUEST", "message": "Lighthouse was unable to reliably load the page" },
        "categories": { "performance": { "score": null, "auditRefs": [{ "id": "first-contentful-paint", "weight": 10 }] } },
        "audits": { "first-contentful-paint": { "id": "first-contentful-paint", "title": "FCP", "score": null, "scoreDisplayMode": "error" } }
    });
    let report = parse(&lhr.to_string()).expect("reads");
    assert_eq!(
        report.runtime_error.as_deref(),
        Some("ERRORED_DOCUMENT_REQUEST")
    );
    assert_eq!(report.scores["performance"], None);
    assert!(report.failed.is_empty());
}

#[test]
fn refuses_what_is_no_lhr() {
    assert!(parse("[]").is_err());
    assert!(parse("{").is_err());
}
