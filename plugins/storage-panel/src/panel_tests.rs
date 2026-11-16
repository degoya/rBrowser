use serde_json::json;

use super::*;
use crate::browser::{Browser, RECORD, view};
use crate::columns::CONFIG_PATH;
use crate::pins::PINS_PATH;

const ORIGIN: &str = "http://127.0.0.1:18767";
const PAGE: &str = "http://127.0.0.1:18767/storage/index.html";

fn browser() -> Browser {
    let mut browser = Browser::default();
    browser.project.insert(
        CONFIG_PATH.into(),
        br#"{"breakpoints":[{"id":"a"},{"id":"b"},{"id":"c","session":"admin"},{"id":"f"}]}"#
            .to_vec(),
    );
    browser.add_view(view(1, "a", "chromium", PAGE), "default");
    browser.add_view(view(2, "b", "chromium", PAGE), "default");
    browser.add_view(view(3, "c", "chromium", PAGE), "admin");
    browser.add_view(view(4, "f", "firefox", PAGE), "firefox");
    browser.set_cookie(
        1,
        json!({ "name": "consent", "value": "yes", "domain": "127.0.0.1", "path": "/" }),
    );
    browser.set_local(1, "featureFlag", "off");
    browser
}

#[test]
fn the_command_without_an_op_asks_for_the_panel() {
    let answer = command(&mut browser(), &json!({})).expect("answers");
    assert_eq!(answer, json!({ "$open": { "panel": "storage" } }));
    assert!(command(&mut browser(), &json!({ "op": "drop" })).is_err());
}

#[test]
fn the_list_has_a_column_per_view_and_marks_the_missing_cell() {
    let answer =
        command(&mut browser(), &json!({ "op": "list", "kind": "cookies" })).expect("answers");
    let columns: Vec<(u64, &str, &str)> = answer["views"]
        .as_array()
        .expect("views")
        .iter()
        .map(|c| {
            (
                c["id"].as_u64().unwrap_or_default(),
                c["session"].as_str().unwrap_or(""),
                c["status"].as_str().unwrap_or(""),
            )
        })
        .collect();
    assert_eq!(
        columns,
        [
            (1, "default", "ok"),
            (2, "default", "ok"),
            (3, "admin", "ok"),
            (4, "default", "unsupported"),
        ]
    );
    let row = &answer["rows"][0];
    assert_eq!(row["name"], "consent");
    assert_eq!(row["state"], "missing");
    assert_eq!(row["cells"]["1"]["value"], "yes");
    assert_eq!(row["cells"]["3"], Value::Null);
    assert_eq!(answer["truncated"], false);
}

/// RB-090-44: a list fills the panels window's message limit (`LIST_BUDGET`, 1 MiB), not the
/// page bridge's 64 KiB; past it the list is cut and says so.
#[test]
fn a_long_list_shows_whole_and_a_huge_one_stays_within_its_budget() {
    let mut browser = browser();
    for i in 0..400 {
        browser.set_local(1, &format!("key{i:03}"), &"x".repeat(500));
    }
    let answer = command(&mut browser, &json!({ "op": "list", "kind": "local" })).expect("answers");
    assert!(
        answer.to_string().len() > 52 * 1024,
        "more than the old budget"
    );
    assert_eq!(answer["truncated"], false);
    assert_eq!(answer["total"], 401);
    assert_eq!(answer["shown"], 401);
    assert_eq!(answer["rows"][1]["cells"]["1"]["truncated"], true);
    // Within the 1000 entries a view is read for, names of 2000 characters pass 1 MiB.
    let long = "n".repeat(2000);
    for i in 400..999 {
        browser.set_local(1, &format!("key{i:03}{long}"), "x");
    }
    let answer = command(&mut browser, &json!({ "op": "list", "kind": "local" })).expect("answers");
    assert!(answer.to_string().len() < LIST_BUDGET + 16 * 1024);
    assert_eq!(answer["truncated"], true);
    assert_eq!(answer["total"], 1000);
}

#[test]
fn value_answers_a_cells_whole_value() {
    let mut browser = browser();
    browser.set_local(2, "long", &"y".repeat(500));
    let args = json!({ "op": "value", "kind": "local", "view": 2, "scope": ORIGIN,
                       "name": "long" });
    let answer = command(&mut browser, &args).expect("answers");
    assert_eq!(answer["length"], 500);
    assert_eq!(answer["value"].as_str().map(str::len), Some(500));
    // RB-090-44: a value as long as a write may set comes whole, not cut at 48 K.
    browser.set_local(2, "huge", &"z".repeat(100_000));
    let huge = json!({ "op": "value", "kind": "local", "view": 2, "scope": ORIGIN,
                       "name": "huge" });
    let answer = command(&mut browser, &huge).expect("answers");
    assert_eq!(answer["value"].as_str().map(str::len), Some(100_000));
    let missing = json!({ "op": "value", "kind": "local", "view": 3, "scope": ORIGIN,
                          "name": "long" });
    assert_eq!(
        command(&mut browser, &missing)
            .expect_err("none")
            .code
            .as_deref(),
        Some("plugin.storage-panel.errors.no_entry")
    );
}

#[test]
fn the_panel_writes_whatever_the_mcp_switch_says() {
    let mut browser = browser();
    let set = json!({ "op": "set", "kind": "local", "target": 2, "name": "featureFlag",
                      "value": "on" });
    let answer = command(&mut browser, &set).expect("written");
    assert_eq!(answer["results"][0]["ok"], true);
    assert_eq!(browser.local_item(1, "featureFlag").as_deref(), Some("on"));
    let delete = json!({ "op": "delete", "kind": "cookies", "target": 1, "name": "consent",
                         "cookie": { "domain": "127.0.0.1", "path": "/" } });
    command(&mut browser, &delete).expect("deleted");
    assert!(browser.cookies_of(2).is_empty());
}

#[test]
fn pinning_a_cell_saves_applies_and_marks_it() {
    let mut browser = browser();
    let pin = json!({ "op": "pin", "kind": "local", "view": 1, "scope": ORIGIN,
                      "name": "featureFlag" });
    let answer = command(&mut browser, &pin).expect("pinned");
    assert_eq!(answer["pins"][0]["key"], "featureFlag");
    assert_eq!(
        answer["failed"],
        json!([]),
        "a Firefox view is passed over, not failed"
    );
    assert!(browser.project.contains_key(PINS_PATH));
    assert_eq!(browser.local_item(3, "featureFlag").as_deref(), Some("off"));
    assert!(browser.session_item(3, RECORD).is_some());

    let list = command(&mut browser, &json!({ "op": "list", "kind": "local" })).expect("answers");
    assert_eq!(list["rows"][0]["pinned"], true);

    let id = answer["pins"][0]["id"].clone();
    command(
        &mut browser,
        &json!({ "op": "enable", "id": id, "enabled": false }),
    )
    .expect("switched");
    assert!(browser.session_item(3, RECORD).is_none());
    let pins = command(&mut browser, &json!({ "op": "pins" })).expect("answers");
    assert_eq!(pins["pins"][0]["enabled"], false);
    command(&mut browser, &json!({ "op": "unpin", "id": id })).expect("removed");
    let pins = command(&mut browser, &json!({ "op": "pins" })).expect("answers");
    assert_eq!(pins["pins"], json!([]));
    assert!(command(&mut browser, &json!({ "op": "unpin", "id": "zz" })).is_err());
}

#[test]
fn a_cookie_pin_of_one_session_keeps_to_it() {
    let mut browser = browser();
    let pin = json!({ "op": "pin", "kind": "cookies", "view": 1, "scope": "127.0.0.1",
                      "path": "/", "name": "consent", "sessionOnly": true });
    let answer = command(&mut browser, &pin).expect("pinned");
    assert_eq!(answer["pins"][0]["session"], "default");
    assert_eq!(answer["pins"][0]["origin"], ORIGIN);
    assert!(browser.cookies_of(3).is_empty(), "admin keeps its own jar");
}

#[test]
fn an_untrusted_workspace_keeps_its_pins_file_and_pins_nothing() {
    let mut browser = browser();
    let pin = json!({ "op": "pin", "kind": "local", "view": 1, "scope": ORIGIN,
                      "name": "featureFlag" });
    let answer = command(&mut browser, &pin).expect("pinned");
    let id = answer["pins"][0]["id"].clone();
    let file = browser.project.get(PINS_PATH).cloned();

    browser.untrusted = true;
    let untrusted = Some(rb_plugin_api::codes::UNTRUSTED_WORKSPACE);
    let refused = command(&mut browser, &pin).expect_err("refused");
    assert_eq!(refused.code.as_deref(), untrusted);
    for op in [
        json!({ "op": "unpin", "id": id }),
        json!({ "op": "enable", "id": id, "enabled": false }),
    ] {
        let refused = command(&mut browser, &op).expect_err("refused");
        assert_eq!(refused.code.as_deref(), untrusted, "{op}");
    }
    assert_eq!(
        browser.project.get(PINS_PATH).cloned(),
        file,
        "never overwritten"
    );
    let pins = command(&mut browser, &json!({ "op": "pins" })).expect("answers");
    assert_eq!(pins["pins"], json!([]));
    assert_eq!(pins["untrusted"], true);
}
