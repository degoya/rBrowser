//! RB-080-66: a page request that waits longer than its page does is given up by the host as
//! well, so it leaves the plugin's queue instead of keeping the next ones waiting.

use std::time::Duration;

use rb_plugin_host::{MAX_PENDING, NO_ANSWER, PAGE_TIMEOUT_MILLISECONDS};
use serde_json::json;

use super::*;
use crate::cdp::fake::FakeBrowser;

/// A component whose page requests never end; counts how often the host gave one up.
#[derive(Default)]
struct Hanging {
    given_up: std::sync::atomic::AtomicUsize,
}

#[async_trait]
impl Component for Hanging {
    async fn page_message(&self, _view: u32, _message: &Value) -> Result<String, Failure> {
        std::future::pending().await
    }

    async fn run_command(&self, _command: &str, _args: &str) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn call_tool(&self, _tool: &str, _args: &str) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    fn gave_up(&self) {
        self.given_up.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test(start_paused = true)]
async fn a_request_the_page_gave_up_leaves_the_queue() {
    let hanging = Arc::new(Hanging::default());
    let (_fake, router) = FakeBrowser::connect();
    let panels: PanelSink = Arc::new(|_, _| {});
    let plugin = Plugin {
        id: "hello".to_owned(),
        source: PluginSource::Bundled,
        component: Arc::clone(&hanging) as Arc<dyn Component>,
        bridge: None,
        console: false,
        network: false,
        status: false,
    };
    let runtime = PluginRuntime::new(
        vec![plugin],
        router,
        Handle::current(),
        Sinks {
            panels,
            status: Arc::new(|_, _| {}),
            host: Arc::new(|_, _| {}),
        },
    );
    let started = tokio::time::Instant::now();
    let answer = runtime
        .request("hello", ViewId(1), json!({ "op": "layout" }))
        .await;
    assert_eq!(answer, Err(NO_ANSWER.to_owned()));
    assert!(started.elapsed() >= Duration::from_millis(u64::from(PAGE_TIMEOUT_MILLISECONDS)));
    // Its place in the queue is free again: every slot can be taken.
    let slots: Vec<_> = (0..MAX_PENDING)
        .map(|_| runtime.take_slot("hello").expect("a slot"))
        .collect();
    assert_eq!(slots.len(), MAX_PENDING);
    // Review of RB-080-66, 5: a request given up counts as the plugin's failure, so a hung
    // plugin is suspended in the end.
    assert_eq!(hanging.given_up.load(Ordering::SeqCst), 1);
}

/// A component whose calls would wait for a running one (`busy`), or not.
struct Busy(bool);

#[async_trait]
impl Component for Busy {
    async fn page_message(&self, _view: u32, _message: &Value) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn run_command(&self, _command: &str, _args: &str) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn call_tool(&self, _tool: &str, _args: &str) -> Result<String, Failure> {
        Ok("{\"ok\":true}".to_owned())
    }

    fn busy(&self) -> bool {
        self.0
    }
}

/// RB-080-90: a panel's call that has to wait for a running one (a check run of the same
/// plugin) tells the plugin's panels first, so they show that they wait instead of failing.
#[tokio::test]
async fn a_panel_call_that_waits_says_so_to_the_panels() {
    for busy in [true, false] {
        let (_fake, router) = FakeBrowser::connect();
        let events: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
        let sink = Arc::clone(&events);
        let panels: PanelSink = Arc::new(move |plugin, event| {
            lock(&sink).push((plugin.to_owned(), event));
        });
        let plugin = Plugin {
            id: "fonts".to_owned(),
            source: PluginSource::Bundled,
            component: Arc::new(Busy(busy)),
            bridge: None,
            console: false,
            network: false,
            status: false,
        };
        let runtime = PluginRuntime::new(
            vec![plugin],
            router,
            Handle::current(),
            Sinks {
                panels,
                status: Arc::new(|_, _| {}),
                host: Arc::new(|_, _| {}),
            },
        );
        let answer = runtime
            .panel_call_tool("fonts", "list", &json!({}), 64 * 1024)
            .await
            .expect("answered");
        assert_eq!(answer, json!({ "ok": true }));
        let posted = lock(&events).clone();
        if busy {
            assert_eq!(posted, [("fonts".to_owned(), json!({ "host": "waiting" }))]);
        } else {
            assert!(posted.is_empty(), "{posted:?}");
        }
    }
}
