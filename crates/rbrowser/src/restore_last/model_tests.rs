use rb_config::{ProjectConfig, StartupRestore};
use rb_engine::ViewId;
use serde_json::{Value, json};

use super::*;

fn project(value: Value) -> ProjectConfig {
    let mut value = value;
    value["version"] = json!(1);
    serde_json::from_value(value).expect("a config")
}

fn three() -> ProjectConfig {
    project(json!({ "environments": [
        { "id": "local", "url": "http://localhost:5173" },
        { "id": "staging", "url": "https://staging.example.com/" },
        { "id": "production", "url": "https://www.example.com" }
    ] }))
}

fn view<'a>(breakpoint: &'a str, url: &'a str, decoupled: bool) -> ViewSnapshot<'a> {
    ViewSnapshot {
        cell_breakpoint: Some(breakpoint),
        url,
        decoupled,
    }
}

fn page(breakpoint: &str, index: u32, path: Option<&str>, url: Option<&str>) -> LastPage {
    LastPage {
        breakpoint: breakpoint.to_owned(),
        index,
        path: path.map(str::to_owned),
        url: url.map(str::to_owned),
        decoupled: false,
    }
}

#[test]
fn a_page_under_the_environment_in_effect_is_kept_as_its_path_another_as_its_url() {
    let config = three();
    let pages = capture(
        &[
            view(
                "mobile",
                "https://staging.example.com/shop?token=SECRET123",
                false,
            ),
            view("desktop", "https://foreign.test/x", true),
            view("tablet", "about:blank", false),
        ],
        &config,
        Some("staging"),
    );
    assert_eq!(pages.version, FORMAT_VERSION);
    assert_eq!(
        pages.views,
        vec![
            page("mobile", 0, Some("/shop?token=SECRET123"), None),
            LastPage {
                decoupled: true,
                ..page("desktop", 0, None, Some("https://foreign.test/x"))
            },
        ],
        "a page that is no web page keeps nothing of a coupled view"
    );
}

#[test]
fn a_breakpoint_shown_twice_counts_its_views_in_grid_order() {
    let config = three();
    let pages = capture(
        &[
            view("mobile", "http://localhost:5173/a", false),
            view("desktop", "http://localhost:5173/b", false),
            view("mobile", "http://localhost:5173/c", false),
        ],
        &config,
        Some("local"),
    );
    let keys: Vec<(&str, u32, Option<&str>)> = pages
        .views
        .iter()
        .map(|page| (page.breakpoint.as_str(), page.index, page.path.as_deref()))
        .collect();
    assert_eq!(
        keys,
        [
            ("mobile", 0, Some("/a")),
            ("desktop", 0, Some("/b")),
            ("mobile", 1, Some("/c")),
        ]
    );
}

#[test]
fn credentials_in_the_url_an_overlong_url_and_a_view_without_a_cell_keep_nothing() {
    let config = three();
    let long = format!("https://foreign.test/{}", "a".repeat(MAX_URL_LEN));
    let pages = capture(
        &[
            view("mobile", "https://user:secret@foreign.test/", false),
            view("tablet", &long, false),
            ViewSnapshot {
                cell_breakpoint: None,
                url: "https://foreign.test/",
                decoupled: false,
            },
        ],
        &config,
        None,
    );
    assert!(pages.views.is_empty(), "{:?}", pages.views);
}

#[test]
fn a_decoupled_view_without_a_web_page_stays_decoupled() {
    let pages = capture(&[view("mobile", "about:blank", true)], &three(), None);
    assert_eq!(
        pages.views,
        vec![LastPage {
            decoupled: true,
            ..page("mobile", 0, None, None)
        }]
    );
    let restores = resolve(&pages, &[(ViewId(1), Some("mobile"))], &three(), None);
    assert_eq!(
        restores,
        vec![Restore {
            view: ViewId(1),
            url: None,
            decoupled: true,
        }]
    );
}

#[test]
fn a_path_opens_in_the_environment_in_effect_at_the_start() {
    let config = three();
    let pages = capture(
        &[view(
            "mobile",
            "https://staging.example.com/shop/cart",
            false,
        )],
        &config,
        Some("staging"),
    );
    let restores = resolve(
        &pages,
        &[(ViewId(4), Some("mobile"))],
        &config,
        Some("production"),
    );
    assert_eq!(
        restores,
        vec![Restore {
            view: ViewId(4),
            url: Some("https://www.example.com/shop/cart".to_owned()),
            decoupled: false,
        }]
    );
}

#[test]
fn the_entries_of_views_that_are_gone_are_dropped() {
    let config = three();
    let pages = LastPages {
        version: FORMAT_VERSION,
        views: vec![
            page("mobile", 0, Some("/a"), None),
            page("mobile", 1, Some("/b"), None),
            page("removed", 0, Some("/c"), None),
        ],
    };
    let restores = resolve(&pages, &[(ViewId(1), Some("mobile"))], &config, None);
    assert_eq!(
        restores,
        vec![Restore {
            view: ViewId(1),
            url: Some("http://localhost:5173/a".to_owned()),
            decoupled: false,
        }],
        "the second mobile view and the removed breakpoint have no view"
    );
}

#[test]
fn a_path_without_a_base_is_dropped() {
    let config = project(json!({}));
    let pages = LastPages {
        version: FORMAT_VERSION,
        views: vec![
            page("mobile", 0, Some("/a"), None),
            page("desktop", 0, None, Some("https://foreign.test/")),
        ],
    };
    let restores = resolve(
        &pages,
        &[(ViewId(1), Some("mobile")), (ViewId(2), Some("desktop"))],
        &config,
        None,
    );
    assert_eq!(
        restores,
        vec![Restore {
            view: ViewId(2),
            url: Some("https://foreign.test/".to_owned()),
            decoupled: false,
        }]
    );
}

#[test]
fn only_last_without_a_url_on_the_command_line_restores_at_the_start() {
    assert!(restores_at_start(StartupRestore::Last, false));
    assert!(
        !restores_at_start(StartupRestore::Last, true),
        "the command line wins"
    );
    assert!(!restores_at_start(StartupRestore::StartUrl, false));
    assert!(!restores_at_start(StartupRestore::StartUrl, true));
}

#[test]
fn four_views_come_back_through_the_file_with_the_decoupled_one_decoupled() {
    let config = three();
    let pages = capture(
        &[
            view("mobile", "http://localhost:5173/one", false),
            view("tablet", "http://localhost:5173/two?q=1#top", false),
            view("laptop", "https://foreign.test/three", false),
            view("desktop", "http://localhost:5173/four", true),
        ],
        &config,
        Some("local"),
    );
    let text = rb_config::to_json_string(&to_value(&pages));
    let read = parse(&text).expect("the written file reads back");
    assert_eq!(read, pages);
    let views = [
        (ViewId(1), Some("mobile")),
        (ViewId(2), Some("tablet")),
        (ViewId(3), Some("laptop")),
        (ViewId(4), Some("desktop")),
    ];
    let restores = resolve(&read, &views, &config, Some("local"));
    let urls: Vec<(ViewId, Option<&str>, bool)> = restores
        .iter()
        .map(|restore| (restore.view, restore.url.as_deref(), restore.decoupled))
        .collect();
    assert_eq!(
        urls,
        [
            (ViewId(1), Some("http://localhost:5173/one"), false),
            (ViewId(2), Some("http://localhost:5173/two?q=1#top"), false),
            (ViewId(3), Some("https://foreign.test/three"), false),
            (ViewId(4), Some("http://localhost:5173/four"), true),
        ]
    );
}

#[test]
fn a_file_of_another_version_or_not_json_is_no_last_session() {
    assert_eq!(parse("not json"), None);
    assert_eq!(parse(r#"{ "version": 99, "views": [] }"#), None);
    assert_eq!(parse(r#"[1, 2]"#), None);
}

#[test]
fn entries_the_check_refuses_are_dropped_when_read() {
    let text = json!({
        "version": FORMAT_VERSION,
        "views": [
            { "breakpoint": "a", "index": 0, "path": "//evil.test/x" },
            { "breakpoint": "b", "index": 0, "url": "javascript:alert(1)" },
            { "breakpoint": "c", "index": 0, "path": "/x", "url": "https://foreign.test/" },
            { "breakpoint": "", "index": 0, "path": "/x" },
            { "breakpoint": "d", "index": 0 },
            { "breakpoint": "e", "index": 0, "url": "https://user:pw@foreign.test/" },
            { "breakpoint": "ok", "index": 0, "path": "/fine" }
        ]
    })
    .to_string();
    let read = parse(&text).expect("the file is readable");
    assert_eq!(read.views, vec![page("ok", 0, Some("/fine"), None)]);
}

#[test]
fn no_more_entries_than_the_cap_are_kept() {
    let urls: Vec<String> = (0..MAX_ENTRIES + 5)
        .map(|n| format!("https://foreign.test/{n}"))
        .collect();
    let views: Vec<ViewSnapshot<'_>> = urls.iter().map(|url| view("m", url, false)).collect();
    assert_eq!(capture(&views, &three(), None).views.len(), MAX_ENTRIES);
    let many = LastPages {
        version: FORMAT_VERSION,
        views: (0..MAX_ENTRIES + 5)
            .map(|n| page("m", u32::try_from(n).expect("small"), Some("/x"), None))
            .collect(),
    };
    let text = rb_config::to_json_string(&to_value(&many));
    assert_eq!(parse(&text).expect("readable").views.len(), MAX_ENTRIES);
}

#[test]
fn a_view_without_a_web_page_now_keeps_its_last_good_entry() {
    let config = three();
    let before = LastPages {
        version: FORMAT_VERSION,
        views: vec![
            page("mobile", 0, Some("/kept"), None),
            page("tablet", 0, None, Some("https://foreign.test/kept")),
            page("laptop", 0, Some("/replaced"), None),
            page("removed", 0, Some("/gone"), None),
        ],
    };
    let pages = capture_over(
        &[
            view("mobile", "about:blank", false),
            view("tablet", "", true),
            view("laptop", "http://localhost:5173/new", false),
            view("desktop", "chrome-error://chromewebdata/", false),
        ],
        &config,
        Some("local"),
        Some(&before),
    );
    assert_eq!(
        pages.views,
        vec![
            page("mobile", 0, Some("/kept"), None),
            LastPage {
                decoupled: true,
                ..page("tablet", 0, None, Some("https://foreign.test/kept"))
            },
            page("laptop", 0, Some("/new"), None),
        ],
        "the coupling is today's; a view with nothing before keeps nothing; a gone view's entry \
         goes"
    );
}

#[test]
fn without_entries_before_capture_over_is_capture() {
    let views = [view("mobile", "about:blank", true)];
    assert_eq!(
        capture_over(&views, &three(), None, None),
        capture(&views, &three(), None)
    );
}
