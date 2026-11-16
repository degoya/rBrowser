use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse, ViewInfo};
use serde_json::json;

use super::*;

const ORIGIN: &str = "https://shop.test";

fn html(body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: vec![("content-type".to_owned(), "text/html".to_owned())],
        body: format!(
            "<html lang=en><head><title>Same title</title></head><body>{body}</body></html>"
        )
        .into_bytes(),
    }
}

/// A site of 20 pages, each linking to the next three, a private page and another origin.
fn site() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Http)
        .grant(Capability::Evaluate)
        .grant(Capability::Views);
    host.responses.insert(
        format!("{ORIGIN}/robots.txt"),
        HttpResponse {
            status: 200,
            headers: Vec::new(),
            body: b"User-agent: *\nDisallow: /private\nSitemap: /sitemap.xml\n".to_vec(),
        },
    );
    host.responses.insert(
        format!("{ORIGIN}/sitemap.xml"),
        HttpResponse {
            status: 200,
            ..HttpResponse::default()
        },
    );
    for page in 0..20 {
        let links: String = (1..=3)
            .map(|step| format!("<a href=\"/p{}\">n</a>", page + step))
            .collect();
        let body =
            format!("{links}<a href=\"/private/x\">p</a><a href=\"https://other.test/\">o</a>");
        let path = if page == 0 {
            "/".to_owned()
        } else {
            format!("/p{page}")
        };
        host.responses
            .insert(format!("{ORIGIN}{path}"), html(&body));
    }
    host
}

fn pages_fetched(host: &MemoryHost) -> Vec<String> {
    host.fetched
        .iter()
        .map(|request| request.url.clone())
        .filter(|url| !url.ends_with("robots.txt") && !url.ends_with("sitemap.xml"))
        .filter(|url| !url.ends_with("llms.txt") && !url.ends_with("security.txt"))
        .collect()
}

/// The origin's files of RB-080-11, fetched once per run.
fn files_fetched(host: &MemoryHost) -> usize {
    host.fetched
        .iter()
        .filter(|request| {
            request.url.ends_with("llms.txt") || request.url.ends_with("security.txt")
        })
        .count()
}

#[test]
fn a_page_run_checks_the_page_and_the_site_once() {
    let mut host = site();
    let request = Request {
        url: Some(format!("{ORIGIN}/")),
        ..Request::default()
    };
    let outcome = run(&mut host, &Config::default(), &request).expect("run");
    assert_eq!(outcome.pages.len(), 1);
    assert_eq!(pages_fetched(&host), [format!("{ORIGIN}/")]);
    assert!(!outcome.rendered);
    assert!(
        outcome
            .findings
            .iter()
            .all(|f| f.rule != "meta.title_duplicate")
    );
}

#[test]
fn a_crawl_stays_within_its_pages_its_origin_and_robots() {
    let mut host = site();
    let config = Config {
        max_pages: 10,
        ..Config::default()
    };
    let request = Request {
        url: Some(format!("{ORIGIN}/")),
        scope: Scope::Crawl,
        ..Request::default()
    };
    let outcome = run(&mut host, &config, &request).expect("run");
    let fetched = pages_fetched(&host);
    assert_eq!(outcome.pages.len(), 10);
    assert_eq!(fetched.len(), 10, "{fetched:?}");
    assert_eq!(
        files_fetched(&host),
        3,
        "llms.txt and both places of security.txt"
    );
    assert!(fetched.iter().all(|url| url.starts_with(ORIGIN)));
    assert!(fetched.iter().all(|url| !url.contains("/private")));
    assert_eq!(host.progress.last(), Some(&(10, 10)));
    let duplicates = outcome
        .findings
        .iter()
        .filter(|finding| finding.rule == "meta.title_duplicate")
        .count();
    assert_eq!(duplicates, 10, "every page shares its title");
}

#[test]
fn without_a_url_the_views_page_is_checked_with_its_rendered_dom() {
    let mut host = site();
    host.views.push(ViewInfo {
        id: 4,
        url: format!("{ORIGIN}/"),
        ..ViewInfo::default()
    });
    host.evaluate_answer = json!({
        "url": format!("{ORIGIN}/"),
        "html": "<html><head><title>Same title</title></head><body></body></html>",
    })
    .to_string();
    let request = Request {
        view: Some(4),
        ..Request::default()
    };
    let outcome = run(&mut host, &Config::default(), &request).expect("run");
    assert_eq!(outcome.url, format!("{ORIGIN}/"));
    assert!(outcome.rendered);
    assert!(outcome.findings.iter().any(|f| f.view == Some(4)));
}

#[test]
fn no_url_and_a_bad_url_are_refused_with_a_code() {
    let mut host = MemoryHost::new();
    let failure = run(&mut host, &Config::default(), &Request::default()).expect_err("no url");
    assert_eq!(failure.code(), Some("plugin.seo.errors.no_url"));
    let request = Request {
        url: Some("file:///etc/passwd".to_owned()),
        ..Request::default()
    };
    let failure = run(&mut host, &Config::default(), &request).expect_err("bad url");
    assert_eq!(failure.code(), Some("plugin.seo.errors.bad_url"));
}

#[test]
fn a_run_of_the_wellknown_group_alone_loads_no_page() {
    let mut host = site();
    let request = Request {
        url: Some(format!("{ORIGIN}/")),
        rules: crate::rules::of_groups(&["wellknown".to_owned()]),
        ..Request::default()
    };
    let outcome = run(&mut host, &Config::default(), &request).expect("run");
    assert!(outcome.pages.is_empty());
    assert!(pages_fetched(&host).is_empty());
    assert_eq!(files_fetched(&host), 3);
    let rules: Vec<&str> = outcome.findings.iter().map(|f| f.rule.as_str()).collect();
    assert_eq!(
        rules,
        ["wellknown.llms_missing", "wellknown.security_missing"]
    );
}
