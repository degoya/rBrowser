use serde_json::json;

use super::*;
use crate::browser::{Browser, view};
use crate::columns::{self, Target};
use crate::read;

const PAGE: &str = "http://127.0.0.1:18767/storage/index.html";
const CONFIG: &str = r#"{"breakpoints":[{"id":"mobile"},{"id":"tablet"},
  {"id":"desktop","session":"admin"},{"id":"other"},{"id":"fx","engine":"firefox"}]}"#;

/// The acceptance fixture: two views in `default`, one in `admin`, one elsewhere, one Firefox.
fn data() -> Vec<ViewData> {
    let mut browser = Browser::default();
    browser
        .project
        .insert(columns::CONFIG_PATH.into(), CONFIG.as_bytes().to_vec());
    browser.add_view(view(1, "mobile", "chromium", PAGE), "default");
    browser.add_view(view(2, "tablet", "chromium", PAGE), "default");
    browser.add_view(view(3, "desktop", "chromium", PAGE), "admin");
    browser.add_view(
        view(4, "other", "chromium", "https://example.com/"),
        "default",
    );
    browser.add_view(view(5, "fx", "firefox", PAGE), "firefox");
    browser.set_cookie(
        1,
        json!({ "name": "consent", "value": "yes", "domain": "127.0.0.1", "path": "/",
                "httpOnly": false, "secure": false }),
    );
    browser.set_local(1, "featureFlag", "off");
    browser.set_local(3, "featureFlag", "on");
    browser.set_local(3, "theme", "dark");
    let columns = columns::columns(&mut browser);
    read::read_all(&mut browser, &columns, Target::All, &Kind::ALL, true)
}

#[test]
fn a_cookie_of_one_session_is_missing_in_the_other() {
    let rows = rows(Kind::Cookies, &data());
    assert_eq!(rows.len(), 1);
    let consent = &rows[0];
    assert_eq!(consent.name, "consent");
    assert_eq!(consent.state(), State::Missing);
    let cells: Vec<(ViewId, bool)> = consent
        .cells
        .iter()
        .map(|(view, cell)| (*view, cell.is_some()))
        .collect();
    assert_eq!(
        cells,
        [(1, true), (2, true), (3, false)],
        "views 4 (another site) and 5 (Firefox) have no cell"
    );
    assert_eq!(consent.groups(), [vec![1, 2], vec![3]]);
}

#[test]
fn storage_rows_compare_values_per_origin() {
    let local = rows(Kind::Local, &data());
    let names: Vec<(&str, State)> = local
        .iter()
        .map(|row| (row.name.as_str(), row.state()))
        .collect();
    assert_eq!(
        names,
        [("featureFlag", State::Differs), ("theme", State::Missing)]
    );
    assert_eq!(local[0].groups(), [vec![1, 2], vec![3]]);
    assert_eq!(local[0].scope, "http://127.0.0.1:18767");
    assert!(rows(Kind::Session, &data()).is_empty());
}

#[test]
fn equal_entries_everywhere_are_the_same() {
    let mut data = data();
    data.truncate(2);
    let cookies = rows(Kind::Cookies, &data);
    assert_eq!(cookies[0].state(), State::Same);
    assert_eq!(cookies[0].groups(), [vec![1, 2]]);
}

#[test]
fn row_ids_are_stable_and_distinct() {
    let local = rows(Kind::Local, &data());
    assert_eq!(local[0].id(), rows(Kind::Local, &data())[0].id());
    assert_ne!(local[0].id(), local[1].id());
    assert_eq!(local[0].id().len(), 16);
}

#[test]
fn values_are_cut_or_hidden() {
    let cut = value_json("abcdef", Values::Shown(3));
    assert_eq!(cut["value"], "abc");
    assert_eq!(cut["length"], 6);
    assert_eq!(cut["truncated"], true);
    let hidden = value_json("secret", Values::Hidden);
    assert!(hidden.get("value").is_none());
    assert_eq!(hidden["length"], 6);
}

#[test]
fn a_row_shows_cells_with_cookie_attributes_and_null_for_missing() {
    let cookies = rows(Kind::Cookies, &data());
    let row = row_json(&cookies[0], Values::Shown(100), true);
    assert_eq!(row["state"], "missing");
    assert_eq!(row["pinned"], true);
    assert_eq!(row["path"], "/");
    assert_eq!(row["cells"]["1"]["value"], "yes");
    assert_eq!(row["cells"]["1"]["httpOnly"], false);
    assert_eq!(row["cells"]["3"], Value::Null);
    assert!(row["cells"].get("4").is_none());

    let difference = difference_json(&cookies[0]);
    assert_eq!(difference["missing"], json!([3]));
    assert_eq!(difference["groups"], json!([[1, 2], [3]]));
    assert!(
        !difference.to_string().contains("yes"),
        "no value in a difference"
    );
}
