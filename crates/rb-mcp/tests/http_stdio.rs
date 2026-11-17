//! The server end to end, in process: a request without or with a wrong token is refused with
//! 401, one with the token gets the tool list; `rbrowser mcp`'s forward carries a whole session
//! (initialize, a notification, tools/list, navigate with `target: "all"`) over stdio; the probe
//! tells the live instance; the token never reaches the log.

use std::sync::{Arc, Mutex, OnceLock};

use rb_mcp::instance::{self, Instance};
use rb_mcp::testing::FakeBackend;
use rb_mcp::{Backend, HttpHandle, McpHttpConfig, McpToken, serve_http, stdio};
use serde_json::{Value, json};

/// Everything the log got, at every level.
static LOG: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn capture_log() -> Arc<Mutex<Vec<u8>>> {
    LOG.get_or_init(|| {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = Capture(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
        buffer
    })
    .clone()
}

struct Running {
    handle: HttpHandle,
    token: McpToken,
    instance_file: std::path::PathBuf,
    fake: Arc<FakeBackend>,
    _dir: tempfile::TempDir,
}

async fn start() -> Running {
    let dir = tempfile::tempdir().expect("temp dir");
    let instance_file = instance::file_path(dir.path(), Some("project1"));
    let token = McpToken::generate().expect("randomness");
    let fake = Arc::new(FakeBackend::default());
    let backend: Arc<dyn Backend> = fake.clone();
    let handle = serve_http(
        backend,
        McpHttpConfig {
            port: 0,
            token: token.clone(),
            instance_file: Some(instance_file.clone()),
            project: "/home/me/site".to_owned(),
        },
    )
    .await
    .expect("the server starts");
    Running {
        handle,
        token,
        instance_file,
        fake,
        _dir: dir,
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .proxy(None)
        .http_status_as_error(false)
        .build()
        .new_agent()
}

/// POSTs `body` to `url` with `auth`; answers the status and the body.
fn post(url: &str, auth: Option<&str>, body: &Value) -> (u16, String) {
    let mut request = agent()
        .post(url)
        .header("Accept", "application/json, text/event-stream")
        .content_type("application/json");
    if let Some(auth) = auth {
        request = request.header("Authorization", auth);
    }
    let mut response = request.send(body.to_string()).expect("an answer");
    let status = response.status().as_u16();
    (
        status,
        response.body_mut().read_to_string().unwrap_or_default(),
    )
}

fn initialize(id: u32) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "rb-mcp-test", "version": "0" },
        },
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_or_with_a_wrong_token_401_with_the_token_the_tool_list() {
    let log = capture_log();
    let running = start().await;
    let url = running.handle.mcp_url();
    let bearer = format!("Bearer {}", running.token.expose());
    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
    let answers = tokio::task::spawn_blocking(move || {
        let none = post(&url, None, &list);
        let wrong = post(&url, Some(&format!("Bearer {}", "0".repeat(64))), &list);
        let init = post(&url, Some(&bearer), &initialize(1));
        let tools = post(&url, Some(&bearer), &list);
        (none, wrong, init, tools)
    })
    .await
    .expect("the requests ran");
    let (none, wrong, init, tools) = answers;
    assert_eq!(none.0, 401, "{}", none.1);
    assert_eq!(wrong.0, 401, "{}", wrong.1);
    assert_eq!(init.0, 200, "{}", init.1);
    assert_eq!(tools.0, 200, "{}", tools.1);
    let tools: Value = serde_json::from_str(&tools.1).expect("JSON");
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .expect("a tool list")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(names.contains(&"navigate"), "{names:?}");
    assert!(names.contains(&"views_list"), "{names:?}");
    let token = running.token.expose().to_owned();
    running.handle.shutdown().await;
    let text = String::from_utf8_lossy(&log.lock().expect("log").clone()).into_owned();
    assert!(!text.contains(&token), "the token reached the log");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_stdio_forward_carries_a_session_and_navigate_reports_every_final_url() {
    let log = capture_log();
    let running = start().await;
    let written = instance::read(&running.instance_file).expect("the instance file");
    assert_eq!(written.port, running.handle.addr().port());
    assert_eq!(written.project, "/home/me/site");
    let input = [
        initialize(1),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "navigate",
                "arguments": { "url": "https://example.test/", "target": "all" },
            },
        }),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    let output = tokio::task::spawn_blocking(move || {
        let instance: Instance = written;
        assert!(
            instance::probe(&instance),
            "the live instance answers the probe"
        );
        let mut output = Vec::new();
        stdio::forward(|| Ok(instance.clone()), input.as_bytes(), &mut output).expect("forwarded");
        output
    })
    .await
    .expect("the forward ran");
    let lines: Vec<Value> = String::from_utf8(output)
        .expect("UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON message per line"))
        .collect();
    let ids: Vec<&Value> = lines.iter().map(|line| &line["id"]).collect();
    assert_eq!(ids, vec![&json!(1), &json!(2), &json!(3)]);
    let call = &lines[2]["result"];
    assert_ne!(call["isError"], true, "{call}");
    let answer: Value =
        serde_json::from_str(call["content"][0]["text"].as_str().expect("text")).expect("JSON");
    assert_eq!(answer["settled"], true);
    let urls: Vec<&str> = answer["views"]
        .as_array()
        .expect("views")
        .iter()
        .filter_map(|view| view["url"].as_str())
        .collect();
    assert_eq!(urls, vec!["https://example.test/", "https://example.test/"]);
    assert_eq!(
        running.fake.url_of(2).as_deref(),
        Some("https://example.test/")
    );
    let token = running.token.expose().to_owned();
    let file = running.instance_file.clone();
    running.handle.shutdown().await;
    assert!(!file.exists(), "the instance file goes with the server");
    let text = String::from_utf8_lossy(&log.lock().expect("log").clone()).into_owned();
    assert!(!text.contains(&token), "the token reached the log");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stopped_server_fails_the_probe_and_the_bridge_answers_with_an_error() {
    let running = start().await;
    let written = instance::read(&running.instance_file).expect("the instance file");
    running.handle.shutdown().await;
    let output = tokio::task::spawn_blocking(move || {
        assert!(!instance::probe(&written));
        let mut output = Vec::new();
        let line = format!("{}\n", initialize(7));
        // The search finds the dead one again: the message is answered, the bridge goes on.
        stdio::forward(|| Ok(written.clone()), line.as_bytes(), &mut output)
            .expect("the bridge ends with its input");
        output
    })
    .await
    .expect("ran");
    let text = String::from_utf8(output).expect("UTF-8");
    let answer: Value = serde_json::from_str(text.trim()).expect("an error answer");
    assert_eq!(answer["id"], 7);
    assert!(answer["error"]["message"].as_str().is_some());
}
