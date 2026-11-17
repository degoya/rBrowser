use serde_json::json;

use super::*;
use crate::manifest::parse_manifest;
use crate::test_support::{ManifestBuilder, signing_key};

fn manifest(tables: &str) -> PluginManifest {
    let text = ManifestBuilder::default()
        .tables(tables)
        .text(&signing_key(1));
    parse_manifest(text.as_bytes()).expect("a valid manifest")
}

const TWO_SCRIPTS: &str = "\n[[page_scripts]]\npath = \"page/a.js\"\nrun_at = \"document-start\"\n\
                           \n[[page_scripts]]\npath = \"page/b.js\"\n";

#[test]
fn a_plugin_without_page_scripts_has_no_bridge() {
    assert!(PageBridge::new(&manifest(""), |_| None).is_none());
}

#[test]
fn the_bridge_names_its_world_by_the_plugin_and_its_binding_at_random() {
    let manifest = manifest(TWO_SCRIPTS);
    let file = |path: &str| Some(format!("/* {path} */").into_bytes());
    let one = PageBridge::new(&manifest, file).expect("a bridge");
    let two = PageBridge::new(&manifest, file).expect("a bridge");
    assert_eq!(one.world, "rbplugin-sample");
    assert!(one.binding.starts_with("__rbp_") && one.binding.len() == 30);
    assert!(one.receiver.starts_with("__rbr_"));
    assert_ne!(one.binding, two.binding, "random per start");
    assert_eq!(one.scripts.len(), 2);
    assert_eq!(one.scripts[0].run_at, "document-start");
    assert_eq!(one.scripts[1].run_at, "document-idle");
}

#[test]
fn a_missing_or_broken_script_is_left_out() {
    let manifest = manifest(TWO_SCRIPTS);
    let bridge = PageBridge::new(&manifest, |path| {
        (path == "page/b.js").then(|| b"ok();".to_vec())
    })
    .expect("one script is left");
    assert_eq!(bridge.scripts.len(), 1);
    assert!(PageBridge::new(&manifest, |_| Some(vec![0xff, 0xfe])).is_none());
}

#[test]
fn the_source_holds_the_configuration_the_prologue_and_every_script_in_order() {
    let manifest = manifest(TWO_SCRIPTS);
    let bridge = PageBridge::new(&manifest, |path| {
        Some(format!("mark(\"{path}\"); // trailing comment").into_bytes())
    })
    .expect("a bridge");
    let source = bridge.source(7);
    assert!(source.starts_with("(() => {\n\"use strict\";\nconst C = {"));
    assert!(source.contains("\"view\":7"));
    assert!(source.contains(&format!("\"binding\":\"{}\"", bridge.binding)));
    assert!(source.contains("Object.defineProperty(g, \"rbPlugin\""));
    let a = source.find("mark(\"page/a.js\")").expect("a");
    let b = source.find("mark(\"page/b.js\")").expect("b");
    assert!(a < b);
    assert!(source.contains("run(\"document-start\", function (rbPlugin) {\n"));
    assert!(source.contains("// trailing comment\n});"));
    assert!(source.ends_with("})();\n"));
}

#[test]
fn a_request_and_an_event_parse() {
    assert_eq!(
        parse_page_payload("s12:{\"a\":1}"),
        Ok(PageInput::Request {
            id: 12,
            message: json!({ "a": 1 })
        })
    );
    assert_eq!(
        parse_page_payload("e:[1,2]"),
        Ok(PageInput::Emit {
            message: json!([1, 2])
        })
    );
    assert_eq!(
        parse_page_payload("e:\"a:b\""),
        Ok(PageInput::Emit {
            message: json!("a:b")
        })
    );
}

#[test]
fn a_malformed_payload_is_refused_with_its_id_when_it_has_one() {
    assert_eq!(
        parse_page_payload("no colon"),
        Err(PageRefusal::Malformed { id: None })
    );
    assert_eq!(
        parse_page_payload("x1:{}"),
        Err(PageRefusal::Malformed { id: None })
    );
    assert_eq!(
        parse_page_payload("s:{}"),
        Err(PageRefusal::Malformed { id: None })
    );
    assert_eq!(
        parse_page_payload("s99999999999:{}"),
        Err(PageRefusal::Malformed { id: None })
    );
    assert_eq!(
        parse_page_payload("s3:{nope"),
        Err(PageRefusal::Malformed { id: Some(3) })
    );
}

#[test]
fn a_message_over_the_limit_is_refused_before_it_is_parsed() {
    let large = format!("\"{}\"", "x".repeat(MAX_MESSAGE_BYTES));
    assert_eq!(
        parse_page_payload(&format!("s4:{large}")),
        Err(PageRefusal::TooLarge { id: Some(4) })
    );
    assert_eq!(
        parse_page_payload(&format!("e:{large}")),
        Err(PageRefusal::TooLarge { id: None })
    );
    let fits = format!("\"{}\"", "x".repeat(MAX_MESSAGE_BYTES - 2));
    assert!(parse_page_payload(&format!("s5:{fits}")).is_ok());
}

#[test]
fn the_receiver_call_cannot_be_broken_out_of() {
    let call = receiver_call("__rbr_ab", &json!({ "post": "\");alert(1);(\"" }));
    assert!(call.starts_with("(()=>{const r=globalThis[\"__rbr_ab\"];"));
    assert!(call.contains(r#"r({"post":"\");alert(1);(\""})"#));
}

#[test]
fn replies_carry_the_value_or_the_code() {
    assert_eq!(
        reply_message(3, Ok(&json!({ "n": 1 }))),
        json!({ "reply": 3, "ok": true, "value": { "n": 1 } })
    );
    assert_eq!(
        reply_message(4, Err("plugins.error.too_large")),
        json!({ "reply": 4, "ok": false, "error": "plugins.error.too_large" })
    );
    assert_eq!(post_message(&json!(1)), json!({ "post": 1 }));
    assert_eq!(stop_message(), json!({ "stop": true }));
}

#[test]
fn the_component_request_names_the_view_and_the_message() {
    let args: Value =
        serde_json::from_str(&page_request_args(2, &json!({ "x": true }))).expect("JSON");
    assert_eq!(args, json!({ "view": 2, "message": { "x": true } }));
    assert_eq!(PAGE_MESSAGE_TOOL, "@page");
}

#[test]
fn posts_are_taken_out_of_an_answer_object() {
    let answer = json!({
        "count": 2,
        "$post": [
            { "target": "all", "message": { "show": true } },
            { "target": 3, "message": "three" },
            { "target": "panel", "message": [1] },
            { "target": "status", "message": { "key": "plugin.sample.notice.done" } },
            { "target": "host", "message": { "action": "add_design" } },
        ],
    });
    let (rest, posts, dropped) = split_posts(answer);
    assert_eq!(rest, json!({ "count": 2 }));
    assert_eq!(dropped, 0);
    assert_eq!(
        posts,
        vec![
            Post {
                target: PostTarget::All,
                message: json!({ "show": true })
            },
            Post {
                target: PostTarget::View(3),
                message: json!("three")
            },
            Post {
                target: PostTarget::Panel,
                message: json!([1])
            },
            Post {
                target: PostTarget::Status,
                message: json!({ "key": "plugin.sample.notice.done" })
            },
            Post {
                target: PostTarget::Host,
                message: json!({ "action": "add_design" })
            },
        ]
    );
}

#[test]
fn an_answer_without_posts_stays_as_it_is() {
    for answer in [json!({ "a": 1 }), json!([1]), json!("text"), Value::Null] {
        let (rest, posts, dropped) = split_posts(answer.clone());
        assert_eq!(rest, answer);
        assert!(posts.is_empty());
        assert_eq!(dropped, 0);
    }
}

#[test]
fn malformed_large_and_surplus_posts_are_dropped() {
    let mut entries = vec![
        json!({ "target": "elsewhere", "message": 1 }),
        json!({ "target": -1, "message": 1 }),
        json!("no object"),
        json!({ "message": 1 }),
        json!({ "target": "all", "message": "x".repeat(MAX_MESSAGE_BYTES) }),
    ];
    entries.extend((0..MAX_POSTS).map(|n| json!({ "target": "panel", "message": n })));
    let (_, posts, dropped) = split_posts(json!({ "$post": entries }));
    assert_eq!(posts.len(), MAX_POSTS - 5);
    assert_eq!(dropped, 10);
    let (rest, posts, dropped) = split_posts(json!({ "$post": "all" }));
    assert_eq!((rest, posts.len(), dropped), (json!({}), 0, 1));
}

// RB-080-66: what a page script logs goes to rBrowser over the binding (`l:`), never into the
// page's console.
#[test]
fn a_log_line_parses_into_its_level_and_texts() {
    assert_eq!(
        parse_page_payload(
            "l:{\"level\":\"warning\",\"values\":[\"[demo]\",\"plugins.error.busy\"]}"
        ),
        Ok(PageInput::Log(PageLog {
            level: LogLevel::Warning,
            values: vec!["[demo]".to_owned(), "plugins.error.busy".to_owned()],
            stack: None,
        }))
    );
    assert_eq!(
        parse_page_payload("l:{\"level\":\"shout\",\"values\":[]}"),
        Err(PageRefusal::Malformed { id: None })
    );
    assert_eq!(
        parse_page_payload("l:{\"level\":\"info\",\"values\":[1]}"),
        Err(PageRefusal::Malformed { id: None })
    );
    let large = format!(
        "{{\"level\":\"info\",\"values\":[\"{}\"]}}",
        "x".repeat(MAX_MESSAGE_BYTES)
    );
    assert_eq!(
        parse_page_payload(&format!("l:{large}")),
        Err(PageRefusal::TooLarge { id: None })
    );
}

#[test]
fn the_prologue_sends_the_worlds_console_to_rbrowser() {
    let manifest = manifest(TWO_SCRIPTS);
    let bridge = PageBridge::new(&manifest, |_| Some(b"x();".to_vec())).expect("a bridge");
    let source = bridge.source(1);
    assert!(source.contains("call(\"l:\" + "));
    // A request without an answer is given up as no answer, not as a stopped plugin.
    assert!(source.contains(&format!("\"{NO_ANSWER}\"")));
    assert!(!source.contains("plugins.error.timed_out"));
    assert!(source.contains(r#"for (const method of ["error", "warn", "info", "log", "debug"])"#));
    assert!(source.contains("own[method] = (...values) => log(method, values);"));
    // A page script's throw is reported with its stack, never printed.
    assert!(!source.contains("console.error("));
    assert!(source.contains("line.stack = cut(failed.stack, 4000)"));
}
