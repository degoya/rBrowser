use std::collections::BTreeSet;

use serde_json::json;

use super::{Cache, RuleUsage, ScriptCoverage, css, js};
use crate::fake::{FakeHost, PAGE, Sheet};
use crate::settings::Settings;

fn used(id: &str, start: f64, end: f64) -> RuleUsage {
    RuleUsage {
        style_sheet_id: id.to_owned(),
        start_offset: start,
        end_offset: end,
        used: true,
    }
}

#[test]
fn style_elements_and_linked_sheets_are_matched_by_their_text() {
    let mut host = FakeHost::new().view(
        1,
        "desktop",
        1280,
        vec![
            Sheet {
                id: "10",
                href: None,
                text: "h1 { x: y }".to_owned(),
                used: Vec::new(),
            },
            Sheet {
                id: "11",
                href: Some("https://cdn.example/lib.css"),
                text: "p { a: b } q { c: d }".to_owned(),
                used: Vec::new(),
            },
        ],
    );
    let usage = [used("11", 0.0, 10.0), used("10", 0.0, 11.0)];
    let mut cache = Cache::default();
    let (page, files) =
        css(&mut host, 1, &usage, &Settings::default(), &mut cache).expect("resolved");
    assert_eq!(page, PAGE);
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].url, format!("{PAGE}#style-1"));
    assert!(files[0].inline);
    assert_eq!(files[0].used, 11);
    assert_eq!(files[1].url, "https://cdn.example/lib.css");
    assert_eq!((files[1].used, files[1].total), (10, 21));
    // Each text is stored once for the panel's source view.
    assert_eq!(cache.written.len(), 2);
    assert!(files.iter().all(|file| file.source.is_some()));

    let settings = Settings {
        third_party: false,
        ..Settings::default()
    };
    let (_, files) = css(&mut host, 1, &usage, &settings, &mut Cache::default()).expect("resolved");
    assert_eq!(files.len(), 1, "the CDN's sheet is another origin");
}

#[test]
fn a_sheet_the_document_does_not_list_is_named_after_the_page() {
    let mut host = FakeHost::new().view(
        1,
        "desktop",
        1280,
        vec![Sheet {
            id: "20",
            href: None,
            text: "a { }".to_owned(),
            used: Vec::new(),
        }],
    );
    // The document lists "a { }" only; Chromium also knows a constructed sheet "b { }".
    host.views[0].sheets.push(Sheet {
        id: "21",
        href: Some("blob:constructed"),
        text: "b { }".to_owned(),
        used: Vec::new(),
    });
    let usage = [used("21", 0.0, 5.0)];
    let (_, files) = css(
        &mut host,
        1,
        &usage,
        &Settings::default(),
        &mut Cache::default(),
    )
    .expect("resolved");
    let urls: Vec<&str> = files.iter().map(|file| file.url.as_str()).collect();
    assert_eq!(urls, [format!("{PAGE}#style-1"), format!("{PAGE}#sheet-1")]);
    assert_eq!(files[1].used, 5);
}

#[test]
fn scripts_keep_http_ones_and_name_inline_ones_after_the_page() {
    let scripts: Vec<ScriptCoverage> = serde_json::from_value(json!([
        { "scriptId": "9", "url": PAGE, "functions": [
            { "functionName": "", "ranges": [{ "startOffset": 0, "endOffset": 40, "count": 1 }] } ] },
        { "scriptId": "3", "url": "", "functions": [] },
        { "scriptId": "4", "url": "chrome-extension://x/a.js", "functions": [] },
        { "scriptId": "5", "url": "http://localhost:8080/coverage/app.js", "functions": [
            { "functionName": "", "ranges": [{ "startOffset": 0, "endOffset": 80, "count": 1 }, { "startOffset": 10, "endOffset": 30, "count": 0 }] },
            { "functionName": "never", "ranges": [{ "startOffset": 40, "endOffset": 70, "count": 0 }] } ] },
        { "scriptId": "8", "url": format!("{PAGE}#x"), "functions": [
            { "functionName": "", "ranges": [{ "startOffset": 0, "endOffset": 10, "count": 1 }] } ] }
    ]))
    .expect("coverage");
    let files = js(&scripts, PAGE, &BTreeSet::new(), &Settings::default());
    let urls: Vec<&str> = files.iter().map(|file| file.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "http://localhost:8080/coverage/app.js",
            &format!("{PAGE}#script-1"),
            &format!("{PAGE}#script-2"),
        ]
    );
    let app = &files[0];
    assert_eq!((app.total, app.used), (80, 30));
    assert_eq!(app.functions.len(), 1);
    assert_eq!(app.functions[0].name, "never");
    let skip: BTreeSet<String> = ["5".to_owned()].into();
    assert_eq!(js(&scripts, PAGE, &skip, &Settings::default()).len(), 2);
}
