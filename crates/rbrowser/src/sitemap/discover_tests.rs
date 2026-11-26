use rb_ipc::{SitemapStartOp, SitemapStartRequest, SitemapViewCounts};
use serde_json::json;

use super::*;

fn source(label: &str, from: Option<&str>) -> plan::Source {
    plan::Source {
        source: json!({ "site": label }),
        label: label.to_owned(),
        from: from.map(str::to_owned),
    }
}

#[test]
fn the_plugins_answer_becomes_the_found_pages_moved_from_the_site() {
    let answer = json!({
        "site": "https://a.test",
        "pages": [
            { "url": "https://a.test/" },
            { "url": "https://a.test/de", "sitemapUrl": "https://a.test/de" }
        ],
        "truncated": true
    });
    let found = found_of(&answer, source("https://a.test/", None), None);
    assert_eq!(found.source, "https://a.test/");
    assert_eq!(found.site.as_deref(), Some("https://a.test"));
    assert_eq!(found.rewrite_from.as_deref(), Some("https://a.test"));
    assert_eq!(found.urls, ["https://a.test/", "https://a.test/de"]);
    assert!(found.truncated);
}

#[test]
fn a_file_or_the_bookmarks_move_from_the_start_url() {
    let answer = json!({ "site": null, "pages": [{ "url": "https://a.test/x" }] });
    let file = found_of(&answer, source("pages.txt", Some("https://a.test")), None);
    assert_eq!(file.rewrite_from.as_deref(), Some("https://a.test"));
    let bookmarks = found_of(
        &answer,
        source("bookmarks", None),
        Some("https://b.test/start"),
    );
    assert_eq!(bookmarks.rewrite_from.as_deref(), Some("https://b.test"));
    assert!(
        found_of(&json!({}), source("x", None), None)
            .urls
            .is_empty()
    );
}

#[test]
fn the_answer_cuts_its_tree_and_keeps_every_page() {
    let found = Found {
        source: "s".to_owned(),
        urls: ["/a/b/c", "/a/b/d", "/e"]
            .iter()
            .map(|path| format!("https://a.test{path}"))
            .collect(),
        ..Found::default()
    }
    .into_discovered("1-1".to_owned(), SitemapViewCounts::default());
    let answer = answer_of(&found, 2);
    assert_eq!(answer.pages.len(), 3);
    assert!(answer.tree[0].children[0].children.is_empty());
    assert_eq!(answer.tree[0].children[0].count, 2);
}

#[test]
fn only_the_start_pages_messages_are_its_own() {
    let message = ShellToCore::SitemapStart(SitemapStartRequest {
        op: SitemapStartOp::Discover,
    });
    assert!(is_request(&message));
    assert!(!is_request(&ShellToCore::GetState));
}

#[test]
fn a_failure_keeps_its_code_and_masks_a_url() {
    let (code, message) = failure(&RunError::Discover(
        "plugins.error.http_failed reason=https://user:secret@a.test/".to_owned(),
    ));
    assert_eq!(code, IpcErrorCode::Internal);
    assert!(message.starts_with("sitemap.error.discover"), "{message}");
    assert!(!message.contains("secret"), "{message}");
    assert_eq!(failure(&RunError::NoPages).0, IpcErrorCode::NotFound);
}
