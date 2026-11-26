use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::{Value, json};

use super::*;

/// A host that records every call and answers what the test set per tool.
#[derive(Clone, Default)]
struct FakeHost {
    calls: Rc<RefCell<Vec<(String, Value)>>>,
    answers: Rc<RefCell<BTreeMap<String, Result<String, String>>>>,
}

impl FakeHost {
    fn answer(self, tool: &str, answer: Result<Value, Value>) -> Self {
        let answer = answer
            .map(|value| value.to_string())
            .map_err(|value| value.to_string());
        self.answers.borrow_mut().insert(tool.to_owned(), answer);
        self
    }

    fn calls(&self) -> Vec<(String, Value)> {
        self.calls.borrow().clone()
    }
}

impl Host for FakeHost {
    fn call(&mut self, tool: &str, arguments: &str) -> Result<String, String> {
        let arguments = serde_json::from_str(arguments).unwrap_or(Value::Null);
        self.calls.borrow_mut().push((tool.to_owned(), arguments));
        self.answers
            .borrow()
            .get(tool)
            .cloned()
            .unwrap_or_else(|| Ok("null".to_owned()))
    }

    fn clock_ms(&self) -> u64 {
        1_700_000_000_000
    }
}

fn ok(code: &str, host: &FakeHost) -> Value {
    let text = run(code, "{}", "\"all\"", Box::new(host.clone())).expect("the script runs");
    serde_json::from_str(&text).expect("JSON")
}

fn err(code: &str, host: &FakeHost) -> Value {
    let text = run(code, "{}", "\"all\"", Box::new(host.clone())).expect_err("the script fails");
    serde_json::from_str(&text).expect("JSON")
}

#[test]
fn a_script_answers_what_it_returns_as_json() {
    let host = FakeHost::default();
    assert_eq!(
        ok("return 1 + 1;", &host),
        json!({ "value": 2, "logs": [] })
    );
    assert_eq!(ok("const x = 1;", &host)["value"], Value::Null);
    let text = run(
        "return [args.width, target];",
        r#"{"width":390}"#,
        "\"mobile\"",
        Box::new(host),
    )
    .expect("runs");
    let answer: Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(answer["value"], json!([390, "mobile"]));
}

#[test]
fn rb_calls_the_host_with_json_and_answers_the_parsed_result() {
    let host = FakeHost::default().answer("views_list", Ok(json!([{ "id": 1 }, { "id": 2 }])));
    let answer = ok(
        "const views = await rb.views_list({});\n\
         await rb.navigate({ target: 'all', url: 'https://a.test/' });\n\
         return views.map((view) => view.id);",
        &host,
    );
    assert_eq!(answer["value"], json!([1, 2]));
    assert_eq!(
        host.calls(),
        vec![
            ("views_list".to_owned(), json!({})),
            (
                "navigate".to_owned(),
                json!({ "target": "all", "url": "https://a.test/" })
            ),
        ]
    );
}

#[test]
fn a_refused_call_throws_a_tool_error_the_script_can_catch() {
    let refusal = json!({ "code": "evaluate_disabled", "message": "evaluate is switched off" });
    let host = FakeHost::default().answer("evaluate", Err(refusal));
    let answer = ok(
        "try { await rb.evaluate({ expression: '1' }); }\n\
         catch (error) { return [error.name, error.code, error.tool, error.message]; }",
        &host,
    );
    assert_eq!(
        answer["value"],
        json!([
            "ToolError",
            "evaluate_disabled",
            "evaluate",
            "evaluate is switched off"
        ])
    );
}

/// RB-099-16: a refusal with a renamed code carries the flat one as `legacyCode` until 1.0; a
/// refusal that is no JSON is `tool.failed`, formerly `tool_failed`.
#[test]
fn a_tool_error_carries_the_flat_code_of_a_renamed_one() {
    let refusal = json!({ "code": "evaluate.disabled", "legacyCode": "evaluate_disabled",
                          "message": "off" });
    let host = FakeHost::default().answer("evaluate", Err(refusal));
    let answer = ok(
        "try { await rb.evaluate({ expression: '1' }); }\n\
         catch (error) { return [error.code, error.legacyCode]; }",
        &host,
    );
    assert_eq!(
        answer["value"],
        json!(["evaluate.disabled", "evaluate_disabled"])
    );

    let kept = json!({ "code": "flows.untrusted", "message": "untrusted" });
    let host = FakeHost::default().answer("flow_run", Err(kept));
    let answer = ok(
        "try { await rb.flow_run({ name: 'a' }); }\n\
         catch (error) { return [error.code, error.legacyCode ?? null]; }",
        &host,
    );
    assert_eq!(answer["value"], json!(["flows.untrusted", null]));
}

#[test]
fn an_uncaught_error_comes_back_with_its_name_code_tool_and_line() {
    let refusal = json!({ "code": "flows.untrusted", "message": "untrusted" });
    let host = FakeHost::default().answer("flow_run", Err(refusal));
    let failure = err("const a = 1;\nawait rb.flow_run({ name: 'login' });", &host);
    assert_eq!(failure["code"], "flows.untrusted");
    assert_eq!(failure["tool"], "flow_run");
    assert_eq!(failure["name"], "ToolError");
    assert_eq!(failure["line"], 2, "{failure}");

    let thrown = err("\n\nthrow new TypeError('bad width');", &host);
    assert_eq!(thrown["code"], SCRIPT_ERROR);
    assert_eq!(thrown["name"], "TypeError");
    assert_eq!(thrown["message"], "bad width");
    assert_eq!(thrown["line"], 3, "{thrown}");
}

/// The code of an uncaught error without a code of its own (the prelude's).
const SCRIPT_ERROR: &str = "script.error";

#[test]
fn a_script_that_does_not_parse_is_a_syntax_failure_with_its_line() {
    let failure = err("const a = 1;\nconst = 2;", &FakeHost::default());
    assert_eq!(failure["code"], SYNTAX);
    assert_eq!(failure["name"], "SyntaxError");
    assert_eq!(failure["line"], 2, "{failure}");
}

#[test]
fn a_script_reaches_no_network_file_system_process_or_module() {
    let host = FakeHost::default();
    let answer = ok(
        "return [typeof fetch, typeof require, typeof process, typeof XMLHttpRequest,\n\
         typeof WebSocket, typeof Deno, typeof Bun, typeof setTimeout, typeof module,\n\
         typeof window, typeof document];",
        &host,
    );
    assert_eq!(answer["value"], Value::Array(vec![json!("undefined"); 11]));
    let imported = ok(
        "try { await import('fs'); return 'loaded'; } catch (error) { return error.name; }",
        &host,
    );
    assert_ne!(imported["value"], "loaded");
    assert_eq!(
        err("import fs from 'fs';", &host)["code"],
        SYNTAX,
        "a static import does not parse in a script"
    );
    assert!(host.calls().is_empty(), "nothing reached the host");
}

#[test]
fn a_promise_that_never_settles_is_unsettled() {
    let failure = err("await new Promise(() => {});", &FakeHost::default());
    assert_eq!(failure["code"], UNSETTLED);
}

#[test]
fn console_lines_come_back_beside_the_value() {
    let answer = ok(
        "console.log('views', 2, { a: 1 });\nconsole.warn('done');\nreturn true;",
        &FakeHost::default(),
    );
    assert_eq!(answer["logs"], json!(["views 2 {\"a\":1}", "done"]));
    assert_eq!(answer["value"], true);
}

#[test]
fn a_value_that_is_no_json_is_a_script_error() {
    let failure = err("return 10n;", &FakeHost::default());
    assert_eq!(failure["code"], SCRIPT_ERROR);
    assert_eq!(failure["name"], "TypeError");
}

#[test]
fn the_clock_is_the_hosts() {
    let answer = ok("return Date.now();", &FakeHost::default());
    assert_eq!(answer["value"], 1_700_000_000_000_u64);
}

#[test]
fn rb_and_console_cannot_be_replaced_and_the_natives_are_gone() {
    let answer = ok(
        "try { rb = 1; } catch {}\n\
         return [typeof rb.navigate, typeof __rb_call, typeof __rb_args, typeof rb.then];",
        &FakeHost::default(),
    );
    assert_eq!(
        answer["value"],
        json!(["function", "undefined", "undefined", "undefined"])
    );
}

#[test]
fn the_overflow_review_takes_one_answer_instead_of_a_round_trip_per_view() {
    let host = FakeHost::default().answer(
        "evaluate",
        // `evaluate` answers one entry per view.
        Ok(json!([
            { "view": 1, "breakpoint": "mobile", "value": { "scroll": 420, "inner": 390 } },
            { "view": 2, "breakpoint": "desktop", "value": { "scroll": 1280, "inner": 1280 } },
        ])),
    );
    let answer = ok(
        "await rb.navigate({ target: 'all', url: args.url ?? 'https://shop.test/' });\n\
         const views = await rb.evaluate({ target: 'all', expression:\n\
           '({ scroll: document.documentElement.scrollWidth, inner: innerWidth })' });\n\
         return views.filter((v) => v.value.scroll > v.value.inner).map((v) => v.breakpoint);",
        &host,
    );
    assert_eq!(answer["value"], json!(["mobile"]));
    assert_eq!(host.calls().len(), 2);
}

#[test]
fn the_line_of_a_parse_message_drops_the_wrapper_line() {
    assert_eq!(syntax_line("unexpected token at line 3, col 5"), Some(2));
    assert_eq!(syntax_line("unexpected token at line 1, col 5"), None);
    assert_eq!(syntax_line("no position"), None);
}

#[test]
fn the_console_keeps_at_most_64_kib_of_utf8() {
    // Two bytes each in UTF-8, one unit each in UTF-16: the cap counts the bytes.
    let answer = ok(
        "console.log('é'.repeat(70000));\nconsole.log('more');\nreturn 1;",
        &FakeHost::default(),
    );
    let lines = answer["logs"].as_array().expect("logs");
    let bytes: usize = lines
        .iter()
        .map(|line| line.as_str().map_or(0, str::len))
        .sum();
    assert!(bytes <= 65_536, "{bytes} bytes");
    assert!(bytes >= 65_534, "{bytes} bytes");
    assert_eq!(lines.len(), 1, "the cap is reached");
}
