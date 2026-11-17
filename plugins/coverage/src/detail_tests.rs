use serde_json::json;

use super::file;
use crate::fake::FakeHost;
use crate::model::{FileRun, Function, Kind, Run, ViewRun};
use crate::resolve::Cache;
use crate::store;

const TEXT: &str = "body { margin: 0 }\n@media (min-width: 1024px) { .wide { display: grid } }\n";

fn run(source: Option<String>) -> Run {
    let css = |segments: Vec<(u32, u32)>| FileRun {
        url: "http://localhost/site.css".to_owned(),
        kind: Kind::Css,
        total: 74,
        used: segments.iter().map(|(a, b)| u64::from(b - a)).sum(),
        rules: segments.clone(),
        segments,
        source: source.clone(),
        ..FileRun::default()
    };
    let js = |executed| FileRun {
        url: "http://localhost/app.js".to_owned(),
        kind: Kind::Js,
        total: 100,
        used: 50,
        functions: vec![Function {
            name: "onClick".to_owned(),
            start: 10,
            end: 40,
            executed,
        }],
        ..FileRun::default()
    };
    Run {
        views: vec![
            ViewRun {
                view: 1,
                breakpoint: "desktop".to_owned(),
                files: vec![css(vec![(0, 18), (48, 71)]), js(true)],
                ..ViewRun::default()
            },
            ViewRun {
                view: 2,
                breakpoint: "mobile".to_owned(),
                files: vec![css(vec![(0, 18)]), js(false)],
                ..ViewRun::default()
            },
        ],
        ..Run::default()
    }
}

#[test]
fn a_style_sheet_shows_its_text_and_which_views_use_each_rule() {
    let mut host = FakeHost::new();
    let key = store::write_source(&mut host, TEXT, &mut Cache::default());
    let run = run(key);
    let answer = file(
        &mut host,
        &run,
        &json!({ "url": "http://localhost/site.css" }),
    )
    .expect("detail");
    assert_eq!(answer["text"], TEXT);
    assert_eq!(answer["rules"][0]["prelude"], "body");
    assert_eq!(answer["rules"][0]["usedIn"], json!([1, 2]));
    assert_eq!(answer["rules"][1]["prelude"], ".wide");
    assert_eq!(answer["rules"][1]["usedIn"], json!([1]));
    assert_eq!(answer["views"][1]["segments"], json!([[0, 18]]));
    assert_eq!(answer["next"], json!(null));

    let window = file(
        &mut host,
        &run,
        &json!({ "url": "http://localhost/site.css", "offset": 40, "length": 20 }),
    )
    .expect("window");
    assert_eq!(window["text"], &TEXT[40..60]);
    assert_eq!(
        window["rules"],
        json!([{ "prelude": ".wide", "start": 48, "end": 71, "usedIn": [1] }])
    );
    assert_eq!(window["views"][0]["segments"], json!([[48, 60]]));
    assert_eq!(window["next"], json!(60));
}

#[test]
fn a_script_lists_its_functions_with_the_views_that_ran_them() {
    let mut host = FakeHost::new();
    let answer = file(
        &mut host,
        &run(None),
        &json!({ "url": "http://localhost/app.js" }),
    )
    .expect("detail");
    assert_eq!(answer["text"], json!(null));
    assert_eq!(answer["functions"][0]["executedIn"], json!([1]));
}

#[test]
fn an_unknown_file_or_no_url_is_refused() {
    let mut host = FakeHost::new();
    let missing = file(&mut host, &run(None), &json!({ "url": "x" })).expect_err("missing");
    assert_eq!(missing.code(), Some("plugins.error.not_found"));
    let bad = file(&mut host, &run(None), &json!({})).expect_err("no url");
    assert_eq!(bad.code(), Some("plugins.error.invalid"));
}
