use rb_visual::sitemap::RunBreakpoint;
use serde_json::json;

use super::*;

#[test]
fn the_pages_of_the_plugins_answer() {
    let answer = json!({ "pages": [
        { "url": "http://local.test/a", "sitemapUrl": "https://prod.test/a" },
        { "url": "http://local.test/b" },
        { "sitemapUrl": "no url" }
    ] });
    assert_eq!(
        pages_of(&answer),
        [
            (
                "http://local.test/a".to_owned(),
                Some("https://prod.test/a".to_owned())
            ),
            ("http://local.test/b".to_owned(), None),
        ]
    );
    assert!(pages_of(&json!({})).is_empty());
}

#[test]
fn only_http_and_https_pages_load() {
    assert!(loadable("http://a.test/"));
    assert!(loadable("HTTPS://a.test/"));
    for refused in [
        "javascript:alert(1)",
        "file:///C:/x",
        "data:text/html,x",
        "ftp://a",
    ] {
        assert!(!loadable(refused), "{refused}");
    }
}

#[test]
fn a_run_id_taken_in_the_same_second_gets_a_suffix() {
    let root = tempfile::tempdir().expect("temp");
    let first = fresh_id(root.path(), 1_778_457_600).expect("an id");
    RunStore::open(root.path(), &first, true)
        .expect("opens")
        .expect("made");
    let second = fresh_id(root.path(), 1_778_457_600).expect("an id");
    assert_eq!(second, format!("{first}-2"));
}

#[test]
fn a_breakpoint_gone_from_the_config_keeps_its_recorded_size() {
    let breakpoints = vec![RunBreakpoint {
        name: "old".to_owned(),
        label: "Old".to_owned(),
        width: 600,
        height: 900,
        dpr: 2.0,
    }];
    let run = SitemapRun::new("r", "s", breakpoints, &[], 0);
    let spec = spec_for(&run, &BTreeMap::new(), "old");
    assert_eq!(
        (
            spec.emulation.width,
            spec.emulation.height,
            spec.emulation.dpr
        ),
        (600, 900, 2.0)
    );
    assert_eq!(spec.session, DEFAULT_SESSION);
}
