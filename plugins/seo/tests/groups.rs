//! The acceptance fixtures of RB-080-11 (`tests/fixtures/site/seo/{social,wellknown,microdata,
//! head}-{ok,broken}/`): each page served at `https://fixture.test/<folder>/` on the fixture
//! site (`tests/common`), checked as `checks_run` checks it; a broken fixture's findings held to
//! `tests/snapshots/<folder>.txt`, an ok fixture without any finding. `social-ok`'s preview, as
//! `seo_social_preview` answers it, is `tests/snapshots/social-ok.preview.json` - the panel's
//! Vitest renders its six cards from that file.

mod common;

use std::path::PathBuf;

use common::{NOW, ORIGIN, answer, check_page, fixture, html, lines, png, site, snapshot};
use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Finding, Severity};
use seo_plugin::Seo;
use seo_plugin::social::{self, cut};
use serde_json::Value;

/// `folder`'s page on the fixture site, `files` (paths below the folder) served at the root.
fn host(folder: &str, files: &[&str]) -> (MemoryHost, String) {
    let page = fixture(&format!("{folder}/index.html"));
    let mut host = site(folder, html(page.clone(), &[]));
    for file in files {
        let target = match *file {
            "security.txt" => ".well-known/security.txt",
            other => other,
        };
        host.responses.insert(
            format!("{ORIGIN}/{target}"),
            answer(
                "text/plain",
                fixture(&format!("{folder}/{file}")).into_bytes(),
            ),
        );
    }
    (host, page)
}

fn run(folder: &str, files: &[&str]) -> Vec<Finding> {
    let (mut host, page) = host(folder, files);
    check_page(&mut host, folder, &page)
}

fn quiet(folder: &str, findings: &[Finding]) {
    assert!(
        findings
            .iter()
            .all(|finding| finding.severity != Severity::Error),
        "{folder}: {}",
        lines(findings)
    );
    assert!(findings.is_empty(), "{folder}: {}", lines(findings));
}

#[test]
fn the_ok_fixtures_have_no_finding() {
    for (folder, files) in [
        ("social-ok", &[][..]),
        ("wellknown-ok", &["llms.txt", "security.txt"][..]),
        ("microdata-ok", &[][..]),
        ("head-ok", &[][..]),
    ] {
        quiet(folder, &run(folder, files));
    }
}

#[test]
fn social_broken_finds_the_relative_small_image_and_the_missing_card() {
    let (mut host, page) = host("social-broken", &[]);
    host.responses.insert(
        format!("{ORIGIN}/img/small.png"),
        answer("image/png", png(600, 300)),
    );
    let findings = check_page(&mut host, "social-broken", &page);
    snapshot("social-broken", &findings);
    let rules: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
    assert_eq!(
        rules,
        [
            "social.image_relative",
            "social.twitter_card",
            "social.image_small"
        ]
    );
}

#[test]
fn wellknown_broken_finds_the_missing_h1_contact_and_the_past_expiry() {
    let findings = run("wellknown-broken", &["llms.txt", "security.txt"]);
    snapshot("wellknown-broken", &findings);
    let messages: Vec<&str> = findings.iter().map(|f| f.message.as_str()).collect();
    assert_eq!(
        messages,
        [
            "plugin.seo.findings.wellknown.llms_no_h1",
            "plugin.seo.findings.wellknown.security_contact",
            "plugin.seo.findings.wellknown.security_expired",
        ]
    );
    assert_eq!(
        findings[1].url.as_deref(),
        Some("https://fixture.test/.well-known/security.txt")
    );
}

#[test]
fn microdata_broken_finds_the_nameless_product_and_the_contradiction() {
    let findings = run("microdata-broken", &[]);
    snapshot("microdata-broken", &findings);
    let rules: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
    assert_eq!(
        rules,
        ["microdata.required_missing", "microdata.jsonld_conflict"]
    );
    assert!(
        findings[1]
            .params
            .contains(&("theirs".to_owned(), "B-2".to_owned()))
    );
}

#[test]
fn head_broken_finds_the_late_charset_and_the_late_title() {
    let findings = run("head-broken", &[]);
    snapshot("head-broken", &findings);
    let rules: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
    assert_eq!(rules, ["head.charset_late", "head.title_late"]);
}

#[test]
fn social_ok_previews_the_default_platforms_cut_by_the_table() {
    let (mut host, _) = host("social-ok", &[]);
    host.now = NOW;
    let answer: Value = serde_json::from_str(
        &Seo.call_tool(&mut host, "social_preview", r#"{"target": 1}"#)
            .expect("preview"),
    )
    .expect("json");
    let cards = answer["cards"].as_array().expect("cards");
    let table = social::data();
    let defaults: Vec<_> = table.platforms.iter().filter(|p| p.default).collect();
    assert_eq!(cards.len(), defaults.len());
    for (card, platform) in cards.iter().zip(defaults) {
        assert_eq!(card["id"], platform.id.as_str());
        let title = &card["title"];
        let full = title["full"].as_str().expect("full title");
        assert_eq!(title["text"], cut(full, platform.title).0.as_str());
        if platform.no_description {
            assert_eq!(card["description"], Value::Null);
        } else {
            let full = card["description"]["full"].as_str().expect("description");
            assert_eq!(
                card["description"]["text"],
                cut(full, platform.description).0.as_str()
            );
        }
        assert_eq!(card["image"], "https://fixture.test/img/share.jpg");
        // RB-080-99: the layout follows the rule with the image's real size (1200 x 630).
        assert_eq!(card["unsure"], false, "{}", platform.id);
        assert_ne!(card["reason"]["code"], "size_unknown", "{}", platform.id);
    }
    let truncated = |id: &str, field: &str| {
        cards.iter().find(|card| card["id"] == id).expect("card")[field]["truncated"] == true
    };
    assert!(truncated("whatsapp", "title") && truncated("facebook", "description"));
    assert!(!truncated("discord", "description") && !truncated("x", "title"));
    assert_eq!(answer["images"][0]["width"], 1200);
    let file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/social-ok.preview.json");
    let pretty = serde_json::to_string_pretty(&answer).expect("json") + "\n";
    if std::env::var_os("RB_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&file, &pretty).expect("snapshot written");
        return;
    }
    let expected = std::fs::read_to_string(&file).unwrap_or_default();
    assert_eq!(pretty, expected, "snapshot social-ok.preview.json");
}

#[test]
fn the_wellknown_command_runs_only_its_group_and_loads_no_page() {
    let (mut host, _) = host("wellknown-broken", &["llms.txt", "security.txt"]);
    let answer: Value = serde_json::from_str(
        &Seo.run_command(&mut host, "check_wellknown", "{}")
            .expect("command"),
    )
    .expect("json");
    assert_eq!(answer["summary"]["total"], 3);
    assert_eq!(answer["pages"], 0);
    assert!(
        host.fetched
            .iter()
            .all(|request| !request.url.ends_with("/wellknown-broken/"))
    );
}
