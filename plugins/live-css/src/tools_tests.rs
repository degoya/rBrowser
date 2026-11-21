use rb_plugin_api::FailureKind;
use serde_json::{Value, json};

use super::{clear, page, set, toggle};
use crate::store::DRAFT_PATH;
use crate::testing::{FakeHost, host};

const OUTLINE: &str = "body { outline: 3px solid red }";
const MD_SHEET: &str =
    "@media (min-width: 768px) and (max-width: 1279px) {\n  body { outline: 3px solid red }\n}";

/// The sheet `answer` posts to every view.
fn posted_sheet(answer: &Value) -> Option<&str> {
    answer["$post"]
        .as_array()?
        .iter()
        .find(|post| post["target"] == "all")?["message"]["css"]
        .as_str()
}

fn targets(answer: &Value) -> Vec<String> {
    answer["$post"]
        .as_array()
        .map(|posts| {
            posts
                .iter()
                .map(|post| post["target"].as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// What a new document's page script gets.
fn start(host: &mut FakeHost) -> String {
    let answer = page(
        host,
        &json!({ "view": 2, "message": { "op": "sheet", "reason": "start" } }),
    )
    .expect("answers");
    answer["css"].as_str().expect("css").to_owned()
}

fn code(result: Result<Value, rb_plugin_api::Failure>) -> String {
    result.expect_err("refused").code.unwrap_or_default()
}

#[test]
fn set_for_md_reaches_only_the_md_range_and_holds_after_a_navigation() {
    let mut host = host();
    let answer = set(&mut host, &json!({ "css": OUTLINE, "breakpoints": ["md"] })).expect("set");
    assert_eq!(posted_sheet(&answer), Some(MD_SHEET));
    assert_eq!(targets(&answer), ["all", "panel"]);
    assert_eq!(answer["$post"][1]["message"], json!({ "changed": true }));
    assert_eq!(answer["enabled"], json!(true));
    assert_eq!(answer["blocks"][0]["scope"], json!("query"));
    assert_eq!(
        answer["blocks"][0]["media"],
        json!("(min-width: 768px) and (max-width: 1279px)")
    );
    assert_eq!(answer["blocks"][0]["copy"], json!(MD_SHEET));
    assert_eq!(host.sheet_file(), MD_SHEET);
    // A navigation: the new document's page script asks again.
    assert_eq!(start(&mut host), MD_SHEET);
}

#[test]
fn off_no_view_gets_the_sheet_and_the_draft_stays() {
    let mut host = host();
    set(&mut host, &json!({ "css": OUTLINE, "breakpoints": ["md"] })).expect("set");
    let off = toggle(&mut host).expect("off");
    assert_eq!(posted_sheet(&off), Some(""));
    assert_eq!(targets(&off), ["all", "panel", "status"]);
    assert_eq!(
        off["$post"][2]["message"],
        json!({ "key": "plugin.live-css.status.off", "level": "info", "params": {} })
    );
    assert_eq!(start(&mut host), "");
    assert_eq!(host.draft()["enabled"], json!(false));
    assert_eq!(host.draft()["blocks"][0]["css"], json!(OUTLINE));
    assert_eq!(
        host.sheet_file(),
        MD_SHEET,
        "the file keeps the whole draft"
    );
    let on = toggle(&mut host).expect("on");
    assert_eq!(posted_sheet(&on), Some(MD_SHEET));
    assert_eq!(
        on["$post"][2]["message"]["key"],
        json!("plugin.live-css.status.on")
    );
}

#[test]
fn enabled_alone_switches_and_keeps_the_blocks() {
    let mut host = host();
    set(&mut host, &json!({ "css": OUTLINE })).expect("set");
    let off = set(&mut host, &json!({ "enabled": false })).expect("off");
    assert_eq!(posted_sheet(&off), Some(""));
    assert_eq!(off["blocks"][0]["css"], json!(OUTLINE));
    assert_eq!(off["blocks"][0]["scope"], json!("all"));
}

#[test]
fn without_arguments_set_answers_the_draft_and_the_grid() {
    let mut host = host();
    let answer = set(&mut host, &json!({})).expect("answers");
    assert_eq!(
        answer,
        json!({
            "available": "trusted",
            "enabled": true,
            "blocks": [],
            "breakpoints": [
                { "id": "mobile", "label": "MOBILE", "width": 375 },
                { "id": "md", "label": "MD", "width": 768 },
                { "id": "lg", "label": "LG", "width": 1280 },
            ],
            "file": ".rbrowser/out/live-css/draft.css",
        })
    );
    assert_eq!(host.writes, 0);
}

#[test]
fn blocks_replace_the_draft_in_order_and_switch_it_on() {
    let mut host = host();
    set(&mut host, &json!({ "css": "a{}", "enabled": false })).expect("set");
    assert_eq!(host.draft()["enabled"], json!(false));
    let answer = set(
        &mut host,
        &json!({ "blocks": [
            { "css": "body { margin: 0 }" },
            { "css": OUTLINE, "breakpoints": ["md"] },
            { "css": "p{}", "breakpoints": ["mobile", "md", "lg"] },
        ] }),
    )
    .expect("set");
    assert_eq!(
        posted_sheet(&answer),
        Some(format!("body {{ margin: 0 }}\n{MD_SHEET}\np{{}}").as_str())
    );
    assert_eq!(answer["blocks"][2]["scope"], json!("all"));
    assert_eq!(host.draft()["enabled"], json!(true));
}

#[test]
fn clear_removes_the_blocks() {
    let mut host = host();
    set(&mut host, &json!({ "css": OUTLINE })).expect("set");
    let answer = clear(&mut host).expect("cleared");
    assert_eq!(posted_sheet(&answer), Some(""));
    assert_eq!(answer["blocks"], json!([]));
    assert_eq!(host.draft()["blocks"], json!([]));
}

#[test]
fn arguments_that_do_not_read_are_refused() {
    for args in [
        json!({ "css": "a{}", "blocks": [] }),
        json!({ "breakpoints": ["md"] }),
        json!({ "enabled": "yes" }),
        json!({ "blocks": "a{}" }),
        json!({ "css": 3 }),
        json!({ "blocks": [{ "breakpoints": ["md"] }] }),
    ] {
        assert_eq!(
            code(set(&mut host(), &args)),
            "plugin.live-css.errors.bad_args",
            "{args}"
        );
    }
}

#[test]
fn a_breakpoint_the_grid_does_not_show_is_named() {
    let mut host = host();
    let failure =
        set(&mut host, &json!({ "css": "a{}", "breakpoints": ["xl"] })).expect_err("refused");
    assert_eq!(failure.kind, FailureKind::NotFound);
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.live-css.errors.unknown_breakpoint")
    );
    assert_eq!(failure.param("id"), Some("xl"));
    assert_eq!(host.writes, 0);
}

#[test]
fn a_block_that_would_leave_its_media_query_is_refused() {
    let mut host = host();
    let failure = set(
        &mut host,
        &json!({ "css": "} body { color: red } @media x {", "breakpoints": ["md"] }),
    )
    .expect_err("refused");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.live-css.errors.braces")
    );
    assert_eq!(host.writes, 0);
}

#[test]
fn a_sheet_past_one_message_is_refused() {
    // 30 000 bytes of empty strings, one per line: within the CSS limit, but indented inside its
    // media query and escaped as JSON well past a page script's message.
    let css = "\"\"\n".repeat(10_000);
    let failure =
        set(&mut host(), &json!({ "css": css, "breakpoints": ["md"] })).expect_err("refused");
    assert_eq!(failure.kind, FailureKind::Limit);
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.live-css.errors.too_long")
    );
}

#[test]
fn an_untrusted_workspace_neither_reads_nor_keeps_a_draft() {
    let mut host = host();
    host.trusted = false;
    // A draft a repository committed.
    host.file(
        DRAFT_PATH,
        r#"{"version": 1, "enabled": true, "blocks": [{"css": "body { display: none }"}]}"#,
    );
    assert_eq!(start(&mut host), "");
    let answer = set(&mut host, &json!({})).expect("answers");
    assert_eq!(answer["available"], json!("untrusted"));
    assert_eq!(answer["blocks"], json!([]));
    assert_eq!(
        code(set(&mut host, &json!({ "css": OUTLINE }))),
        "plugin.live-css.errors.untrusted"
    );
    assert_eq!(code(toggle(&mut host)), "plugin.live-css.errors.untrusted");
    assert_eq!(code(clear(&mut host)), "plugin.live-css.errors.untrusted");
    assert_eq!(host.writes, 0);
}

#[test]
fn without_a_project_nothing_is_kept() {
    let mut host = host();
    host.no_project = true;
    assert_eq!(
        set(&mut host, &json!({})).expect("answers")["available"],
        json!("noProject")
    );
    assert_eq!(
        code(set(&mut host, &json!({ "css": OUTLINE }))),
        "plugin.live-css.errors.no_project"
    );
    assert_eq!(start(&mut host), "");
}

#[test]
fn apply_on_navigate_off_leaves_a_new_document_bare_but_not_a_refresh() {
    let mut host = host();
    set(&mut host, &json!({ "css": OUTLINE })).expect("set");
    host.set("applyOnNavigate", "false");
    assert_eq!(start(&mut host), "");
    let refresh = page(
        &mut host,
        &json!({ "view": 2, "message": { "op": "sheet", "reason": "refresh" } }),
    )
    .expect("answers");
    assert_eq!(refresh["css"], json!(OUTLINE));
    host.set("applyOnNavigate", "\"maybe\"");
    assert_eq!(start(&mut host), OUTLINE);
    assert_eq!(host.logs.len(), 1);
}

#[test]
fn a_page_request_other_than_sheet_is_refused() {
    for message in [
        json!({ "op": "init" }),
        json!({ "op": "sheet" }),
        json!("sheet"),
    ] {
        assert_eq!(
            code(page(&mut host(), &json!({ "view": 1, "message": message }))),
            "plugin.live-css.errors.bad_args"
        );
    }
}
