//! `script_run` end to end over Streamable HTTP (RB-098-01), with the JSON-program sandbox of
//! `rb_mcp::testing`: off by default; on, a script's calls run through the session's router -
//! navigate and evaluate of every view in one answer - and through every guard of a direct call
//! (`evaluate_disabled`, `flows.untrusted`, the protected keys of `config_set` and
//! `settings_set`); `script_run` inside a script, too many calls, an endless run and a script
//! that does not parse end with their codes; a large answer is cut; the tool list carries the
//! `rb.*` signatures.

use std::sync::Arc;
use std::time::Duration;

use rb_mcp::testing::{FakeBackend, FakeScripts};
use rb_mcp::{
    Backend, ConfigScope, HttpHandle, McpHttpConfig, McpOptions, McpToken, ScriptOptions,
    serve_http,
};
use serde_json::{Value, json};

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .proxy(None)
        .http_status_as_error(false)
        .build()
        .new_agent()
}

fn post(url: &str, bearer: &str, body: &Value) -> Value {
    let mut response = agent()
        .post(url)
        .header("Accept", "application/json, text/event-stream")
        .header("Authorization", bearer)
        .content_type("application/json")
        .send(body.to_string())
        .expect("an answer");
    let text = response.body_mut().read_to_string().unwrap_or_default();
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

fn call(id: u32, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

struct Running {
    handle: HttpHandle,
    url: String,
    bearer: String,
    fake: Arc<FakeBackend>,
}

async fn start() -> Running {
    let token = McpToken::generate().expect("randomness");
    let fake = Arc::new(FakeBackend::default());
    fake.set_script_runner(Arc::new(FakeScripts::default()));
    let backend: Arc<dyn Backend> = fake.clone();
    let handle = serve_http(
        backend,
        McpHttpConfig {
            port: 0,
            token: token.clone(),
            instance_file: None,
            project: "/home/me/site".to_owned(),
        },
    )
    .await
    .expect("the server starts");
    let url = handle.mcp_url();
    let bearer = format!("Bearer {}", token.expose());
    let init = call(
        1,
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "rb-mcp-test", "version": "0" } }),
    );
    let (u, b) = (url.clone(), bearer.clone());
    tokio::task::spawn_blocking(move || post(&u, &b, &init))
        .await
        .expect("initialized");
    Running {
        handle,
        url,
        bearer,
        fake,
    }
}

fn scripts(fake: &FakeBackend, scripts: ScriptOptions, allow_evaluate: bool) {
    fake.set_options(McpOptions {
        allow_evaluate,
        navigation_timeout: Duration::from_millis(500),
        scripts,
        ..McpOptions::default()
    });
}

fn on() -> ScriptOptions {
    ScriptOptions {
        enabled: true,
        ..ScriptOptions::default()
    }
}

/// `script_run` with `program` (the fake's JSON script): is-error and the answer's JSON.
async fn run(running: &Running, program: Value) -> (bool, Value) {
    let (url, bearer) = (running.url.clone(), running.bearer.clone());
    let request = call(
        7,
        "tools/call",
        json!({ "name": "script_run", "arguments": { "code": program.to_string() } }),
    );
    let answer = tokio::task::spawn_blocking(move || post(&url, &bearer, &request))
        .await
        .expect("the request ran");
    let result = &answer["result"];
    let text = result["content"][0]["text"].as_str().unwrap_or("null");
    (
        result["isError"].as_bool().unwrap_or(false),
        serde_json::from_str(text).unwrap_or(Value::Null),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn script_run_over_http_bundles_calls_behind_every_guard() {
    let running = start().await;
    let fake = &running.fake;

    let (error, off) = run(&running, json!({ "return": 1 })).await;
    assert!(error);
    assert_eq!(off["code"], "scripts_disabled");
    assert_eq!(off["settingsKey"], "mcp.scripts.enabled");

    // Navigate every view and read each page's width: one answer for all of them.
    scripts(fake, on(), true);
    fake.answer_cdp(
        "Runtime.evaluate",
        json!({ "result": { "type": "boolean", "value": true } }),
    );
    let (error, overflow) = run(
        &running,
        json!({ "calls": [
            ["navigate", { "target": "all", "url": "https://shop.test/" }],
            ["evaluate", { "target": "all",
                           "expression": "document.documentElement.scrollWidth > innerWidth" }],
        ], "return": "answers" }),
    )
    .await;
    assert!(!error, "{overflow}");
    assert_eq!(overflow["calls"], 2);
    assert_eq!(
        overflow["result"][1].as_array().map(Vec::len),
        Some(2),
        "{overflow}"
    );
    assert_eq!(fake.url_of(1).as_deref(), Some("https://shop.test/"));
    let evaluated = fake
        .cdp_calls()
        .iter()
        .filter(|(_, method, _)| method == "Runtime.evaluate")
        .count();
    assert_eq!(evaluated, 2, "evaluate ran in both views");

    // The guards hold inside a script exactly as outside.
    scripts(fake, on(), false);
    let (_, evaluate) = run(
        &running,
        json!({ "calls": [["evaluate", { "expression": "1" }]] }),
    )
    .await;
    assert_eq!(evaluate["code"], "evaluate_disabled");
    assert_eq!(evaluate["tool"], "evaluate");
    let (_, flow) = run(
        &running,
        json!({ "calls": [["flow_run", { "name": "login" }]] }),
    )
    .await;
    assert_eq!(flow["code"], "flows.untrusted");
    assert!(fake.flow_runs().is_empty());
    let (_, config) = run(
        &running,
        json!({ "calls": [["config_set", { "key": "api-client.allowedHosts",
                                           "value": ["attacker.example"], "scope": "local" }]] }),
    )
    .await;
    assert_eq!(config["code"], "config_key_protected");
    assert_eq!(fake.document(ConfigScope::Local), json!({}));
    let (_, setting) = run(
        &running,
        json!({ "calls": [["settings_set", { "key": "mcp.allowEvaluate", "value": true }]] }),
    )
    .await;
    assert_eq!(setting["code"], "settings.rejected.protected");
    assert!(fake.written().is_empty());

    // The script's own limits.
    let (_, recursion) = run(
        &running,
        json!({ "calls": [["script_run", { "code": "{}" }]] }),
    )
    .await;
    assert_eq!(recursion["code"], "script.recursion");
    scripts(
        fake,
        ScriptOptions {
            max_calls: 2,
            ..on()
        },
        false,
    );
    let (_, limit) = run(
        &running,
        json!({ "calls": [["views_list", {}], ["views_list", {}], ["views_list", {}]],
                "catch": true, "return": "answers" }),
    )
    .await;
    assert_eq!(limit["code"], "script.call_limit");
    assert_eq!(limit["calls"], 2);
    scripts(
        fake,
        ScriptOptions {
            timeout: Duration::from_millis(200),
            ..on()
        },
        false,
    );
    let (_, endless) = run(&running, json!({ "loop": true })).await;
    assert_eq!(endless["code"], "script.timeout");
    assert_eq!(endless["settingsKey"], "mcp.scripts.timeoutMs");

    scripts(
        fake,
        ScriptOptions {
            max_result_bytes: 1024,
            ..on()
        },
        false,
    );
    let (error, big) = run(&running, json!({ "return": "x".repeat(5000) })).await;
    assert!(!error);
    assert_eq!(big["result"]["truncated"], true);
    assert!(big["note"].is_string());

    let (url, bearer) = (running.url.clone(), running.bearer.clone());
    let (syntax, list) = tokio::task::spawn_blocking(move || {
        let syntax = post(
            &url,
            &bearer,
            &call(
                8,
                "tools/call",
                json!({ "name": "script_run", "arguments": { "code": "not json" } }),
            ),
        );
        let list = post(&url, &bearer, &call(9, "tools/list", json!({})));
        (syntax, list)
    })
    .await
    .expect("the requests ran");
    let syntax: Value = serde_json::from_str(
        syntax["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("null"),
    )
    .unwrap_or(Value::Null);
    assert_eq!(syntax["code"], "script.syntax");
    let description = list["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == "script_run"))
        .and_then(|tool| tool["description"].as_str())
        .unwrap_or_default()
        .to_owned();
    assert!(description.contains("rb.navigate({"), "{description}");
    running.handle.shutdown().await;
}
