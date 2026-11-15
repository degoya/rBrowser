use serde_json::{Value, json};

use super::*;

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON")
}

#[test]
fn greeting_posts_a_badge_to_every_view_and_a_line_to_the_panel() {
    let answer = parse(&greet("{}").expect("answers"));
    assert_eq!(answer["greeting"], "Hello, rBrowser!");
    assert_eq!(
        answer["$post"],
        json!([
            { "target": "all", "message": { "show": "Hello, rBrowser!" } },
            { "target": "panel", "message": { "greeted": "rBrowser" } },
        ])
    );
}

#[test]
fn a_target_view_gets_the_badge_alone_and_a_long_name_is_cut() {
    let name = "n".repeat(100);
    let args = json!({ "name": name, "target": 3 }).to_string();
    let answer = parse(&greet(&args).expect("answers"));
    assert_eq!(answer["$post"][0]["target"], 3);
    let greeted = answer["$post"][1]["message"]["greeted"]
        .as_str()
        .expect("a name");
    assert_eq!(greeted.chars().count(), MAX_NAME_CHARS);
    assert_eq!(
        parse(&greet(r#"{"target":"all"}"#).expect("answers"))["$post"][0]["target"],
        "all"
    );
}

#[test]
fn a_page_request_is_answered_and_the_page_goes_to_the_panel() {
    let args = json!({
        "view": 2,
        "message": { "url": "http://localhost:5173/", "title": "Home" },
    })
    .to_string();
    let answer = parse(&call_tool(PAGE_TOOL, &args).expect("answers"));
    assert_eq!(answer["greeting"], "Hello from the component, view 2");
    assert_eq!(
        answer["$post"],
        json!([{
            "target": "panel",
            "message": { "page": "http://localhost:5173/", "title": "Home", "view": 2 },
        }])
    );
}

#[test]
fn the_command_runs_the_tool_and_anything_else_is_refused() {
    assert_eq!(run_command("say_hello", "{}"), greet("{}"));
    assert_eq!(
        run_command("nope", "{}").expect_err("unknown").code,
        "plugins.error.unknown_contribution"
    );
    assert_eq!(
        call_tool("greet", "[1]").expect_err("no object").code,
        "plugins.error.invalid"
    );
    assert_eq!(
        call_tool(PAGE_TOOL, r#"{"message":{}}"#)
            .expect_err("no view")
            .code,
        "plugins.error.invalid"
    );
}
