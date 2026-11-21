use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rb_plugin_host::{PageBridge, PageScriptSource, PluginSource, world_name};
use rb_plugin_types::{Failure, codes};
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tokio::sync::Notify;

use super::keep_bridges;
use crate::cdp::fake::FakeBrowser;
use crate::plugins::runtime::{Component, Plugin, PluginRuntime, Sinks};

/// A component whose tool answers `{}` once it is let go; it says when a call came in.
#[derive(Default)]
struct Held {
    entered: Notify,
    go: Notify,
    calls: AtomicUsize,
}

#[async_trait]
impl Component for Held {
    async fn page_message(&self, _view: u32, _message: &Value) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn run_command(&self, _command: &str, _args: &str) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn call_tool(&self, tool: &str, _args: &str) -> Result<String, Failure> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        if tool == "wait" {
            self.entered.notify_one();
            self.go.notified().await;
        }
        Ok(r#"{"done": true}"#.to_owned())
    }
}

fn bridge(id: &str, script: &str) -> Arc<PageBridge> {
    Arc::new(PageBridge {
        plugin_id: id.to_owned(),
        world: world_name(id),
        binding: format!("__rbp_{id}_{script}"),
        receiver: format!("__rbr_{id}_{script}"),
        scripts: vec![PageScriptSource {
            run_at: "document-start".to_owned(),
            path: "page/a.js".to_owned(),
            text: script.to_owned(),
        }],
    })
}

fn served(id: &str, component: Arc<dyn Component>, script: Option<&str>) -> Plugin {
    Plugin {
        id: id.to_owned(),
        source: PluginSource::Bundled,
        component,
        bridge: script.map(|script| bridge(id, script)),
        console: false,
        network: false,
        status: false,
    }
}

fn runtime(plugins: Vec<Plugin>) -> Arc<PluginRuntime> {
    let (_fake, router) = FakeBrowser::connect();
    PluginRuntime::new(
        plugins,
        router,
        Handle::current(),
        Sinks {
            panels: Arc::new(|_, _| {}),
            status: Arc::new(|_, _| {}),
            host: Arc::new(|_, _| {}),
        },
    )
}

#[test]
fn a_plugin_with_the_same_page_scripts_keeps_its_bridge() {
    // RB-092-06: a bridge's names are random per load; the scripts already in the views call the
    // old ones, so a plugin whose scripts did not change keeps its bridge across a reload.
    let component: Arc<dyn Component> = Arc::new(Held::default());
    let old = vec![
        served("same", component.clone(), Some("1;")),
        served("changed", component.clone(), Some("1;")),
    ];
    let new = vec![
        served("same", component.clone(), Some("1;")),
        served("changed", component.clone(), Some("2;")),
        served("added", component.clone(), Some("1;")),
        served("quiet", component, None),
    ];
    let fresh: Vec<Option<Arc<PageBridge>>> =
        new.iter().map(|plugin| plugin.bridge.clone()).collect();
    let kept = keep_bridges(&old, new);
    let bridge_of = |id: &str| {
        kept.iter()
            .find(|plugin| plugin.id == id)
            .and_then(|plugin| plugin.bridge.clone())
    };
    let same = bridge_of("same").expect("a bridge");
    assert!(Arc::ptr_eq(&same, old[0].bridge.as_ref().expect("old")));
    let changed = bridge_of("changed").expect("a bridge");
    assert!(Arc::ptr_eq(&changed, fresh[1].as_ref().expect("fresh")));
    assert_eq!(changed.scripts[0].text, "2;");
    let added = bridge_of("added").expect("a bridge");
    assert!(Arc::ptr_eq(&added, fresh[2].as_ref().expect("fresh")));
    assert!(bridge_of("quiet").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_to_a_plugin_removed_while_it_runs_ends_with_an_error() {
    let held = Arc::new(Held::default());
    let runtime = runtime(vec![served("gone", held.clone(), None)]);
    let call = {
        let runtime = runtime.clone();
        tokio::spawn(async move { runtime.call_tool("gone", "wait", &json!({})).await })
    };
    tokio::time::timeout(Duration::from_secs(10), held.entered.notified())
        .await
        .expect("the call came in");
    runtime.replace_plugins(Vec::new());
    assert!(!runtime.serves("gone"));
    held.go.notify_one();
    let failure = tokio::time::timeout(Duration::from_secs(10), call)
        .await
        .expect("the call ended")
        .expect("no panic")
        .expect_err("its plugin is gone");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
    // A new call is refused at once, without reaching the old component.
    let before = held.calls.load(Ordering::Acquire);
    let refused = runtime
        .call_tool("gone", "now", &json!({}))
        .await
        .expect_err("not served");
    assert_eq!(refused.code(), Some(codes::NOT_AVAILABLE));
    assert_eq!(held.calls.load(Ordering::Acquire), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_replaced_plugin_answers_from_its_new_component() {
    let (old, new) = (Arc::new(Held::default()), Arc::new(Held::default()));
    let runtime = runtime(vec![served("swap", old.clone(), None)]);
    runtime
        .call_tool("swap", "now", &json!({}))
        .await
        .expect("answers");
    runtime.replace_plugins(vec![served("swap", new.clone(), None)]);
    let answer = runtime
        .call_tool("swap", "now", &json!({}))
        .await
        .expect("answers");
    assert_eq!(answer, json!({ "done": true }));
    assert_eq!(old.calls.load(Ordering::Acquire), 1);
    assert_eq!(new.calls.load(Ordering::Acquire), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn calls_during_swaps_answer_or_fail_cleanly() {
    // Many calls while the plugin comes and goes: each answers or ends with `not_available`.
    let held: Arc<dyn Component> = Arc::new(Held::default());
    let runtime = runtime(vec![served("flip", held.clone(), None)]);
    let calls: Vec<_> = (0..64)
        .map(|_| {
            let runtime = runtime.clone();
            tokio::spawn(async move { runtime.call_tool("flip", "now", &json!({})).await })
        })
        .collect();
    for round in 0..64 {
        let plugins = if round % 2 == 0 {
            Vec::new()
        } else {
            vec![served("flip", held.clone(), None)]
        };
        runtime.replace_plugins(plugins);
        tokio::task::yield_now().await;
    }
    for call in calls {
        match call.await.expect("no panic") {
            Ok(answer) => assert_eq!(answer, json!({ "done": true })),
            Err(failure) => assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE)),
        }
    }
}
