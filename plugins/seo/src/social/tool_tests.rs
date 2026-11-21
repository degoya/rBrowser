use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse, ViewInfo};

use super::*;
use crate::social::image::tests::png;

const PAGE: &str = "https://shop.test/p/";

fn host(head: &str) -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Http)
        .grant(Capability::Views);
    host.responses.insert(
        PAGE.to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "text/html".to_owned())],
            body: format!("<html><head>{head}</head><body></body></html>").into_bytes(),
        },
    );
    host.views.push(ViewInfo {
        id: 4,
        url: PAGE.to_owned(),
        ..ViewInfo::default()
    });
    host.active = Some(4);
    host
}

fn answer(host: &mut MemoryHost, args: &str) -> Value {
    serde_json::from_str(&preview(host, args).expect("preview")).expect("json")
}

#[test]
fn the_view_s_page_gives_the_default_cards_and_its_image_inline() {
    let mut host = host(
        r#"<title>T</title><meta property="og:image" content="/s.png">
<meta property="og:url" content="https://shop.test/p/">"#,
    );
    host.responses.insert(
        "https://shop.test/s.png".to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "image/png".to_owned())],
            body: png(1200, 630),
        },
    );
    let answer = answer(&mut host, r#"{"target": 4}"#);
    assert_eq!(answer["cards"].as_array().map(Vec::len), Some(7));
    assert_eq!(answer["domain"], "shop.test");
    assert_eq!(answer["dataVersion"], 2);
    // RB-080-99: the size rules settle with the image's real size, the card rules on twitter:card.
    let card = |id: &str| {
        answer["cards"]
            .as_array()
            .and_then(|cards| cards.iter().find(|card| card["id"] == id))
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(card("facebook")["layout"], "large");
    assert_eq!(card("facebook")["reason"]["code"], "meets");
    assert_eq!(card("facebook")["unsure"], false);
    assert_eq!(card("x")["layout"], "small");
    assert_eq!(card("x")["reason"]["code"], "card_missing");
    let image = &answer["images"][0];
    assert_eq!(
        (image["width"].as_u64(), image["height"].as_u64()),
        (Some(1200), Some(630))
    );
    assert!(
        image["dataUri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("data:image/png;base64,"))
    );
}

#[test]
fn an_image_over_the_budget_is_described_but_not_inlined() {
    let mut host = host(r#"<meta property="og:image" content="https://shop.test/big.png">"#);
    let mut big = png(2400, 1260);
    big.resize(ANSWER_BUDGET, 0);
    host.responses.insert(
        "https://shop.test/big.png".to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "image/png".to_owned())],
            body: big,
        },
    );
    let answer = answer(&mut host, "{}");
    let image = &answer["images"][0];
    assert_eq!(image["dataUri"], Value::Null);
    assert_eq!(image["tooLarge"], true);
    assert!(answer.to_string().len() < ANSWER_BUDGET);
}

/// RB-090-44: the owner's "The share image is too large to show here (169951 bytes)". The panel
/// asks for a `budget` as large as its message limit, so a share image of 170 KB is inlined; an
/// agent (MCP, no budget) keeps the small default.
#[test]
fn a_share_image_of_170_kb_is_inlined_within_the_panel_s_budget() {
    let mut host = host(r#"<meta property="og:image" content="https://shop.test/big.png">"#);
    let mut big = png(1200, 630);
    big.resize(169_951, 0);
    host.responses.insert(
        "https://shop.test/big.png".to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "image/png".to_owned())],
            body: big,
        },
    );
    let panel = answer(&mut host, r#"{"budget": 4188768}"#);
    let image = &panel["images"][0];
    assert_eq!(image["tooLarge"], false);
    assert_eq!(image["bytes"], 169_951);
    assert!(
        image["dataUri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("data:image/png;base64,")),
        "{image}"
    );
    assert!(panel.to_string().len() <= 4_188_768);
    let agent = answer(&mut host, "{}");
    assert_eq!(agent["images"][0]["tooLarge"], true);
    assert_eq!(agent["images"][0]["dataUri"], Value::Null);
}

#[test]
fn a_budget_outside_its_range_is_refused() {
    let mut host = host("");
    for budget in ["1", "4095", "4194305", "\"big\"", "-1"] {
        let refused =
            preview(&mut host, &format!(r#"{{"budget": {budget}}}"#)).expect_err("out of range");
        assert_eq!(
            refused.code(),
            Some("plugin.seo.errors.bad_args"),
            "{budget}"
        );
        assert_eq!(refused.param("reason"), Some("budget"), "{budget}");
    }
}

#[test]
fn the_platforms_setting_narrows_the_cards() {
    let mut host = host("<title>T</title>");
    host.settings.insert(
        "social.platforms".to_owned(),
        r#"["x", "slack"]"#.to_owned(),
    );
    let answer = answer(&mut host, r#"{"url": "https://shop.test/p/"}"#);
    let ids: Vec<&str> = answer["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .filter_map(|card| card["id"].as_str())
        .collect();
    assert_eq!(ids, ["x", "slack"]);
    assert_eq!(answer["images"], json!([]));
}

#[test]
fn bad_arguments_and_pages_are_refused() {
    let mut host = host("");
    let code = |result: Result<String, Failure>| {
        result
            .err()
            .and_then(|failure| failure.code().map(str::to_owned))
    };
    assert_eq!(
        code(preview(&mut host, r#"{"nope": 1}"#)).as_deref(),
        Some("plugin.seo.errors.bad_args")
    );
    assert_eq!(
        code(preview(&mut host, r#"{"url": "file:///etc/passwd"}"#)).as_deref(),
        Some("plugin.seo.errors.bad_url")
    );
    host.active = None;
    assert_eq!(
        code(preview(&mut host, "{}")).as_deref(),
        Some("plugin.seo.errors.no_url")
    );
}

#[test]
fn an_image_that_does_not_load_leaves_a_size_rule_unsure() {
    let mut host = host(r#"<meta property="og:image" content="https://shop.test/gone.png">"#);
    let answer = answer(&mut host, r#"{"target": 4}"#);
    let facebook = &answer["cards"][0];
    assert_eq!(facebook["id"], "facebook");
    assert_eq!(facebook["unsure"], true);
    assert_eq!(facebook["reason"]["code"], "size_unknown");
    assert_eq!(
        (&facebook["layout"], &facebook["alternate"]),
        (&json!("large"), &json!("small"))
    );
}

/// RB-090-95: the search result of the page beside the cards - its texts, breadcrumb, `noindex`,
/// the limits per device and the favicon inline, fetched once.
#[test]
fn the_answer_has_the_search_result_with_its_favicon_inline() {
    let mut host = host(
        r#"<title>Red shoes</title><meta name="description" content="Light and red.">
<meta name="robots" content="noindex"><link rel="icon" href="/fav.png">"#,
    );
    host.responses.insert(
        "https://shop.test/fav.png".to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "image/png".to_owned())],
            body: png(32, 32),
        },
    );
    let answer = answer(&mut host, r#"{"target": 4}"#);
    let serp = &answer["serp"];
    assert_eq!(serp["title"]["text"], "Red shoes");
    assert_eq!(serp["description"]["source"], "description");
    assert_eq!(serp["siteName"]["text"], "shop.test");
    assert_eq!(
        serp["breadcrumb"],
        json!({ "host": "shop.test", "segments": ["p"] })
    );
    assert_eq!(serp["noindex"], true);
    assert_eq!(serp["limits"]["desktop"]["width"], 600);
    assert_eq!(serp["favicon"]["source"], "icon");
    assert!(
        serp["favicon"]["dataUri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("data:image/png;base64,"))
    );
    let favicons = host
        .fetched
        .iter()
        .filter(|request| request.url.contains("fav"))
        .count();
    assert_eq!(favicons, 1);
}

/// The favicon comes before the share images in the budget: small, and shown by both results.
#[test]
fn the_favicon_is_inlined_first_and_a_share_image_over_the_rest_is_not() {
    let mut host = host(r#"<meta property="og:image" content="https://shop.test/big.png">"#);
    let mut big = png(1200, 630);
    big.resize(ANSWER_BUDGET / 2, 0);
    host.responses.insert(
        "https://shop.test/big.png".to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "image/png".to_owned())],
            body: big,
        },
    );
    let mut icon = png(48, 48);
    icon.resize(ANSWER_BUDGET / 4, 0);
    host.responses.insert(
        "https://shop.test/favicon.ico".to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "image/png".to_owned())],
            body: icon,
        },
    );
    let answer = answer(&mut host, "{}");
    assert_eq!(answer["serp"]["favicon"]["source"], "default");
    assert!(answer["serp"]["favicon"]["dataUri"].is_string());
    assert_eq!(answer["images"][0]["tooLarge"], true);
    assert!(answer.to_string().len() <= ANSWER_BUDGET);
}

#[test]
fn a_page_that_does_not_load_still_has_a_search_result_without_texts() {
    let mut host = host("");
    host.responses.remove(PAGE);
    let answer = answer(&mut host, r#"{"target": 4}"#);
    let serp = &answer["serp"];
    assert_eq!(serp["title"], Value::Null);
    assert_eq!(serp["description"], Value::Null);
    assert_eq!(serp["url"], PAGE);
    assert_eq!(serp["favicon"]["status"], 404);
}
