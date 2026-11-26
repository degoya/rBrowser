use std::time::Duration;

use rb_plugin_host::{MAX_PENDING, PageScriptSource, world_name};
use serde_json::json;

use super::*;
use crate::cdp::fake::FakeBrowser;

/// A component answering every call with the text it was made with.
struct FakeComponent {
    answer: String,
}

#[async_trait]
impl Component for FakeComponent {
    async fn page_message(&self, view: u32, message: &Value) -> Result<String, Failure> {
        if self.answer == "echo" {
            return Ok(json!({ "view": view, "message": message }).to_string());
        }
        Ok(self.answer.clone())
    }

    async fn run_command(&self, _command: &str, _args: &str) -> Result<String, Failure> {
        Ok(self.answer.clone())
    }

    async fn call_tool(&self, tool: &str, _args: &str) -> Result<String, Failure> {
        if tool == "broken" {
            return Err(Failure::coded(
                FailureKind::Other,
                "plugin.hello.error.broken",
            ));
        }
        Ok(self.answer.clone())
    }
}

fn bridge(id: &str) -> Arc<PageBridge> {
    Arc::new(PageBridge {
        plugin_id: id.to_owned(),
        world: world_name(id),
        binding: format!("__rbp_{id}"),
        receiver: format!("__rbr_{id}"),
        scripts: vec![PageScriptSource {
            run_at: "document-start".to_owned(),
            path: "page/a.js".to_owned(),
            text: "1;".to_owned(),
        }],
    })
}

fn plugin(id: &str, source: PluginSource, answer: &str) -> Plugin {
    Plugin {
        id: id.to_owned(),
        source,
        component: Arc::new(FakeComponent {
            answer: answer.to_owned(),
        }),
        bridge: Some(bridge(id)),
        console: false,
        network: false,
        status: false,
    }
}

type Events = Arc<Mutex<Vec<(String, Value)>>>;

/// A panel limit as small as the page scripts' (`plugins.panelMessageMaxBytes` at its least).
const LIMIT: usize = rb_plugin_host::MAX_MESSAGE_BYTES;

fn setup(plugins: Vec<Plugin>) -> (Arc<FakeBrowser>, Events, Arc<PluginRuntime>) {
    let (fake, router) = FakeBrowser::connect();
    let events: Events = Arc::default();
    let sink = events.clone();
    let panels: PanelSink = Arc::new(move |plugin, event| {
        lock(&sink).push((plugin.to_owned(), event));
    });
    let status: NoticeSink = Arc::new(|_, _| {});
    let runtime = PluginRuntime::new(
        plugins,
        router,
        Handle::current(),
        Sinks {
            panels,
            status,
            host: Arc::new(|_, _| {}),
        },
    );
    (fake, events, runtime)
}

#[test]
fn page_scripts_run_unless_switched_off_and_a_project_plugins_only_under_trust() {
    let plugins = vec![
        plugin("hello", PluginSource::Bundled, "{}"),
        plugin("shared", PluginSource::Project, "{}"),
        Plugin {
            bridge: None,
            ..plugin("quiet", PluginSource::Bundled, "{}")
        },
    ];
    let ids = |bridges: Vec<Arc<PageBridge>>| -> Vec<String> {
        bridges
            .iter()
            .map(|bridge| bridge.plugin_id.clone())
            .collect()
    };
    let none = BTreeSet::new();
    assert_eq!(
        ids(enabled_bridges(&plugins, &none, true)),
        ["hello", "shared"]
    );
    assert_eq!(
        ids(enabled_bridges(&plugins, &none, false)),
        ["hello"],
        "no project plugin's page script without workspace trust"
    );
    let off = BTreeSet::from(["hello".to_owned()]);
    assert_eq!(ids(enabled_bridges(&plugins, &off, true)), ["shared"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_untrusted_project_plugin_injects_nothing() {
    let (fake, _, runtime) = setup(vec![plugin("shared", PluginSource::Project, "{}")]);
    runtime.set_enabled(&BTreeSet::new(), false);
    runtime.pages().attach(ViewId(1), "S1").await;
    assert!(fake.sent("Runtime.addBinding").is_empty());
    assert!(
        fake.sent("Page.addScriptToEvaluateOnNewDocument")
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_command_answers_without_its_posts_and_the_panel_post_reaches_the_shell() {
    let answer = json!({
        "done": true,
        "$post": [{ "target": "panel", "message": { "count": 2 } }],
    });
    let (_, events, runtime) = setup(vec![plugin(
        "hello",
        PluginSource::Bundled,
        &answer.to_string(),
    )]);
    let value = runtime
        .run_command("hello", "greet", &json!({}))
        .await
        .expect("answers");
    assert_eq!(value, json!({ "done": true }));
    assert_eq!(
        lock(&events).clone(),
        vec![("hello".to_owned(), json!({ "message": { "count": 2 } }))]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_post_to_all_views_reaches_the_plugins_page_scripts() {
    let answer = json!({ "$post": [{ "target": "all", "message": "show" }] });
    let (fake, _, runtime) = setup(vec![plugin(
        "hello",
        PluginSource::Bundled,
        &answer.to_string(),
    )]);
    runtime.set_enabled(&BTreeSet::new(), true);
    runtime.pages().attach(ViewId(1), "S1").await;
    fake.emit(json!({
        "method": "Runtime.executionContextCreated",
        "sessionId": "S1",
        "params": { "context": { "id": 5, "name": "rbplugin-hello" } },
    }));
    // The post reaches the page once the pages know the world's context: the tool is called
    // until its post shows (every call posts the same message).
    for _ in 0..200 {
        let value = runtime
            .call_tool("hello", "show", &json!({}))
            .await
            .expect("answers");
        assert_eq!(value, json!({}));
        if !fake.sent("Runtime.evaluate").is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let sent = fake.sent("Runtime.evaluate");
    assert_eq!(sent[0]["params"]["contextId"], 5);
    assert!(
        sent[0]["params"]["expression"]
            .as_str()
            .is_some_and(|text| text.contains(r#"{"post":"show"}"#))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_request_reaches_the_component_and_its_answer_comes_back() {
    let (_, _, runtime) = setup(vec![plugin("hello", PluginSource::Bundled, "echo")]);
    let answer = runtime
        .request("hello", ViewId(4), json!({ "q": 1 }))
        .await
        .expect("answers");
    assert_eq!(answer, json!({ "view": 4, "message": { "q": 1 } }));
}

#[tokio::test(flavor = "multi_thread")]
async fn too_many_waiting_requests_are_refused_as_busy() {
    let (_, _, runtime) = setup(vec![plugin("hello", PluginSource::Bundled, "echo")]);
    let slots: Vec<_> = (0..MAX_PENDING)
        .map(|_| runtime.take_slot("hello").expect("a slot"))
        .collect();
    let refused = runtime.request("hello", ViewId(1), json!(1)).await;
    assert_eq!(refused, Err(BUSY.to_owned()));
    drop(slots);
    assert!(runtime.request("hello", ViewId(1), json!(1)).await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_event_goes_to_the_panels_with_its_view() {
    let (_, events, runtime) = setup(vec![plugin("hello", PluginSource::Bundled, "{}")]);
    runtime.emit("hello", ViewId(2), json!("log line"));
    assert_eq!(
        lock(&events).clone(),
        vec![(
            "hello".to_owned(),
            json!({ "view": 2, "message": "log line" })
        )]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn failures_keep_their_codes() {
    let (_, _, runtime) = setup(vec![plugin("hello", PluginSource::Bundled, "not json")]);
    let unknown = runtime
        .call_tool("nobody", "x", &json!({}))
        .await
        .expect_err("not served");
    assert_eq!(unknown.code(), Some(codes::NOT_AVAILABLE));
    let broken = runtime
        .call_tool("hello", "broken", &json!({}))
        .await
        .expect_err("fails");
    assert_eq!(broken.code(), Some("plugin.hello.error.broken"));
    let bad = runtime
        .run_command("hello", "x", &json!({}))
        .await
        .expect_err("no JSON");
    assert_eq!(bad.code(), Some(codes::BAD_OUTPUT));
    assert_eq!(
        runtime.request("hello", ViewId(1), json!(1)).await,
        Err(codes::BAD_OUTPUT.to_owned())
    );
}

#[tokio::test]
async fn an_answer_over_the_message_limit_is_refused_for_the_page_and_the_panel() {
    let big = json!({ "text": "x".repeat(rb_plugin_host::MAX_MESSAGE_BYTES) }).to_string();
    let (_fake, _events, runtime) = setup(vec![plugin("hello", PluginSource::Bundled, &big)]);
    assert_eq!(
        runtime.request("hello", ViewId(1), json!(1)).await,
        Err(codes::TOO_LARGE.to_owned())
    );
    // RB-090-44: a panel's limit is its setting; at the page scripts' 64 KiB it refuses too.
    let refused = runtime
        .panel_call_tool("hello", "greet", &json!({}), LIMIT)
        .await
        .expect_err("too large for a panel");
    assert_eq!(refused.code(), Some(codes::TOO_LARGE));
    // MCP and the palette take the whole answer.
    assert!(
        runtime
            .call_tool("hello", "greet", &json!({}))
            .await
            .is_ok()
    );
    assert!(
        runtime
            .run_command("hello", "say", &json!({}))
            .await
            .is_ok()
    );
}

/// Answers every call with the arguments it got.
struct EchoArgs;

#[async_trait]
impl Component for EchoArgs {
    async fn page_message(&self, _view: u32, _message: &Value) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn run_command(&self, _command: &str, args: &str) -> Result<String, Failure> {
        Ok(args.to_owned())
    }

    async fn call_tool(&self, _tool: &str, args: &str) -> Result<String, Failure> {
        Ok(args.to_owned())
    }
}

#[tokio::test]
async fn only_a_plugin_granted_console_logs_gets_the_hosts_feed() {
    let echo = |id: &str, console: bool| Plugin {
        id: id.to_owned(),
        source: PluginSource::Bundled,
        component: Arc::new(EchoArgs),
        bridge: None,
        console,
        network: false,
        status: false,
    };
    let (_fake, _events, runtime) = setup(vec![echo("console", true), echo("other", false)]);
    let forged = json!({ "$console": { "views": [{ "view": 9 }] }, "level": "error" });
    let fed = runtime
        .call_tool("console", "list", &forged)
        .await
        .expect("answered");
    assert_eq!(
        fed["$console"],
        json!({ "views": [] }),
        "the host's, not the caller's"
    );
    assert_eq!(fed["level"], "error");
    let fed = runtime
        .run_command("console", "show", &json!({}))
        .await
        .expect("answered");
    assert!(fed["$console"]["views"].is_array());
    let plain = runtime
        .call_tool("other", "list", &json!({ "level": "error" }))
        .await
        .expect("answered");
    assert_eq!(plain, json!({ "level": "error" }));
}

/// Answers every call with its arguments and `@rules` with one block rule.
struct NetworkPlugin;

#[async_trait]
impl Component for NetworkPlugin {
    async fn page_message(&self, _view: u32, _message: &Value) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn run_command(&self, _command: &str, args: &str) -> Result<String, Failure> {
        Ok(args.to_owned())
    }

    async fn call_tool(&self, _tool: &str, args: &str) -> Result<String, Failure> {
        Ok(args.to_owned())
    }

    async fn network_rules(&self, _args: &str) -> Result<String, Failure> {
        Ok(json!({ "rules": [{ "id": "b", "origin": "settings",
            "match": { "urlPattern": "*/tracking.js" }, "action": { "type": "block" } }] })
        .to_string())
    }
}

#[tokio::test]
async fn a_plugin_granted_network_rules_gets_the_state_and_its_rules_apply_after_a_call() {
    let network = Plugin {
        id: "network-panel".to_owned(),
        source: PluginSource::Bundled,
        component: Arc::new(NetworkPlugin),
        bridge: None,
        console: false,
        network: true,
        status: false,
    };
    let (_fake, _events, runtime) = setup(vec![network]);
    let rules = Arc::new(crate::network_rules::state::Rules::new(
        Box::new(Vec::new),
        "s".to_owned(),
    ));
    runtime.set_network_rules(Arc::clone(&rules)).await;
    assert_eq!(rules.active()[0].1.len(), 1, "asked at once");
    rules.apply("network-panel", &json!({})).expect("reads");
    let fed = runtime
        .call_tool(
            "network-panel",
            "requests",
            &json!({ "$network": "forged" }),
        )
        .await
        .expect("answered");
    assert_eq!(
        fed["$network"]["session"], "s",
        "the host's, not the caller's"
    );
    assert_eq!(rules.active()[0].1.len(), 1, "asked again after the call");
}

#[tokio::test]
async fn a_project_plugins_network_rules_need_trust_and_go_when_it_is_revoked() {
    let network = Plugin {
        id: "shared-mocks".to_owned(),
        source: PluginSource::Project,
        component: Arc::new(NetworkPlugin),
        bridge: None,
        console: false,
        network: true,
        status: false,
    };
    let (_fake, _events, runtime) = setup(vec![network]);
    let rules = Arc::new(crate::network_rules::state::Rules::new(
        Box::new(Vec::new),
        "s".to_owned(),
    ));
    rules.set_trusted(true);
    runtime.set_network_rules(Arc::clone(&rules)).await;
    assert!(
        rules.active().iter().all(|(_, rules)| rules.is_empty()),
        "not asked before the workspace is trusted"
    );
    runtime.set_enabled(&BTreeSet::new(), true);
    runtime.sync_all_network().await;
    let active = rules.active();
    assert_eq!(active[0].1.len(), 1, "asked under trust");
    assert_eq!(
        active[0].1[0].origin,
        crate::network_rules::rule::Origin::Project,
        "the host sets the origin, not the plugin's \"settings\""
    );

    // Revoked: the rules go, though the plugin called its block rule the person's own.
    runtime.set_enabled(&BTreeSet::new(), false);
    runtime.sync_all_network().await;
    assert!(
        rules.active().iter().all(|(_, rules)| rules.is_empty()),
        "trust revoked: no rule of the project plugin applies (review M2, Charge 7)"
    );
}

#[test]
fn a_project_plugin_may_run_only_under_workspace_trust() {
    assert!(may_run(PluginSource::Bundled, false));
    assert!(may_run(PluginSource::User, false));
    assert!(may_run(PluginSource::Project, true));
    assert!(!may_run(PluginSource::Project, false));
    let entries = [
        ("bundled", PluginSource::Bundled),
        ("shared", PluginSource::Project),
    ];
    let ids = |trusted| -> Vec<&str> {
        runnable(&entries, |entry| entry.1, trusted)
            .map(|entry| entry.0)
            .collect()
    };
    assert_eq!(ids(false), ["bundled"], "a check source drops out too");
    assert_eq!(ids(true), ["bundled", "shared"]);
}

/// Every way into a project plugin's component refuses without trust (review M2, Charge 6).
async fn assert_stopped(runtime: &PluginRuntime) {
    let untrusted = Some(codes::UNTRUSTED_WORKSPACE);
    assert!(!runtime.serves("shared"));
    let refused = runtime.run_command("shared", "x", &json!({})).await;
    assert_eq!(refused.expect_err("command").code(), untrusted);
    let refused = runtime.call_tool("shared", "x", &json!({})).await;
    assert_eq!(refused.expect_err("tool").code(), untrusted);
    let refused = runtime
        .panel_run_command("shared", "x", &json!({}), LIMIT)
        .await;
    assert_eq!(refused.expect_err("panel command").code(), untrusted);
    let refused = runtime
        .panel_call_tool("shared", "x", &json!({}), LIMIT)
        .await;
    assert_eq!(refused.expect_err("panel tool").code(), untrusted);
    assert_eq!(
        runtime.request("shared", ViewId(1), json!(1)).await,
        Err(codes::UNTRUSTED_WORKSPACE.to_owned()),
        "a page request"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_project_plugin_stops_entirely_while_the_workspace_is_not_trusted() {
    let (_fake, _events, runtime) = setup(vec![
        plugin("hello", PluginSource::Bundled, "{}"),
        plugin("shared", PluginSource::Project, "{}"),
    ]);
    // Before the first switch the runtime trusts nothing.
    assert_stopped(&runtime).await;
    assert!(runtime.serves("hello"));
    assert!(runtime.call_tool("hello", "x", &json!({})).await.is_ok());

    runtime.set_enabled(&BTreeSet::new(), true);
    assert!(runtime.serves("shared"));
    assert!(runtime.call_tool("shared", "x", &json!({})).await.is_ok());
    assert!(runtime.request("shared", ViewId(1), json!(1)).await.is_ok());

    // Revoked, or stale after a change: everything stops at once.
    runtime.set_enabled(&BTreeSet::new(), false);
    assert_stopped(&runtime).await;
    assert!(!runtime.serves("nobody"));
}

#[path = "runtime_limit_tests.rs"]
mod limit;
