//! `rbrowser mcp`'s bridge against servers in process (RB-030-02): a restart of rBrowser between
//! two calls is found again without restarting the client, the calls in between are answered
//! with an error, and stdout carries JSON-RPC alone; two instances without a project are listed;
//! the WSL share's two names find one project. And the breakpoint analyser's tools over HTTP
//! (RB-010-13): `breakpoints_detect` answers the proposal without a write, `breakpoints_apply`
//! writes.

use std::io::{BufReader, Write as _};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use rb_mcp::backend::ConfigScope;
use rb_mcp::stdio::{self, Locator, StdioError};
use rb_mcp::testing::FakeBackend;
use rb_mcp::{Backend, HttpHandle, McpHttpConfig, McpToken, instance, serve_http};
use serde_json::{Value, json};

async fn start(dir: &Path, key: &str, project: &str) -> (HttpHandle, Arc<FakeBackend>) {
    let fake = Arc::new(FakeBackend::default());
    let backend: Arc<dyn Backend> = fake.clone();
    let handle = serve_http(
        backend,
        McpHttpConfig {
            port: 0,
            token: McpToken::generate().expect("randomness"),
            instance_file: Some(instance::file_path(dir, Some(key))),
            project: project.to_owned(),
        },
    )
    .await
    .expect("the server starts");
    (handle, fake)
}

/// What the bridge wrote so far.
#[derive(Clone, Default)]
struct Out(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Out {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Out {
    fn lines(&self) -> Vec<String> {
        let bytes = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        String::from_utf8(bytes)
            .expect("UTF-8")
            .lines()
            .map(str::to_owned)
            .collect()
    }

    async fn wait_for(&self, count: usize) {
        for _ in 0..200 {
            if self.lines().len() >= count {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("{count} answers expected, got {:?}", self.lines());
    }
}

fn call(id: u32, name: &str, arguments: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    })
    .to_string()
}

/// The notification that the tool list changed.
fn list_changed() -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" })
}

fn initialize() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "rb-mcp-test", "version": "0" },
        },
    })
}

fn answers(out: &Out) -> Vec<Value> {
    out.lines()
        .iter()
        .map(|line| serde_json::from_str(line).expect("only JSON-RPC on stdout"))
        .collect()
}

/// F2 (RB-030-02): a client that connected while rBrowser was not running learns its tools: the
/// offline `initialize` promises `listChanged`, and the first message that reaches rBrowser is
/// followed by `notifications/tools/list_changed` - after the client said it is initialized.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reaching_rbrowser_after_an_offline_start_announces_the_tools() {
    let dir = tempfile::tempdir().expect("temp dir");
    let project = "/home/me/site";
    let locator = Locator::new(dir.path(), Some("late"), Some(project));
    let (reader, mut writer) = std::io::pipe().expect("a pipe");
    let out = Out::default();
    let bridge_out = out.clone();
    let bridge = std::thread::spawn(move || {
        stdio::forward(|| locator.find(), BufReader::new(reader), bridge_out)
    });
    writeln!(writer, "{}", initialize()).expect("written");
    out.wait_for(1).await;
    let (server, _) = start(dir.path(), "late", project).await;
    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    writeln!(writer, "{initialized}").expect("written");
    out.wait_for(2).await;
    writeln!(writer, "{}", call(2, "views_list", json!({}))).expect("written");
    out.wait_for(3).await;
    drop(writer);
    let ended = tokio::task::spawn_blocking(move || bridge.join())
        .await
        .expect("joined")
        .expect("the bridge did not panic");
    assert!(ended.is_ok(), "{ended:?}");
    server.shutdown().await;
    let answers = answers(&out);
    assert_eq!(answers[0]["id"], 1);
    assert_eq!(
        answers[0]["result"]["capabilities"]["tools"]["listChanged"],
        true
    );
    assert_eq!(answers[1], list_changed(), "{answers:?}");
    assert_eq!(answers[2]["id"], 2);
    assert_eq!(answers.len(), 3, "announced once: {answers:?}");
}

/// F2: plugin tools that appear after the client listed the tools (the plugin runtime started)
/// are announced after the next answer, once; the server's own `initialize` promises it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plugin_tools_that_appear_later_are_announced() {
    let dir = tempfile::tempdir().expect("temp dir");
    let project = "/home/me/site";
    let (server, fake) = start(dir.path(), "plug", project).await;
    let locator = Locator::new(dir.path(), Some("plug"), Some(project));
    let (reader, mut writer) = std::io::pipe().expect("a pipe");
    let out = Out::default();
    let bridge_out = out.clone();
    let bridge = std::thread::spawn(move || {
        stdio::forward(|| locator.find(), BufReader::new(reader), bridge_out)
    });
    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
    writeln!(writer, "{}\n{list}", initialize()).expect("written");
    out.wait_for(2).await;
    fake.set_plugin_tools(vec![rb_mcp::PluginToolInfo {
        name: "hello_greet".to_owned(),
        description: "Greets.".to_owned(),
        input_schema: json!({ "type": "object", "properties": {} }),
    }]);
    writeln!(writer, "{}", call(3, "views_list", json!({}))).expect("written");
    out.wait_for(4).await;
    writeln!(writer, "{}", call(4, "views_list", json!({}))).expect("written");
    out.wait_for(5).await;
    drop(writer);
    let ended = tokio::task::spawn_blocking(move || bridge.join())
        .await
        .expect("joined")
        .expect("the bridge did not panic");
    assert!(ended.is_ok(), "{ended:?}");
    server.shutdown().await;
    let answers = answers(&out);
    assert_eq!(
        answers[0]["result"]["capabilities"]["tools"]["listChanged"],
        true
    );
    assert_eq!(answers[1]["id"], 2);
    assert_eq!(answers[2]["id"], 3);
    assert_eq!(answers[3], list_changed(), "{answers:?}");
    assert_eq!(answers[4]["id"], 4);
    assert_eq!(answers.len(), 5, "announced once: {answers:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_restarted_instance_answers_the_next_call_and_stdout_stays_json_rpc() {
    let dir = tempfile::tempdir().expect("temp dir");
    let project = "/home/me/site";
    let (first, _) = start(dir.path(), "project1", project).await;
    let locator = Locator::new(dir.path(), Some("project1"), Some(project));
    let (reader, mut writer) = std::io::pipe().expect("a pipe");
    let out = Out::default();
    let bridge_out = out.clone();
    let bridge = std::thread::spawn(move || {
        stdio::forward(|| locator.find(), BufReader::new(reader), bridge_out)
    });
    let initialize = initialize();
    let views = json!({});
    writeln!(
        writer,
        "{initialize}\n{}",
        call(2, "views_list", views.clone())
    )
    .expect("written");
    out.wait_for(2).await;
    first.shutdown().await;
    writeln!(writer, "{}", call(3, "views_list", views.clone())).expect("written");
    out.wait_for(3).await;
    let (second, _) = start(dir.path(), "project1", project).await;
    writeln!(writer, "{}", call(4, "views_list", views)).expect("written");
    out.wait_for(4).await;
    drop(writer);
    let ended = tokio::task::spawn_blocking(move || bridge.join())
        .await
        .expect("joined")
        .expect("the bridge did not panic");
    assert!(ended.is_ok(), "{ended:?}");
    second.shutdown().await;
    let answers = answers(&out);
    let ids: Vec<&Value> = answers.iter().map(|answer| &answer["id"]).collect();
    // The new instance may serve other plugin tools: the client is told to list them again.
    assert_eq!(
        ids,
        vec![&json!(1), &json!(2), &json!(3), &json!(4), &Value::Null]
    );
    assert_eq!(answers[4], list_changed());
    assert!(answers[1]["result"].is_object(), "{}", answers[1]);
    let gone = answers[2]["error"]["message"].as_str().unwrap_or_default();
    assert!(gone.contains("no running rBrowser instance"), "{gone}");
    assert!(answers[3]["result"].is_object(), "{}", answers[3]);
    assert_ne!(answers[3]["result"]["isError"], true, "{}", answers[3]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_instances_without_a_project_are_listed_and_a_project_picks_its_own() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (a, _) = start(dir.path(), "a1", r"C:\sites\a").await;
    let (b, _) = start(dir.path(), "b2", r"\\wsl.localhost\Ubuntu\home\me\b").await;
    let local = dir.path().to_path_buf();
    let (several, picked) = tokio::task::spawn_blocking(move || {
        let several = Locator::new(&local, None, None).find();
        let picked = Locator::new(&local, Some("other"), Some(r"\\wsl$\Ubuntu\home\me\b\")).find();
        (several, picked)
    })
    .await
    .expect("ran");
    let Err(StdioError::Several(list)) = several else {
        panic!("several expected: {several:?}");
    };
    assert!(list.contains(r"C:\sites\a"), "{list}");
    assert!(list.contains(r"\\wsl.localhost\Ubuntu\home\me\b"), "{list}");
    assert_eq!(picked.expect("found").port, b.addr().port());
    a.shutdown().await;
    b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn breakpoints_detect_proposes_without_a_write_and_apply_writes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (server, fake) = start(dir.path(), "site", "/home/me/site").await;
    let proposal = json!({ "proposalId": 1, "breakpoints": [{ "id": "md", "width": 768 }] });
    fake.answer("breakpoints.analyze", Ok(proposal.clone()));
    fake.answer(
        "breakpoints.append_detected",
        Ok(json!({ "written": ["md"] })),
    );
    let locator = Locator::new(dir.path(), Some("site"), Some("/home/me/site"));
    let input = [
        call(
            1,
            "breakpoints_detect",
            json!({ "urls": ["https://a.test/"] }),
        ),
        call(
            2,
            "breakpoints_apply",
            json!({ "proposalId": 1, "selected": ["md"] }),
        ),
    ]
    .join("\n");
    let output = tokio::task::spawn_blocking(move || {
        let mut output = Vec::new();
        stdio::forward(|| locator.find(), input.as_bytes(), &mut output).expect("forwarded");
        output
    })
    .await
    .expect("ran");
    let answers: Vec<Value> = String::from_utf8(output)
        .expect("UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSON-RPC"))
        .collect();
    let text = |answer: &Value| -> Value {
        serde_json::from_str(
            answer["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or("null"),
        )
        .unwrap_or(Value::Null)
    };
    assert_eq!(text(&answers[0]), proposal);
    assert_eq!(text(&answers[1]), json!({ "written": ["md"] }));
    let ids: Vec<String> = fake.calls().into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, ["breakpoints.analyze", "breakpoints.append_detected"]);
    assert_eq!(
        fake.document(ConfigScope::Project),
        json!({}),
        "no config tool wrote"
    );
    server.shutdown().await;
}
