use rb_config::{Breakpoint, EngineKind, ProjectConfig};
use rb_ipc::SitemapViewCounts;
use serde_json::json;

use super::*;

fn found(paths: &[&str]) -> Found {
    Found {
        source: "https://a.test/".to_owned(),
        site: Some("https://a.test".to_owned()),
        rewrite_from: Some("https://a.test".to_owned()),
        urls: paths
            .iter()
            .map(|path| format!("https://a.test{path}"))
            .collect(),
        truncated: false,
    }
}

fn discovered(paths: &[&str]) -> Discovered {
    found(paths).into_discovered("d1".to_owned(), SitemapViewCounts::default())
}

fn urls(paths: &[&str]) -> Vec<String> {
    paths
        .iter()
        .map(|path| format!("https://a.test{path}"))
        .collect()
}

#[test]
fn a_discovery_answers_its_pages_in_the_tree_order() {
    let kept = discovered(&["/b", "/a/1", "/a"]);
    assert_eq!(kept.answer.discovery_id, "d1");
    assert_eq!(kept.answer.pages, urls(&["/a", "/a/1", "/b"]));
    assert_eq!(kept.answer.tree[0].count, 3);
    assert_eq!(kept.answer.max_pages, MAX_PAGES);
    assert!(!kept.answer.truncated);
}

#[test]
fn a_discovery_keeps_at_most_max_pages() {
    let many = Found {
        urls: (0..=MAX_PAGES)
            .map(|n| format!("https://a.test/{n}"))
            .collect(),
        ..found(&[])
    };
    let kept = many.into_discovered("d".to_owned(), SitemapViewCounts::default());
    assert_eq!(kept.answer.pages.len(), MAX_PAGES);
    assert!(kept.answer.truncated);
}

#[test]
fn a_chosen_subtree_comes_back_in_the_discovery_order_once() {
    let kept = discovered(&["/", "/de", "/de/x", "/de/y", "/en"]);
    let de = &kept.answer.tree[0].children[1];
    assert_eq!(de.name, "de");
    let subtree = kept.answer.pages[de.first..de.first + de.count].to_vec();
    let mut wanted = subtree.clone();
    wanted.reverse();
    wanted.push(subtree[0].clone());
    assert_eq!(chosen(&kept, &wanted).expect("chosen"), subtree);
}

#[test]
fn a_url_the_discovery_did_not_find_is_refused() {
    let kept = discovered(&["/", "/de"]);
    let refused = chosen(&kept, &urls(&["/de", "/admin"])).expect_err("foreign");
    assert_eq!(refused.code(), "sitemap.error.foreign_url");
    assert!(refused.to_string().contains("https://a.test/admin"));
    let other_host = vec!["https://evil.test/de".to_owned()];
    assert!(matches!(
        chosen(&kept, &other_host),
        Err(RunError::ForeignUrl(_))
    ));
}

#[test]
fn no_url_and_too_many_are_refused() {
    let kept = discovered(&["/"]);
    assert!(matches!(chosen(&kept, &[]), Err(RunError::NoPages)));
    let many: Vec<String> = (0..=MAX_PAGES)
        .map(|n| format!("https://a.test/{n}"))
        .collect();
    let refused = chosen(&kept, &many).expect_err("too many");
    assert_eq!(refused.code(), "sitemap.error.too_many_urls");
}

#[test]
fn the_kept_discoveries_are_bounded() {
    let discoveries = Discoveries::default();
    let root = std::path::Path::new("/work/shop");
    let ids: Vec<String> = (0..=KEPT).map(|_| discoveries.next_id(7)).collect();
    assert_eq!(ids[0], "7-1");
    for id in &ids {
        let made = found(&["/"]).into_discovered(id.clone(), SitemapViewCounts::default());
        discoveries.keep(root, made);
    }
    assert!(
        discoveries.get(&ids[0], root).is_none(),
        "the oldest is let go"
    );
    for id in &ids[1..] {
        assert!(discoveries.get(id, root).is_some(), "{id} is kept");
    }
    assert!(discoveries.get("unknown", root).is_none());
}

#[test]
fn a_discovery_serves_only_the_project_it_was_made_for() {
    let discoveries = Discoveries::default();
    let (shop, blog) = (
        std::path::Path::new("/work/shop"),
        std::path::Path::new("/work/blog"),
    );
    let id = discoveries.next_id(7);
    discoveries.keep(
        shop,
        found(&["/"]).into_discovered(id.clone(), SitemapViewCounts::default()),
    );
    assert!(discoveries.get(&id, shop).is_some());
    assert!(
        discoveries.get(&id, blog).is_none(),
        "another project's discovery is unknown"
    );
}

#[test]
fn the_run_takes_exactly_the_pages_and_their_rewrite() {
    let pages = urls(&["/de"]);
    assert_eq!(
        run_request(pages.clone(), None),
        json!({
            "source": { "urls": ["https://a.test/de"] },
            "limits": { "maxUrls": 1 },
            "selection": {}
        })
    );
    let rewrite = json!({ "from": "https://a.test", "to": "http://127.0.0.1:8080" });
    let moved = run_request(pages, Some(rewrite.clone()));
    assert_eq!(moved["rewrite"], rewrite);
}

fn breakpoint(id: &str, engine: EngineKind) -> Breakpoint {
    serde_json::from_value(json!({
        "id": id,
        "width": 800,
        "height": 600,
        "engine": serde_json::to_value(engine).expect("engine"),
    }))
    .expect("a breakpoint")
}

#[test]
fn the_view_counts_take_the_chromium_breakpoints_of_each_choice() {
    let mut config = ProjectConfig {
        breakpoints: vec![
            breakpoint("mobile", EngineKind::Chromium),
            breakpoint("tablet", EngineKind::Chromium),
            breakpoint("desktop", EngineKind::Chromium),
            breakpoint("fox", EngineKind::Firefox),
        ],
        ..ProjectConfig::default()
    };
    config.layout.hidden = vec!["tablet".to_owned()];
    assert_eq!(
        view_counts(&config),
        SitemapViewCounts { all: 3, visible: 2 }
    );
    assert_eq!(
        view_counts(&ProjectConfig::default()),
        SitemapViewCounts::default()
    );
}

#[test]
fn a_url_list_gives_each_url_once_without_comments() {
    let urls =
        parse_url_list("# chosen\nhttps://a.test/de\n\n  https://a.test/en  \nhttps://a.test/de\n")
            .expect("urls");
    assert_eq!(urls, ["https://a.test/de", "https://a.test/en"]);
    assert!(parse_url_list("# nothing\n\n").is_err());
}

#[test]
fn a_url_list_takes_max_pages_and_no_more() {
    let list = |n: usize| -> String { (0..n).map(|at| format!("https://a.test/{at}\n")).collect() };
    let full = list(MAX_PAGES);
    assert!(u64::try_from(full.len()).expect("fits") <= MAX_URLS_FILE_BYTES);
    assert_eq!(
        parse_url_list(&full).expect("at the limit").len(),
        MAX_PAGES
    );
    assert!(parse_url_list(&list(MAX_PAGES + 1)).is_err());
}

#[test]
fn a_large_discovery_goes_to_a_file_and_its_reply_stays_small() {
    let root = tempfile::tempdir().expect("temp");
    let many = Found {
        urls: (0..10_000)
            .map(|n| format!("https://a.test/section-{}/page-{n}", n % 40))
            .collect(),
        ..found(&[])
    };
    let kept = many.into_discovered("1795000000-7".to_owned(), SitemapViewCounts::default());
    let answer_bytes = serde_json::to_vec(&kept.answer).expect("json").len();
    assert!(
        answer_bytes > crate::instance::protocol::MAX_REPLY,
        "{answer_bytes}"
    );
    let reply = write_discovery(root.path(), &kept.answer).expect("written");
    let file = std::path::PathBuf::from(&reply.file);
    assert!(file.starts_with(root.path().join(".rbrowser/out/sitemap-discover")));
    assert_eq!(reply.pages, 10_000);
    let reply_bytes = serde_json::to_vec(&reply).expect("json").len();
    assert!(
        reply_bytes < crate::instance::protocol::MAX_REPLY,
        "{reply_bytes}"
    );
    let back: rb_ipc::SitemapDiscovery =
        serde_json::from_slice(&std::fs::read(file).expect("read")).expect("parses");
    assert_eq!(back, kept.answer);
}

#[test]
fn a_discovery_file_is_only_named_by_a_plain_id() {
    let root = tempfile::tempdir().expect("temp");
    let mut answer = discovered(&["/"]).answer;
    answer.discovery_id = "../escape".to_owned();
    assert!(write_discovery(root.path(), &answer).is_err());
}
