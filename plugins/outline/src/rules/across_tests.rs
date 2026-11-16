use serde_json::json;

use crate::rules::test_util::{heading, landmark, page, param, short, with};
use crate::rules::*;

fn desktop() -> crate::page::PageOutline {
    page(json!({
        "url": "https://site.test/",
        "headings": [heading("h1", 1, "Title"), heading("div > h2", 2, "Promotion")],
        "landmarks": [landmark("nav", "navigation", "Main"), landmark("main", "main", "")]
    }))
}

fn mobile() -> crate::page::PageOutline {
    page(json!({
        "url": "https://site.test/",
        "headings": [heading("h1", 1, "Title"), with(heading("div > h2", 2, "Promotion"), json!({ "hidden": true, "visible": false }))],
        "landmarks": [landmark("main", "main", "")]
    }))
}

#[test]
fn what_a_view_lacks_that_another_shows_is_found_in_that_view() {
    let (wide, narrow) = (desktop(), mobile());
    let peers = [
        Peer {
            view: 1,
            label: "Desktop",
            outline: &wide,
        },
        Peer {
            view: 2,
            label: "Phone",
            outline: &narrow,
        },
    ];
    assert!(
        across(&[], 1, &peers, &[]).is_empty(),
        "the desktop lacks nothing"
    );
    let found = across(&[], 2, &peers, &[]);
    assert_eq!(
        short(&found),
        ["heading-differs div > h2", "landmark-differs nav"]
    );
    assert!(found.iter().all(|f| f.view == Some(2)));
    assert_eq!(param(&found[0], "text"), Some("H2 Promotion"));
    assert_eq!(param(&found[0], "shown"), Some("Desktop"));
    assert_eq!(param(&found[0], "view"), Some("Phone"));
    assert_eq!(param(&found[1], "text"), Some("navigation \"Main\""));
    assert_eq!(found[0].url.as_deref(), Some("https://site.test/"));
}

#[test]
fn one_view_alone_the_sections_and_the_run_rules_hold() {
    let (wide, narrow) = (desktop(), mobile());
    let alone = [Peer {
        view: 2,
        label: "Phone",
        outline: &narrow,
    }];
    assert!(across(&[], 2, &alone, &[]).is_empty());
    let peers = [
        Peer {
            view: 1,
            label: "Desktop",
            outline: &wide,
        },
        Peer {
            view: 2,
            label: "Phone",
            outline: &narrow,
        },
    ];
    let headings_only = across(&[], 2, &peers, &["headings".to_owned()]);
    assert_eq!(short(&headings_only), ["heading-differs div > h2"]);
    let landmark_rule = across(&[LANDMARK_DIFFERS.to_owned()], 2, &peers, &[]);
    assert_eq!(short(&landmark_rule), ["landmark-differs nav"]);
    assert!(across(&[], 9, &peers, &[]).is_empty(), "no such view");
}

#[test]
fn hidden_in_every_view_is_no_difference() {
    let hidden = || {
        page(json!({
            "headings": [with(heading("h2", 2, "Menu"), json!({ "hidden": true, "visible": false }))]
        }))
    };
    let (a, b) = (hidden(), hidden());
    let peers = [
        Peer {
            view: 1,
            label: "A",
            outline: &a,
        },
        Peer {
            view: 2,
            label: "B",
            outline: &b,
        },
    ];
    assert!(across(&[], 1, &peers, &[]).is_empty());
    assert!(across(&[], 2, &peers, &[]).is_empty());
}
