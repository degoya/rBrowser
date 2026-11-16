use serde_json::json;

use super::*;
use crate::fake::FakeHost;

const URL: &str = "https://site.test/";

fn located(path: &str, visible: bool) -> Value {
    json!({ "selector": path, "path": path, "visible": visible, "hidden": !visible })
}

fn outline(nav_visible: bool) -> Value {
    let mut nav = located("nav", nav_visible);
    nav["role"] = json!("navigation");
    nav["name"] = json!("Main");
    let mut h2 = located("h2", true);
    h2["level"] = json!(2);
    h2["text"] = json!("Intro");
    let mut h4 = located("h4", true);
    h4["level"] = json!(4);
    h4["text"] = json!("Details");
    let mut link = located("a", nav_visible);
    link["href"] = json!("https://site.test/docs");
    link["text"] = json!("Docs");
    json!({ "url": URL, "headings": [h2, h4], "landmarks": [nav], "links": [link] })
}

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", URL)
        .view(2, "Mobile", URL)
        .page(1, &outline(true))
        .page(2, &outline(false));
    host.cdp.insert(
        "Accessibility.getFullAXTree".to_owned(),
        Ok(json!({ "nodes": [{ "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Page" } }] })),
    );
    host
}

#[test]
fn get_merges_the_sections_and_marks_what_only_some_views_show() {
    let mut host = host();
    let answer = get(&mut host, &json!({ "sections": ["landmarks", "headings"] })).expect("answer");
    assert_eq!(answer["views"][0]["label"], "Desktop");
    let nav = &answer["sections"]["landmarks"]["rows"][0];
    assert_eq!(nav["item"]["name"], "Main");
    assert_eq!(nav["only"], json!([1]));
    assert!(answer["sections"].get("links").is_none());
    assert!(answer["sections"].get("axtree").is_none());
    let rules: Vec<&str> = answer["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|f| f["rule"].as_str())
        .collect();
    assert!(rules.contains(&"heading-skip"));
    assert!(!rules.contains(&"link-empty"), "links were not asked for");
    assert!(host.fetched.is_empty(), "get checks no links");
}

#[test]
fn get_finds_what_a_width_lacks_and_answers_the_findings_alone() {
    let mut host = host();
    let answer = get(&mut host, &json!({ "sections": ["landmarks"] })).expect("answer");
    let differs: Vec<&Value> = answer["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["rule"] == "landmark-differs")
        .collect();
    assert_eq!(differs.len(), 1);
    assert_eq!(differs[0]["view"], 2);
    assert_eq!(differs[0]["severity"], "info");
    assert_eq!(differs[0]["params"]["shown"], "Desktop");

    let alone = get(&mut host, &json!({ "findingsOnly": true })).expect("answer");
    assert!(alone.get("sections").is_none());
    let rules: Vec<&str> = alone["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|f| f["rule"].as_str())
        .collect();
    for rule in ["heading-skip", "h1-missing", "landmark-differs"] {
        assert!(rules.contains(&rule), "{rule}");
    }
    assert!(
        host.cdp_sent.is_empty(),
        "no accessibility tree for the findings alone"
    );
    assert_eq!(
        get(&mut host, &json!({ "findingsOnly": "yes" }))
            .expect_err("invalid")
            .code(),
        Some(codes::INVALID)
    );
}

#[test]
fn get_without_sections_reads_everything_the_tree_included() {
    let mut host = host();
    let answer = get(&mut host, &json!({})).expect("answer");
    for name in SECTIONS {
        assert!(answer["sections"].get(name).is_some(), "{name}");
    }
    assert_eq!(
        answer["sections"]["axtree"]["views"]["1"]["nodes"][0]["name"],
        "Page"
    );
}

#[test]
fn get_per_view_and_the_tree_alone() {
    let mut host = host();
    let answer = get(
        &mut host,
        &json!({ "shape": "views", "sections": ["links"], "target": 2 }),
    )
    .expect("answer");
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        answer["views"][0]["outline"]["links"][0]["href"],
        "https://site.test/docs"
    );
    assert!(answer.get("sections").is_none());

    let mut host = self::host();
    let answer = get(&mut host, &json!({ "sections": ["axtree"] })).expect("answer");
    assert!(host.evaluated.is_empty(), "the tree needs no collector");
    assert!(answer["sections"]["axtree"]["views"]["2"].is_object());
}

#[test]
fn get_names_a_view_without_an_outline_and_refuses_bad_arguments() {
    let mut host = host();
    host.view(3, "Firefox", URL);
    let answer = get(&mut host, &json!({ "sections": ["links"] })).expect("answer");
    assert_eq!(answer["views"][2]["error"], codes::NOT_FOUND);
    for bad in [
        json!({ "sections": ["nonsense"] }),
        json!({ "sections": "links" }),
        json!({ "shape": "tree" }),
        json!({ "budget": 10 }),
        json!({ "target": "active" }),
    ] {
        assert_eq!(
            get(&mut host, &bad).expect_err("refused").code(),
            Some(codes::INVALID),
            "{bad}"
        );
    }
}

#[test]
fn get_with_a_budget_fits_it() {
    let mut host = FakeHost::default();
    let links: Vec<Value> = (0..400)
        .map(|n| {
            let mut link = located(&format!("a:nth-of-type({n})"), true);
            link["href"] = json!(format!("https://site.test/page/{n}"));
            link["text"] = json!(format!("Page number {n}"));
            link
        })
        .collect();
    host.view(1, "Desktop", URL)
        .page(1, &json!({ "url": URL, "links": links }));
    let answer = get(&mut host, &json!({ "sections": ["links"], "budget": 8192 })).expect("answer");
    assert!(answer.to_string().len() <= 8192);
    assert_eq!(answer["sections"]["links"]["cut"], true);
}

#[test]
fn check_links_checks_only_links_on_the_page() {
    let mut host = host();
    host.settings
        .insert("linkCheck.rps".to_owned(), "4".to_owned());
    host.respond("HEAD", "https://site.test/docs", 404);
    let answer = check_links(
        &mut host,
        &json!({ "urls": ["https://site.test/docs#top", "http://169.254.169.254/latest"] }),
    )
    .expect("answer");
    assert_eq!(answer["results"][0]["state"], "broken");
    assert_eq!(answer["results"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        answer["notOnPage"],
        json!(["http://169.254.169.254/latest"])
    );
    assert_eq!(host.fetched.len(), 1);
}

#[test]
fn check_links_is_off_with_the_setting() {
    let mut host = host();
    host.settings
        .insert("linkCheck.enabled".to_owned(), "false".to_owned());
    let answer = check_links(&mut host, &json!({})).expect("answer");
    assert_eq!(answer["enabled"], false);
    assert!(host.fetched.is_empty());
}

#[test]
fn mark_marks_per_view_and_names_unknown_views() {
    let mut host = host();
    host.cdp.insert(
        "DOM.getDocument".to_owned(),
        Ok(json!({ "root": { "nodeId": 1 } })),
    );
    host.cdp
        .insert("DOM.querySelector".to_owned(), Ok(json!({ "nodeId": 5 })));
    let answer = mark(
        &mut host,
        &json!({ "targets": [{ "view": 1, "selector": "nav" }, { "view": 9, "selector": "nav" }] }),
    )
    .expect("answer");
    assert_eq!(answer["marked"][0], json!({ "view": 1, "found": true }));
    assert_eq!(answer["marked"][1]["error"], codes::NOT_FOUND);
    let cleared = mark(&mut host, &json!({ "clear": true })).expect("cleared");
    assert_eq!(cleared["cleared"], true);
    assert_eq!(host.methods().last(), Some(&"Overlay.hideHighlight"));
}

#[test]
fn the_check_command_reports_its_findings() {
    let mut host = host();
    host.settings
        .insert("linkCheck.rps".to_owned(), "4".to_owned());
    host.respond("HEAD", "https://site.test/docs", 404);
    let answer = check_command(&mut host).expect("answer");
    // Found where the page's lead view shows it; the mobile view hides the link.
    assert_eq!(answer["brokenLinks"], 1);
    assert_eq!(
        host.findings.len(),
        answer["findings"].as_u64().map_or(0, |n| n as usize)
    );
}

#[test]
fn arguments_must_be_an_object() {
    assert_eq!(parse("").expect("empty"), json!({}));
    assert_eq!(
        parse("[1]").expect_err("array").code(),
        Some(codes::INVALID)
    );
    assert_eq!(parse("{").expect_err("broken").code(), Some(codes::INVALID));
}
