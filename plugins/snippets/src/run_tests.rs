use rb_plugin_api::{Failure, FailureKind, codes};
use serde_json::{Value, json};

use super::{BUDGET_SECONDS, Job, RunTarget, Target, WORLD_NAME, resolve, run};
use crate::fake_host::{FakeHost, evaluated, view};
use crate::header::World;

fn host() -> FakeHost {
    FakeHost::with_views(vec![
        view(1, "Mobile"),
        view(2, "Tablet"),
        view(3, "Desktop"),
    ])
}

fn job(world: World) -> Job<'static> {
    Job {
        code: "return document.title",
        world,
        timeout_ms: 5000,
    }
}

fn title(text: &str) -> Value {
    json!({ "ok": true, "type": "string", "text": format!("\"{text}\""), "value": text })
}

#[test]
fn equal_results_are_one_with_their_views() {
    let mut host = host();
    for id in 1..=3 {
        let text = if id == 3 { "Desktop" } else { "Home" };
        host.answer(id, "Runtime.evaluate", Ok(evaluated(&title(text))));
    }
    let known = host.views.clone();
    let answer = run(&mut host, &known, &[1, 2, 3], &job(World::Main));
    assert_eq!(answer["views"], json!([1, 2, 3]));
    let results = answer["results"].as_array().expect("results");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["text"], "\"Home\"");
    assert_eq!(results[0]["value"], "Home");
    assert_eq!(results[0]["views"], json!([1, 2]));
    assert_eq!(results[1]["views"], json!([3]));
}

#[test]
fn the_main_world_is_the_default_context_with_the_snippets_timeout() {
    let mut host = host();
    let known = host.views.clone();
    run(&mut host, &known, &[1], &job(World::Main));
    assert_eq!(host.methods(), ["Runtime.evaluate"]);
    let params: Value = serde_json::from_str(&host.sent[0].2).expect("JSON");
    assert!(params.get("contextId").is_none(), "the page's own world");
    assert_eq!(params["timeout"], 5000);
    assert_eq!(params["awaitPromise"], true);
    assert_eq!(params["returnByValue"], true);
    assert!(
        params["expression"]
            .as_str()
            .expect("text")
            .contains("return document.title")
    );
}

#[test]
fn the_isolated_world_is_the_plugins_own_in_the_main_frame() {
    let mut host = host();
    host.answer(
        1,
        "Page.getFrameTree",
        Ok(r#"{"frameTree":{"frame":{"id":"F1"}}}"#.to_owned()),
    );
    host.answer(
        1,
        "Page.createIsolatedWorld",
        Ok(r#"{"executionContextId":42}"#.to_owned()),
    );
    let known = host.views.clone();
    run(&mut host, &known, &[1], &job(World::Isolated));
    assert_eq!(
        host.methods(),
        [
            "Page.getFrameTree",
            "Page.createIsolatedWorld",
            "Runtime.evaluate"
        ]
    );
    let world: Value = serde_json::from_str(&host.sent[1].2).expect("JSON");
    assert_eq!(world["frameId"], "F1");
    assert_eq!(world["worldName"], WORLD_NAME);
    let params: Value = serde_json::from_str(&host.sent[2].2).expect("JSON");
    assert_eq!(params["contextId"], 42);
}

#[test]
fn an_isolated_world_that_cannot_be_made_is_that_views_failure() {
    let mut host = host();
    let known = host.views.clone();
    let answer = run(&mut host, &known, &[1], &job(World::Isolated));
    assert_eq!(answer["results"][0]["ok"], false);
    assert_eq!(answer["results"][0]["text"], "plugin.snippets.errors.frame");
    assert_eq!(host.methods(), ["Page.getFrameTree"]);
}

#[test]
fn an_endless_loop_cut_by_the_timeout_says_so() {
    let mut host = host();
    host.answer(
        1,
        "Runtime.evaluate",
        Ok(json!({ "result": { "type": "object" }, "exceptionDetails": {
            "text": "Uncaught", "exception": { "description": "Error: Execution was terminated" } } })
        .to_string()),
    );
    host.answer(
        2,
        "Runtime.evaluate",
        Ok(evaluated(
            &json!({ "ok": false, "type": "error", "timedOut": true,
            "text": "Error: The snippet did not finish within 5000 ms." }),
        )),
    );
    let known = host.views.clone();
    let answer = run(&mut host, &known, &[1, 2], &job(World::Main));
    let results = answer["results"].as_array().expect("results");
    assert!(
        results.iter().all(|result| result["timedOut"] == true),
        "{results:?}"
    );
}

#[test]
fn a_syntax_error_is_the_exceptions_text() {
    let mut host = host();
    host.answer(
        1,
        "Runtime.evaluate",
        Ok(json!({ "exceptionDetails": { "text": "Uncaught",
            "exception": { "description": "SyntaxError: Unexpected token '}'" } } })
        .to_string()),
    );
    let known = host.views.clone();
    let answer = run(&mut host, &known, &[1], &job(World::Main));
    assert_eq!(answer["results"][0]["ok"], false);
    assert_eq!(answer["results"][0]["timedOut"], false);
    assert_eq!(
        answer["results"][0]["text"],
        "SyntaxError: Unexpected token '}'"
    );
}

#[test]
fn a_view_that_cannot_run_it_answers_its_code() {
    let mut host = host();
    host.views[1].engine = "firefox".to_owned();
    host.answer(
        1,
        "Runtime.evaluate",
        Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED).with_param("reason", "timeout")),
    );
    let known = host.views.clone();
    let answer = run(&mut host, &known, &[1, 2, 9], &job(World::Main));
    let results = answer["results"].as_array().expect("results");
    assert_eq!(results[0]["text"], codes::HOST_FAILED);
    assert_eq!(results[0]["timedOut"], true);
    assert_eq!(results[1]["text"], codes::NOT_AVAILABLE);
    assert_eq!(results[1]["views"], json!([2]));
    assert_eq!(results[2]["text"], "plugin.snippets.errors.unknown_view");
    assert_eq!(
        host.sent.len(),
        1,
        "nothing sent to a Firefox view or a gone one"
    );
}

#[test]
fn a_run_past_its_budget_leaves_the_remaining_views() {
    let mut host = host();
    host.tick = BUDGET_SECONDS / 2;
    let known = host.views.clone();
    let answer = run(&mut host, &known, &[1, 2, 3], &job(World::Main));
    let budget: Vec<&Value> = answer["results"]
        .as_array()
        .expect("results")
        .iter()
        .filter(|result| result["text"] == "plugin.snippets.errors.budget")
        .collect();
    assert_eq!(budget.len(), 1);
    assert!(
        !budget[0]["views"]
            .as_array()
            .expect("views")
            .contains(&json!(1))
    );
}

#[test]
fn targets_read_and_resolve() {
    assert_eq!(
        Target::parse(&Value::Null, RunTarget::Active),
        Ok(Target::Active)
    );
    assert_eq!(
        Target::parse(&json!("all"), RunTarget::Active),
        Ok(Target::All)
    );
    assert_eq!(
        Target::parse(&json!(2), RunTarget::All),
        Ok(Target::View(2))
    );
    assert_eq!(
        Target::parse(&json!([2, 1]), RunTarget::All),
        Ok(Target::Views(vec![2, 1]))
    );
    for bad in [json!("some"), json!(-1), json!([]), json!(["a"]), json!({})] {
        assert!(Target::parse(&bad, RunTarget::All).is_err(), "{bad}");
    }
    let mut host = host();
    host.active = Some(2);
    let known = host.views.clone();
    assert_eq!(resolve(&mut host, &known, &Target::All), Ok(vec![1, 2, 3]));
    assert_eq!(resolve(&mut host, &known, &Target::Active), Ok(vec![2]));
    assert_eq!(
        resolve(&mut host, &known, &Target::Views(vec![3, 3, 1])),
        Ok(vec![3, 1])
    );
    host.active = None;
    let failure = resolve(&mut host, &known, &Target::Active).expect_err("no active view");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.no_active_view")
    );
}
