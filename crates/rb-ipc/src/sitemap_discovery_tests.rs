use serde_json::json;

use super::*;
use crate::{CoreToShell, ShellToCore, parse_request};

#[test]
fn the_start_page_asks_to_discover_or_to_close() {
    for (op, text) in [
        (SitemapStartOp::Discover, "discover"),
        (SitemapStartOp::Close, "close"),
    ] {
        let request = format!(r#"{{ "type": "sitemapStart", "op": "{text}" }}"#);
        let message = parse_request(&request).expect("parses");
        assert_eq!(
            message,
            ShellToCore::SitemapStart(SitemapStartRequest { op })
        );
        assert_eq!(
            serde_json::to_value(&message).expect("writes"),
            json!({ "type": "sitemapStart", "op": text })
        );
    }
    assert!(parse_request(r#"{ "type": "sitemapStart", "op": "capture" }"#).is_err());
}

#[test]
fn the_discovery_carries_its_pages_and_tree_flat_beside_its_type() {
    let leaf = |name: &str, first| SitemapTreeNode {
        name: name.to_owned(),
        first,
        count: 1,
        children: Vec::new(),
    };
    let answer = CoreToShell::SitemapDiscovery(SitemapDiscovery {
        discovery_id: "d1".to_owned(),
        source: "https://a.test/".to_owned(),
        site: Some("https://a.test".to_owned()),
        pages: vec!["https://a.test/".to_owned(), "https://a.test/de".to_owned()],
        tree: vec![SitemapTreeNode {
            name: "https://a.test".to_owned(),
            first: 0,
            count: 2,
            children: vec![leaf("/", 0), leaf("de", 1)],
        }],
        truncated: false,
        max_pages: 100_000,
        views: SitemapViewCounts { all: 4, visible: 2 },
    });
    let value = serde_json::to_value(&answer).expect("writes");
    assert_eq!(value["type"], "sitemapDiscovery");
    assert_eq!(value["discoveryId"], "d1");
    assert_eq!(
        value["tree"][0]["children"][1],
        json!({ "name": "de", "first": 1, "count": 1 })
    );
    assert_eq!(value["views"], json!({ "all": 4, "visible": 2 }));
    let back: CoreToShell = serde_json::from_value(value).expect("reads");
    assert_eq!(back, answer);
}
