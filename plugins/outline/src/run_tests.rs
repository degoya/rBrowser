use serde_json::json;

use super::*;
use crate::fake::FakeHost;

const URL: &str = "https://site.test/";

fn located(path: &str, visible: bool) -> Value {
    json!({ "selector": path, "path": path, "visible": visible, "hidden": !visible })
}

fn page(links: &[(&str, &str, bool)]) -> Value {
    let links: Vec<Value> = links
        .iter()
        .enumerate()
        .map(|(at, (href, text, visible))| {
            let mut link = located(&format!("a:nth-of-type({})", at + 1), *visible);
            link["href"] = json!(href);
            link["text"] = json!(text);
            link
        })
        .collect();
    let mut main = located("main", true);
    main["role"] = json!("main");
    json!({ "url": URL, "links": links, "landmarks": [main] })
}

/// Two Chromium views on the same page: the desktop shows a link the mobile view hides.
fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", URL)
        .view(2, "Mobile", URL)
        .page(
            1,
            &page(&[
                ("https://site.test/ok", "Ok", true),
                ("https://site.test/gone", "Old", true),
            ]),
        )
        .page(
            2,
            &page(&[
                ("https://site.test/ok", "Ok", true),
                ("https://site.test/desk", "Desk", false),
            ]),
        )
        .respond("HEAD", "https://site.test/ok", 200)
        .respond("HEAD", "https://site.test/desk", 200);
    host.settings
        .insert("linkCheck.rps".to_owned(), "4".to_owned());
    host
}

fn ctx(view: Option<ViewId>) -> CheckContext {
    CheckContext {
        view,
        url: Some(URL.to_owned()),
        rules: Vec::new(),
        args: String::new(),
    }
}

#[test]
fn the_first_view_on_a_page_checks_the_links_of_every_view_on_it_once() {
    let mut host = host();
    let found = check(&mut host, &ctx(Some(1))).expect("findings");
    let broken: Vec<_> = found
        .iter()
        .filter(|f| f.rule == rules::LINK_BROKEN)
        .collect();
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].selector.as_deref(), Some("a:nth-of-type(2)"));
    let mut urls: Vec<&str> = host.fetched.iter().map(|(_, url)| url.as_str()).collect();
    urls.sort_unstable();
    assert_eq!(
        urls,
        [
            "https://site.test/desk",
            "https://site.test/gone",
            "https://site.test/ok"
        ]
    );

    let mut second = self::host();
    let found = check(&mut second, &ctx(Some(2))).expect("findings");
    assert!(second.fetched.is_empty(), "view 2 does not lead its page");
    assert!(found.iter().all(|f| f.rule != rules::LINK_BROKEN));
}

#[test]
fn without_a_view_every_view_is_checked() {
    let mut host = host();
    let found = check(&mut host, &ctx(None)).expect("findings");
    assert_eq!(
        found
            .iter()
            .filter(|f| f.rule == rules::LINK_BROKEN)
            .count(),
        1
    );
    assert_eq!(host.fetched.len(), 3, "each link once");
}

#[test]
fn the_link_check_switched_off_requests_nothing() {
    let mut host = host();
    host.settings
        .insert("linkCheck.enabled".to_owned(), "false".to_owned());
    check(&mut host, &ctx(Some(1))).expect("findings");
    assert!(host.fetched.is_empty());
}

#[test]
fn a_view_without_the_collector_gives_no_findings_but_a_named_one_is_skipped_for_its_engine() {
    let mut host = host();
    host.view(3, "Firefox", URL);
    host.views[2].engine = "firefox".to_owned();
    assert!(check(&mut host, &ctx(None)).is_ok());
    let failure = check(&mut host, &ctx(Some(3))).expect_err("skipped");
    assert_eq!(failure.unserved_engine_in(3), Some("firefox"));
    let failure = check(&mut host, &ctx(Some(9))).expect_err("no view");
    assert_eq!(failure.code(), Some(codes::NOT_FOUND));
}

#[test]
fn targets_resolve_all_one_or_refuse() {
    let mut host = host();
    assert_eq!(targets(&mut host, &Value::Null).expect("all").len(), 2);
    assert_eq!(targets(&mut host, &json!("all")).expect("all").len(), 2);
    assert_eq!(targets(&mut host, &json!(2)).expect("one")[0].id, 2);
    assert_eq!(
        targets(&mut host, &json!(7)).expect_err("none").code(),
        Some(codes::NOT_FOUND)
    );
    assert_eq!(
        targets(&mut host, &json!("active"))
            .expect_err("bad")
            .code(),
        Some(codes::INVALID)
    );
}

#[test]
fn a_view_run_finds_what_its_width_lacks_against_the_other_views_on_the_page() {
    let mut host = host();
    let mut nav = located("nav", true);
    nav["role"] = json!("navigation");
    nav["name"] = json!("Main");
    let mut main = located("main", true);
    main["role"] = json!("main");
    let mut hidden_nav = nav.clone();
    hidden_nav["visible"] = json!(false);
    hidden_nav["hidden"] = json!(true);
    host.page(1, &json!({ "url": URL, "landmarks": [nav, main.clone()] }))
        .page(2, &json!({ "url": URL, "landmarks": [hidden_nav, main] }))
        .view(3, "Other page", "https://site.test/other")
        .page(
            3,
            &json!({ "url": "https://site.test/other", "landmarks": [] }),
        );
    let rule = |found: &[Finding]| -> Vec<(Option<ViewId>, Option<String>)> {
        found
            .iter()
            .filter(|f| f.rule == rules::LANDMARK_DIFFERS)
            .map(|f| (f.view, f.path.clone()))
            .collect()
    };
    assert_eq!(rule(&check(&mut host, &ctx(Some(1))).expect("1")), []);
    assert_eq!(
        rule(&check(&mut host, &ctx(Some(2))).expect("2")),
        [(Some(2), Some("nav".to_owned()))]
    );
    assert_eq!(
        rule(&check(&mut host, &ctx(Some(3))).expect("3")),
        [],
        "another page"
    );
    assert_eq!(
        rule(&check(&mut host, &ctx(None)).expect("all")),
        [(Some(2), Some("nav".to_owned()))]
    );
}

#[test]
fn the_breakpoint_rules_switched_off_read_no_other_view() {
    let mut host = host();
    let ctx = CheckContext {
        rules: vec![rules::LINK_EMPTY.to_owned()],
        ..ctx(Some(2))
    };
    host.settings
        .insert("linkCheck.enabled".to_owned(), "false".to_owned());
    check(&mut host, &ctx).expect("findings");
    assert!(host.evaluated.iter().all(|(view, _)| *view == 2));
}

#[test]
fn the_compared_elements_follow_outline_diff_elements() {
    let mut host = host();
    let link = |visible: bool| {
        let mut link = located("nav > a", visible);
        link["tag"] = json!("a");
        link["role"] = json!("link");
        link["name"] = json!("Pricing");
        link
    };
    host.page(1, &json!({ "url": URL, "focus": [link(true)] }))
        .page(2, &json!({ "url": URL, "focus": [link(false)] }));
    let interactive = |found: &[Finding]| -> Vec<Option<ViewId>> {
        found
            .iter()
            .filter(|f| f.rule == rules::INTERACTIVE_DIFFERS)
            .map(|f| f.view)
            .collect()
    };
    assert_eq!(
        interactive(&check(&mut host, &ctx(Some(2))).expect("2")),
        [Some(2)]
    );
    host.settings
        .insert("diff.elements".to_owned(), r#"["headings"]"#.to_owned());
    assert_eq!(
        interactive(&check(&mut host, &ctx(Some(2))).expect("2")),
        []
    );
    host.settings
        .insert("diff.elements".to_owned(), "[]".to_owned());
    host.evaluated.clear();
    check(&mut host, &ctx(Some(2))).expect("2");
    assert!(
        host.evaluated.iter().all(|(view, _)| *view == 2),
        "nothing compared: no other view read"
    );
}
