use serde_json::json;

use super::*;

#[test]
fn capture_args_read_and_check() {
    let args = CaptureArgs::parse(&json!({
        "url": "https://www.example.test/",
        "breakpoints": ["mobile", "desktop"],
        "env": "local",
        "max": 20,
        "compare": true
    }))
    .expect("reads");
    assert_eq!(args.breakpoints, ["mobile", "desktop"]);
    assert_eq!(args.env.as_deref(), Some("local"));
    assert!(args.compare);
    assert_eq!(CaptureArgs::parse(&json!(null)), Ok(CaptureArgs::default()));
}

#[test]
fn capture_args_out_of_bounds_are_refused() {
    let refused = |value: serde_json::Value| CaptureArgs::parse(&value).is_err();
    assert!(refused(json!({ "max": MAX_PAGES + 1 })));
    assert!(refused(json!({ "url": "" })));
    assert!(refused(
        json!({ "url": "https://a.test/", "bookmarks": true })
    ));
    assert!(refused(json!({ "breakpoints": vec!["a"; MAX_NAMES + 1] })));
    assert!(refused(json!({ "unknown": 1 })));
}

#[test]
fn run_args_take_an_optional_id() {
    assert_eq!(
        RunArgs::parse(&json!({ "runId": "2026-11-11_00-00-00" }))
            .expect("reads")
            .run_id
            .as_deref(),
        Some("2026-11-11_00-00-00")
    );
    assert_eq!(RunArgs::parse(&json!(null)), Ok(RunArgs::default()));
    assert!(RunArgs::parse(&json!({ "id": 1 })).is_err());
}

#[test]
fn the_tools_have_their_names() {
    assert_eq!(
        [CAPTURE_TOOL, STATUS_TOOL, CANCEL_TOOL, GALLERY_TOOL],
        [
            "sitemap_capture",
            "sitemap_status",
            "sitemap_cancel",
            "sitemap_gallery"
        ]
    );
    let started = serde_json::to_value(CaptureStarted {
        run_id: "r".to_owned(),
        folder: "f".to_owned(),
        pending: 76,
        pages: 19,
        breakpoints: 4,
    })
    .expect("serialises");
    assert_eq!(
        started,
        json!({ "runId": "r", "folder": "f", "pending": 76, "pages": 19, "breakpoints": 4 })
    );
}

#[test]
fn capture_args_take_the_chosen_urls_of_a_discovery() {
    let args = CaptureArgs::parse(&json!({
        "discovery": "1-1",
        "urls": ["https://a.test/de"],
        "env": "local"
    }))
    .expect("reads");
    assert_eq!(args.discovery.as_deref(), Some("1-1"));
    assert_eq!(args.urls, Some(vec!["https://a.test/de".to_owned()]));
    let rediscovered = CaptureArgs::parse(&json!({
        "url": "sitemap.txt",
        "urls": ["https://a.test/de"]
    }));
    assert!(
        rediscovered.is_ok(),
        "urls without discovery find the source again"
    );
}

#[test]
fn chosen_urls_out_of_bounds_are_refused() {
    let refused = |value: serde_json::Value| CaptureArgs::parse(&value).is_err();
    let url = "https://a.test/";
    assert!(refused(json!({ "urls": [] })));
    assert!(refused(json!({ "urls": [""] })));
    assert!(refused(json!({ "urls": vec![url; MAX_PAGES + 1] })));
    assert!(refused(json!({ "urls": [url], "resume": "r1" })));
    assert!(refused(json!({ "discovery": "1-1" })));
    assert!(refused(json!({ "discovery": "", "urls": [url] })));
    for source in [
        json!({ "url": url }),
        json!({ "bookmarks": true }),
        json!({ "max": 3 }),
        json!({ "include": ["/de/**"] }),
        json!({ "exclude": ["/en/**"] }),
    ] {
        let mut value = source.clone();
        value["discovery"] = json!("1-1");
        value["urls"] = json!([url]);
        assert!(refused(value), "{source} beside discovery");
    }
}

#[test]
fn a_url_file_names_a_project_text_file_alone() {
    let args = CaptureArgs::parse(&json!({ "urlsFile": "lists/pages.txt", "env": "local" }))
        .expect("reads");
    assert_eq!(args.urls_file.as_deref(), Some("lists/pages.txt"));
    assert!(args.chooses_pages());
    assert!(!CaptureArgs::default().chooses_pages());
    let refused = |value: serde_json::Value| CaptureArgs::parse(&value).is_err();
    let url = "https://a.test/";
    assert!(refused(json!({ "urlsFile": "" })));
    assert!(refused(json!({ "urlsFile": "pages.xml" })));
    assert!(refused(json!({ "urlsFile": "a.txt", "urls": [url] })));
    assert!(refused(json!({ "urlsFile": "a.txt", "resume": "r1" })));
    assert!(refused(
        json!({ "urlsFile": "a.txt", "discovery": "1-1", "urls": [url] })
    ));
}

#[test]
fn views_take_all_or_visible_and_become_the_scope() {
    let visible = CaptureArgs::parse(&json!({ "views": "visible" })).expect("visible");
    assert_eq!(visible.views, Some(CaptureViews::Visible));
    assert_eq!(visible.scope(), rb_config::ViewScope::Visible);
    let all = CaptureArgs::parse(&json!({ "views": "all" })).expect("all");
    assert_eq!(all.scope(), rb_config::ViewScope::All);
    assert_eq!(CaptureArgs::default().scope(), rb_config::ViewScope::All);
    let named = CaptureArgs::parse(&json!({ "breakpoints": ["desktop"] })).expect("named");
    assert_eq!(
        named.scope(),
        rb_config::ViewScope::Breakpoints(vec!["desktop".to_owned()])
    );
    assert!(CaptureArgs::parse(&json!({ "views": "hidden" })).is_err());
    assert!(CaptureArgs::parse(&json!({ "views": ["desktop"] })).is_err());
}

#[test]
fn views_and_breakpoints_exclude_each_other() {
    for views in ["all", "visible"] {
        let error = CaptureArgs::parse(&json!({ "views": views, "breakpoints": ["mobile"] }))
            .expect_err("both");
        assert_eq!(error, "views and breakpoints exclude each other");
    }
}
