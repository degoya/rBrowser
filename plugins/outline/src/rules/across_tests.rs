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

/// A focusable link `name` at `path`, `order` its place in the document.
fn link(path: &str, name: &str) -> serde_json::Value {
    with(
        crate::rules::test_util::at(path),
        json!({ "tag": "a", "role": "link", "name": name }),
    )
}

/// The acceptance page of RB-094-02: the menu link "Pricing" only in the desktop's navigation,
/// in three breakpoints.
fn three_breakpoints() -> [crate::page::PageOutline; 3] {
    let common = [link("header > a", "Home"), link("main > a", "Docs")];
    let desktop = page(json!({
        "url": "https://site.test/",
        "focus": [common[0].clone(), link("nav > a", "Pricing"), common[1].clone()]
    }));
    let tablet = page(json!({
        "url": "https://site.test/",
        "focus": [common[0].clone(), common[1].clone()]
    }));
    let phone = page(json!({
        "url": "https://site.test/",
        "focus": [common[0].clone(), with(link("nav > a", "Pricing"), json!({ "visible": false })), common[1].clone()]
    }));
    [desktop, tablet, phone]
}

#[test]
fn a_menu_link_only_on_the_desktop_is_one_difference_naming_where_it_is_missing() {
    let [desktop, tablet, phone] = three_breakpoints();
    let peers = [
        Peer {
            view: 1,
            label: "Desktop",
            outline: &desktop,
        },
        Peer {
            view: 2,
            label: "Tablet",
            outline: &tablet,
        },
        Peer {
            view: 3,
            label: "Phone",
            outline: &phone,
        },
    ];
    let rows = differences(&peers, &[]);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].kind, "interactive");
    assert_eq!(rows[0].label, "link \"Pricing\"");
    assert_eq!(rows[0].shown, [1]);
    assert_eq!(rows[0].missing, [2, 3]);
    assert_eq!(rows[0].path, "nav > a");
    assert!(!rows[0].views[&3].visible, "the phone has it, hidden");
    assert!(!rows[0].views.contains_key(&2), "the tablet lacks it");

    // The check runs view by view: the views lacking it find it at the same element with the
    // same words, so the findings panel shows one finding with both breakpoints.
    assert!(across(&[], 1, &peers, &[]).is_empty());
    let found: Vec<_> = [2, 3]
        .into_iter()
        .flat_map(|view| across(&[], view, &peers, &[]))
        .collect();
    assert_eq!(
        short(&found),
        ["interactive-differs nav > a", "interactive-differs nav > a"]
    );
    for finding in &found {
        assert_eq!(param(finding, "text"), Some("link \"Pricing\""));
        assert_eq!(param(finding, "shown"), Some("Desktop"));
        assert_eq!(param(finding, "missing"), Some("Tablet, Phone"));
        assert_eq!(
            finding.message,
            "plugin.outline.findings.interactive-differs"
        );
    }
}

#[test]
fn the_compared_sections_choose_the_kinds_of_element() {
    let [desktop, tablet, _] = three_breakpoints();
    let peers = [
        Peer {
            view: 1,
            label: "Desktop",
            outline: &desktop,
        },
        Peer {
            view: 2,
            label: "Tablet",
            outline: &tablet,
        },
    ];
    let headings_only = ["headings".to_owned(), "landmarks".to_owned()];
    assert!(differences(&peers, &headings_only).is_empty());
    assert!(across(&[], 2, &peers, &headings_only).is_empty());
    let rule_off = [HEADING_DIFFERS.to_owned()];
    assert!(across(&rule_off, 2, &peers, &[]).is_empty());
    assert_eq!(differences(&peers, &["focus".to_owned()]).len(), 1);
}

#[test]
fn the_heading_and_landmark_findings_name_the_views_missing_them_too() {
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
    let found = across(&[], 2, &peers, &[]);
    assert!(
        found
            .iter()
            .all(|finding| param(finding, "missing") == Some("Phone"))
    );
    let kinds: Vec<&str> = differences(&peers, &[])
        .iter()
        .map(|row| row.kind)
        .collect();
    assert_eq!(kinds, ["headings", "landmarks"]);
}
