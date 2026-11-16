use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;

fn page(path: &str) -> Url {
    Url::parse(&format!("https://shop.test{path}")).expect("url")
}

#[test]
fn robots_is_read_for_the_search_engine_and_the_crawl() {
    let mut host = MemoryHost::new().grant(Capability::Http);
    host.responses.insert(
        "https://shop.test/robots.txt".to_owned(),
        HttpResponse {
            status: 200,
            headers: Vec::new(),
            body: b"User-agent: *\nDisallow: /private\nSitemap: /sitemap.xml\n".to_vec(),
        },
    );
    let site = Site::load(&mut host, &page("/de/start"));
    assert_eq!(site.origin, "https://shop.test");
    assert_eq!(site.robots_status, Some(200));
    assert!(!site.engine.allows(&page("/private/x")));
    assert!(!site.crawler.allows(&page("/private/x")));
    assert!(site.engine.allows(&page("/de/")));
    assert_eq!(
        site.sitemaps()[0].to_string(),
        "https://shop.test/sitemap.xml"
    );
}

#[test]
fn a_missing_robots_allows_everything() {
    let mut host = MemoryHost::new().grant(Capability::Http);
    let site = Site::load(&mut host, &page("/"));
    assert_eq!(site.robots_status, Some(404));
    assert!(site.engine.allows(&page("/private")));
    assert!(site.sitemaps().is_empty());
}
