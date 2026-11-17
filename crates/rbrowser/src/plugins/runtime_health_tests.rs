//! RB-090-59 (diag14): a long call of one plugin - a Lighthouse run over every breakpoint, one
//! call for minutes - never holds another plugin's call slot, so the other plugins' page
//! requests keep their answers, nobody is given up and nobody is suspended; and the long run
//! itself counts as no failure.

use std::time::Duration;

use rb_plugin_host::{PAGE_TIMEOUT_MILLISECONDS, PluginHealth, PluginHealths};
use serde_json::json;
use tokio::sync::Notify;

use super::*;
use crate::cdp::fake::FakeBrowser;

/// A component behind a real [`PluginHealth`], as [`LoadedPlugin`] is: each call checks the
/// suspension and takes a slot of its `max_concurrent`; the tool `run` holds its slot until
/// `release`, and a request the host gave up counts as a failure.
struct Gated {
    health: Arc<PluginHealth>,
    max_concurrent: u32,
    release: Arc<Notify>,
}

impl Gated {
    async fn answer(&self, long: bool) -> Result<String, Failure> {
        self.health.check()?;
        let _slot = self.health.enter(self.max_concurrent).await;
        if long {
            self.release.notified().await;
        }
        Ok("{\"ok\":true}".to_owned())
    }
}

#[async_trait]
impl Component for Gated {
    async fn page_message(&self, _view: u32, _message: &Value) -> Result<String, Failure> {
        self.answer(false).await
    }

    async fn run_command(&self, _command: &str, _args: &str) -> Result<String, Failure> {
        self.answer(false).await
    }

    async fn call_tool(&self, tool: &str, _args: &str) -> Result<String, Failure> {
        self.answer(tool == "run").await
    }

    fn gave_up(&self) {
        self.health.record_failure();
    }

    fn busy(&self) -> bool {
        self.health.busy(self.max_concurrent)
    }
}

fn served(id: &str, component: Arc<Gated>) -> Plugin {
    Plugin {
        id: id.to_owned(),
        source: PluginSource::Bundled,
        component,
        bridge: None,
        console: false,
        network: false,
        status: false,
    }
}

#[tokio::test(start_paused = true)]
async fn a_long_call_of_one_plugin_leaves_the_page_requests_of_another_answered() {
    let book = PluginHealths::new(3);
    let release = Arc::new(Notify::new());
    let gated = |id: &str, max_concurrent| {
        Arc::new(Gated {
            health: book.of(id),
            max_concurrent,
            release: Arc::clone(&release),
        })
    };
    // As the manifests: lighthouse four calls at once, rulers-guides the default one.
    let lighthouse = gated("lighthouse", 4);
    let rulers = gated("rulers-guides", 1);
    let (_fake, router) = FakeBrowser::connect();
    let runtime = PluginRuntime::new(
        vec![
            served("lighthouse", Arc::clone(&lighthouse)),
            served("rulers-guides", Arc::clone(&rulers)),
        ],
        router,
        Handle::current(),
        Sinks {
            panels: Arc::new(|_, _| {}),
            status: Arc::new(|_, _| {}),
            host: Arc::new(|_, _| {}),
        },
    );
    let run = tokio::spawn({
        let runtime = Arc::clone(&runtime);
        async move { runtime.call_tool("lighthouse", "run", &json!({})).await }
    });
    // A bounded wait until the run holds its slot.
    for _ in 0..1_000 {
        if book.of("lighthouse").busy(1) {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(book.of("lighthouse").busy(1), "the run holds its slot");

    // The run goes on past several page timeouts (paused time: no real wait).
    let page_time = Duration::from_millis(u64::from(PAGE_TIMEOUT_MILLISECONDS));
    tokio::time::sleep(page_time * 3).await;
    for view in 1..=4 {
        let answer = runtime
            .request("rulers-guides", ViewId(view), json!({ "op": "layout" }))
            .await;
        assert_eq!(answer, Ok(json!({ "ok": true })), "view {view}");
    }
    assert!(!rulers.busy(), "its slot is free again");
    // The run's own panel still asks for its status while the run goes on.
    let status = runtime
        .panel_call_tool("lighthouse", "status", &json!({}), 64 * 1024)
        .await
        .expect("the status answers during the run");
    assert_eq!(status, json!({ "ok": true }));

    release.notify_one();
    let done = run.await.expect("joined").expect("the run answers");
    assert_eq!(done, json!({ "ok": true }));
    assert!(
        book.suspended().is_empty(),
        "nobody suspended, a long run is no failure: {:?}",
        book.suspended()
    );
}

/// The other half of diag14: one plugin suspended, the others' calls still run.
#[tokio::test]
async fn a_suspended_plugin_does_not_suspend_the_others() {
    let book = PluginHealths::new(1);
    let release = Arc::new(Notify::new());
    let gated = |id: &str| {
        Arc::new(Gated {
            health: book.of(id),
            max_concurrent: 1,
            release: Arc::clone(&release),
        })
    };
    let (_fake, router) = FakeBrowser::connect();
    let runtime = PluginRuntime::new(
        vec![
            served("rulers-guides", gated("rulers-guides")),
            served("lighthouse", gated("lighthouse")),
            served("todo", gated("todo")),
        ],
        router,
        Handle::current(),
        Sinks {
            panels: Arc::new(|_, _| {}),
            status: Arc::new(|_, _| {}),
            host: Arc::new(|_, _| {}),
        },
    );
    assert!(book.of("rulers-guides").record_failure(), "suspended");
    let refused = runtime
        .call_tool("rulers-guides", "list", &json!({}))
        .await
        .expect_err("suspended");
    assert_eq!(refused.code(), Some(codes::SUSPENDED));
    for plugin in ["lighthouse", "todo"] {
        runtime
            .call_tool(plugin, "status", &json!({}))
            .await
            .expect("another plugin keeps answering");
    }
    assert!(book.resume("rulers-guides"));
    runtime
        .call_tool("rulers-guides", "list", &json!({}))
        .await
        .expect("resumed");
}
