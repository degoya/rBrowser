use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;
use crate::config::Config;
use crate::url::Url;

fn run(robots: Option<&str>, sitemap_status: u16) -> (Vec<String>, MemoryHost) {
    let mut host = MemoryHost::new().grant(Capability::Http);
    if let Some(robots) = robots {
        host.responses.insert(
            "https://shop.test/robots.txt".to_owned(),
            HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: robots.as_bytes().to_vec(),
            },
        );
    }
    host.responses.insert(
        "https://shop.test/sitemap.xml".to_owned(),
        HttpResponse {
            status: sitemap_status,
            ..HttpResponse::default()
        },
    );
    let site = Site::load(&mut host, &Url::parse("https://shop.test/").expect("url"));
    let config = Config::default();
    let enabled = Vec::new();
    let mut sink = Sink::new(&config, &enabled);
    check(&mut host, &site, &mut sink);
    let found = sink
        .findings
        .iter()
        .map(|finding| format!("{} {}", finding.rule, finding.message))
        .collect();
    (found, host)
}

#[test]
fn a_reachable_sitemap_in_robots_is_quiet() {
    let (found, _) = run(Some("Sitemap: https://shop.test/sitemap.xml\n"), 200);
    assert!(found.is_empty());
}

#[test]
fn no_robots_no_sitemap_line_and_a_dead_sitemap_are_reported() {
    let (found, _) = run(None, 200);
    assert_eq!(
        found,
        ["site.sitemap_missing plugin.seo.findings.site.robots_missing"]
    );
    let (found, _) = run(Some("User-agent: *\nDisallow:\n"), 200);
    assert_eq!(
        found,
        ["site.sitemap_missing plugin.seo.findings.site.sitemap_missing"]
    );
    let (found, _) = run(Some("Sitemap: /sitemap.xml\n"), 404);
    assert_eq!(
        found,
        ["site.sitemap_unreachable plugin.seo.findings.site.sitemap_unreachable"]
    );
}
