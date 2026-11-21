use rb_plugin_api::{FailureKind, codes};
use serde_json::{Map, Value, json};

use super::*;
use crate::testing::{self, PAGE, SECRET};

fn object(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn rule_ids(answer: &Value) -> Vec<String> {
    answer["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|finding| finding["rule"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn the_view_mode_reads_the_active_view_and_names_its_limits() {
    let mut host = testing::host();
    let args = object(testing::feed_args(1));
    let answer = check(&mut host, &args, &[], None)
        .expect("an answer")
        .answer;
    assert_eq!(answer["mode"], "view");
    assert_eq!(answer["view"], 1);
    assert_eq!(answer["url"], PAGE);
    assert_eq!(answer["site"], "example.test");
    assert_eq!(answer["notice"], NOTICE_KEY);
    assert_eq!(answer["noticeText"], NOTICE_TEXT);
    assert_eq!(
        answer["caveats"],
        json!(["viewSession", "firstPartyCookies", "noRejectStep"])
    );
    assert_eq!(
        rule_ids(&answer),
        vec!["request.before_consent", "cookie.before_consent"]
    );
    assert_eq!(answer["banner"]["cmp"], "onetrust");
    assert_eq!(answer["banner"]["name"], "OneTrust");
    assert!(answer["afterReject"].is_null());
    assert!(!answer.to_string().contains(SECRET));
}

#[test]
fn the_probe_capture_is_judged_without_touching_a_view() {
    let mut host = testing::host();
    let args = object(testing::probe_args(true));
    let answer = check(&mut host, &args, &[], None)
        .expect("an answer")
        .answer;
    assert_eq!(answer["mode"], "session");
    assert!(answer["view"].is_null());
    assert_eq!(
        answer["session"],
        json!({ "fresh": true, "discarded": true })
    );
    assert_eq!(answer["caveats"], json!([]));
    assert_eq!(rule_ids(&answer).len(), 4);
    assert_eq!(answer["afterReject"]["hosts"][0]["host"], "pixel.ads.test");
    assert_eq!(
        answer["diff"]["hostsAdded"],
        json!(["pixel.ads.test pixel"])
    );
    assert!(host.evaluated.is_empty() && host.cdp_sent.is_empty());
    assert!(!answer.to_string().contains(SECRET));
}

#[test]
fn a_probe_session_left_behind_or_not_rejected_is_a_caveat() {
    let mut host = testing::host();
    let mut probe = testing::probe_args(false);
    probe[crate::capture::PROBE_KEY]["session"]["discarded"] = json!(false);
    let args = object(probe);
    let answer = check(&mut host, &args, &[], None)
        .expect("an answer")
        .answer;
    assert_eq!(answer["caveats"], json!(["sessionKept", "notRejected"]));
}

#[test]
fn another_url_needs_the_probe_session() {
    let mut host = testing::host();
    let args = object(json!({ "url": "https://other.test/" }));
    let failure = check(&mut host, &args, &[], None).expect_err("no probe");
    assert_eq!(failure.kind, FailureKind::NotSupported);
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
    let same = object(json!({ "url": format!("{PAGE}#top") }));
    assert!(check(&mut host, &same, &[], None).is_ok());
}

#[test]
fn a_malformed_url_or_target_is_refused() {
    let mut host = testing::host();
    let args = object(json!({ "url": 7 }));
    assert_eq!(
        check(&mut host, &args, &[], None)
            .expect_err("bad url")
            .code(),
        Some(codes::INVALID)
    );
    let args = object(json!({ "target": "left" }));
    assert!(check(&mut host, &args, &[], None).is_err());
}

#[test]
fn the_export_carries_the_notice_and_no_value() {
    let mut host = testing::host();
    let args = object(json!({ "export": true }));
    let answer = check(&mut host, &args, &[], None)
        .expect("an answer")
        .answer;
    assert_eq!(
        answer["exported"],
        format!(".rbrowser/out/consent-check/{EXPORT_FILE}")
    );
    let written = host.out.get(EXPORT_FILE).expect("the export");
    let text = String::from_utf8_lossy(written);
    assert!(text.contains(NOTICE_TEXT));
    assert!(!text.contains(SECRET));
}

#[test]
fn the_command_reports_the_findings_and_posts_the_answer_to_the_panel() {
    let mut host = testing::host();
    let answer = command(&mut host, &object(testing::feed_args(1))).expect("an answer");
    assert_eq!(answer["findings"], 2);
    assert_eq!(answer["reported"], true);
    assert_eq!(host.findings.len(), 2);
    let post = &answer[crate::POST_KEY][0];
    assert_eq!(post["target"], "panel");
    assert_eq!(post["message"]["type"], "checked");
    assert_eq!(post["message"]["answer"]["mode"], "view");
}

#[test]
fn a_view_that_is_gone_is_not_found() {
    let mut host = testing::host();
    let args = object(json!({ "target": 9 }));
    let failure = check(&mut host, &args, &[], None).expect_err("no view 9");
    assert_eq!(failure.code(), Some(codes::NOT_FOUND));
}

fn offered(extra: Value) -> Map<String, Value> {
    let mut args = object(extra);
    args.insert(ASK_KEY.to_owned(), Value::Bool(true));
    args
}

#[test]
fn the_hosts_offer_is_answered_with_a_probe_of_the_views_page() {
    let mut host = testing::host();
    let checked = check(&mut host, &offered(json!({})), &[], None).expect("an answer");
    assert!(checked.probing);
    assert!(checked.findings.is_empty());
    let request = &checked.answer[ASK_KEY];
    assert_eq!(request["url"], PAGE);
    assert!(
        request["read"]
            .as_str()
            .is_some_and(|script| script.contains("\"mode\":\"read\""))
    );
    assert!(
        request["reject"]
            .as_str()
            .is_some_and(|script| script.contains("\"mode\":\"reject\""))
    );
    assert!(
        host.evaluated.is_empty() && host.cdp_sent.is_empty(),
        "the view is not read"
    );
}

#[test]
fn without_the_reject_step_the_probe_has_no_reject_collector() {
    let mut host = testing::host();
    host.settings
        .insert("rejectStep".to_owned(), "false".to_owned());
    let checked = check(&mut host, &offered(json!({})), &[], None).expect("an answer");
    assert!(checked.answer[ASK_KEY]["reject"].is_null());
}

#[test]
fn with_the_offer_another_url_is_probed() {
    let mut host = testing::host();
    let args = offered(json!({ "url": "https://other.test/page" }));
    let checked = check(&mut host, &args, &[], None).expect("an answer");
    assert_eq!(checked.answer[ASK_KEY]["url"], "https://other.test/page");
    let bad = offered(json!({ "url": "file:///etc/passwd" }));
    assert_eq!(
        check(&mut host, &bad, &[], None)
            .expect_err("no web page")
            .code(),
        Some(codes::INVALID)
    );
}

#[test]
fn the_command_passes_the_probe_request_on_and_reports_nothing_yet() {
    let mut host = testing::host();
    let answer = command(&mut host, &offered(json!({}))).expect("an answer");
    assert_eq!(answer[ASK_KEY]["url"], PAGE);
    assert!(answer.get(crate::POST_KEY).is_none());
    assert!(host.findings.is_empty());
}

#[test]
fn the_second_call_with_the_capture_is_judged() {
    let mut host = testing::host();
    let answer = command(&mut host, &object(testing::probe_args(true))).expect("an answer");
    assert_eq!(answer["findings"], 4);
    assert_eq!(
        answer[crate::POST_KEY][0]["message"]["answer"]["mode"],
        "session"
    );
}
