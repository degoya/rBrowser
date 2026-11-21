//! The prompts and resources end to end over Streamable HTTP (RB-095-01): only with the token;
//! `initialize` declares both; `prompts/list` gives the three prompts with their arguments,
//! `prompts/get responsive_review` the text with the URL in it; `resources/read
//! rbrowser://views` the views `views_list` answers; with `mcp.resources` off `resources/list`
//! is empty and a read is a JSON-RPC error naming the setting.

use std::sync::Arc;

use rb_mcp::testing::FakeBackend;
use rb_mcp::{Backend, McpHttpConfig, McpOptions, McpToken, serve_http};
use serde_json::{Value, json};

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .proxy(None)
        .http_status_as_error(false)
        .build()
        .new_agent()
}

/// POSTs `body` to `url` with `auth`; the status and the body.
fn post(url: &str, auth: Option<&str>, body: &Value) -> (u16, Value) {
    let mut request = agent()
        .post(url)
        .header("Accept", "application/json, text/event-stream")
        .content_type("application/json");
    if let Some(auth) = auth {
        request = request.header("Authorization", auth);
    }
    let mut response = request.send(body.to_string()).expect("an answer");
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn call(id: u32, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn initialize() -> Value {
    call(
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "rb-mcp-test", "version": "0" },
        }),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prompts_and_resources_over_http_with_the_token_only() {
    let token = McpToken::generate().expect("randomness");
    let fake = Arc::new(FakeBackend::default());
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
    let first = {
        let (url, bearer) = (url.clone(), bearer.clone());
        tokio::task::spawn_blocking(move || {
            let refused = post(&url, None, &call(2, "prompts/list", json!({}))).0;
            let init = post(&url, Some(&bearer), &initialize()).1;
            let prompts = post(&url, Some(&bearer), &call(3, "prompts/list", json!({}))).1;
            let prompt = post(
                &url,
                Some(&bearer),
                &call(
                    4,
                    "prompts/get",
                    json!({
                        "name": "responsive_review",
                        "arguments": { "url": "https://shop.test/", "breakpoints": "mobile" },
                    }),
                ),
            )
            .1;
            let views = post(
                &url,
                Some(&bearer),
                &call(5, "resources/read", json!({ "uri": "rbrowser://views" })),
            )
            .1;
            let tool = post(
                &url,
                Some(&bearer),
                &call(
                    6,
                    "tools/call",
                    json!({ "name": "views_list", "arguments": {} }),
                ),
            )
            .1;
            (refused, init, prompts, prompt, views, tool)
        })
        .await
        .expect("the requests ran")
    };
    let (refused, init, prompts, prompt, views, tool) = first;
    assert_eq!(refused, 401);
    let capabilities = &init["result"]["capabilities"];
    assert!(capabilities["prompts"].is_object(), "{init}");
    assert!(capabilities["resources"].is_object(), "{init}");
    let names: Vec<&str> = prompts["result"]["prompts"]
        .as_array()
        .expect("prompts")
        .iter()
        .filter_map(|prompt| prompt["name"].as_str())
        .collect();
    assert_eq!(names, ["responsive_review", "a11y_triage", "bug_repro"]);
    assert_eq!(
        prompts["result"]["prompts"][0]["arguments"][0]["name"],
        "url"
    );
    let text = prompt["result"]["messages"][0]["content"]["text"]
        .as_str()
        .expect("the prompt's text");
    assert!(text.contains("https://shop.test/"), "{text}");
    let read: Value = serde_json::from_str(
        views["result"]["contents"][0]["text"]
            .as_str()
            .expect("the views' text"),
    )
    .expect("JSON");
    let listed: Value = serde_json::from_str(
        tool["result"]["content"][0]["text"]
            .as_str()
            .expect("the tool's text"),
    )
    .expect("JSON");
    assert_eq!(read, listed);

    fake.set_options(McpOptions {
        resources: false,
        ..McpOptions::default()
    });
    let (list, read) = tokio::task::spawn_blocking(move || {
        let list = post(&url, Some(&bearer), &call(7, "resources/list", json!({}))).1;
        let read = post(
            &url,
            Some(&bearer),
            &call(8, "resources/read", json!({ "uri": "rbrowser://config" })),
        )
        .1;
        (list, read)
    })
    .await
    .expect("the requests ran");
    assert_eq!(list["result"]["resources"], json!([]), "{list}");
    assert_eq!(
        read["error"]["data"]["settingsKey"], "mcp.resources",
        "{read}"
    );
    assert_eq!(read["error"]["data"]["code"], "resources_disabled");
    handle.shutdown().await;
}
