use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use rb_engine::{EngineError, EngineKind, ViewId, ViewInfo};
use rb_plugin_types::codes;
use serde_json::{Value, json};

use super::{Views, cdp, evaluate, plugin_view, world_of};

/// Two views - 1 Chromium, 2 Firefox - answering CDP as told.
struct Fake {
    calls: Mutex<Vec<(u32, String, Value)>>,
    evaluate: Value,
    hang: bool,
}

impl Fake {
    fn new(evaluate: Value) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            evaluate,
            hang: false,
        }
    }

    fn calls(&self) -> Vec<(u32, String, Value)> {
        self.calls.lock().expect("calls").clone()
    }
}

fn info(id: u32, engine: EngineKind) -> ViewInfo {
    ViewInfo {
        id: ViewId(id),
        label: format!("View {id}"),
        breakpoint: Some(format!("bp{id}")),
        cell_breakpoint: None,
        engine,
        width: 375,
        height: 812,
        dpr: 3.0,
        scale: 1.0,
        url: "https://site.test/".to_owned(),
        title: "Site".to_owned(),
        loading: false,
        session: "default".to_owned(),
        cell: None,
    }
}

#[async_trait]
impl Views for Fake {
    fn views(&self) -> Vec<ViewInfo> {
        vec![info(1, EngineKind::Chromium), info(2, EngineKind::Firefox)]
    }

    fn active_view(&self) -> Option<ViewId> {
        Some(ViewId(1))
    }

    async fn cdp(&self, view: ViewId, method: &str, params: Value) -> Result<Value, EngineError> {
        self.calls
            .lock()
            .expect("calls")
            .push((view.0, method.to_owned(), params));
        match method {
            "Page.getFrameTree" => Ok(json!({ "frameTree": { "frame": { "id": "MAIN" } } })),
            "Page.createIsolatedWorld" => Ok(json!({ "executionContextId": 77 })),
            "Runtime.evaluate" if self.hang => {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(json!({}))
            }
            "Runtime.evaluate" => Ok(self.evaluate.clone()),
            _ => Ok(json!({ "echo": method })),
        }
    }
}

#[tokio::test]
async fn evaluate_runs_in_the_plugins_own_world_by_value_and_awaited() {
    let fake = Fake::new(json!({ "result": { "type": "object", "value": { "n": 2 } } }));
    let answer = evaluate(
        &fake,
        ViewId(1),
        &world_of("seo"),
        "({n: 2})",
        Duration::from_secs(5),
    )
    .await
    .expect("evaluated");
    assert_eq!(answer, r#"{"n":2}"#);
    let calls = fake.calls();
    assert_eq!(calls[1].1, "Page.createIsolatedWorld");
    assert_eq!(calls[1].2["worldName"], "rbplugin-seo");
    assert_eq!(calls[1].2["frameId"], "MAIN");
    assert_eq!(calls[1].2["grantUniveralAccess"], false);
    let run = &calls[2].2;
    assert_eq!(run["contextId"], 77);
    assert_eq!(run["returnByValue"], true);
    assert_eq!(run["awaitPromise"], true);
    assert_eq!(run["userGesture"], false);
    assert_eq!(run["timeout"], 5000);
}

#[tokio::test]
async fn undefined_is_null_and_a_throw_is_the_exception_reason() {
    let fake = Fake::new(json!({ "result": { "type": "undefined" } }));
    let answer = evaluate(&fake, ViewId(1), "w", "void 0", Duration::from_secs(1))
        .await
        .expect("evaluated");
    assert_eq!(answer, "null");

    let fake = Fake::new(json!({
        "result": {},
        "exceptionDetails": { "text": "Uncaught", "exception": { "description": "Error: boom" } },
    }));
    let failure = evaluate(&fake, ViewId(1), "w", "throw 1", Duration::from_secs(1))
        .await
        .expect_err("threw");
    assert_eq!(failure.code(), Some(codes::HOST_FAILED));
    assert_eq!(failure.param("reason"), Some("exception"));
    assert_eq!(failure.param("message"), Some("Error: boom"));
}

#[tokio::test]
async fn a_script_past_its_time_ends_with_the_timeout_reason() {
    let mut fake = Fake::new(json!({}));
    fake.hang = true;
    let failure = evaluate(
        &fake,
        ViewId(1),
        "w",
        "for(;;){}",
        Duration::from_millis(50),
    )
    .await
    .expect_err("too long");
    assert_eq!(failure.param("reason"), Some("timeout"));
}

#[tokio::test]
async fn only_chromium_views_take_evaluate_and_cdp() {
    let fake = Fake::new(json!({}));
    let failure = evaluate(&fake, ViewId(2), "w", "1", Duration::from_secs(1))
        .await
        .expect_err("firefox");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
    assert_eq!(failure.param("engine"), Some("firefox"));
    // The one constructor, naming the view, so the check runner skips exactly it (RB-080-98).
    assert_eq!(
        failure,
        rb_plugin_types::Failure::engine_unserved("evaluate", 2, "firefox")
    );
    assert_eq!(failure.unserved_engine_in(2), Some("firefox"));
    let failure = cdp(
        &fake,
        ViewId(2),
        "DOM.getDocument",
        "{}",
        Duration::from_secs(1),
    )
    .await
    .expect_err("firefox");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
    let failure = cdp(
        &fake,
        ViewId(9),
        "DOM.getDocument",
        "{}",
        Duration::from_secs(1),
    )
    .await
    .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::NOT_FOUND));
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn cdp_sends_an_object_and_answers_the_result() {
    let fake = Fake::new(json!({}));
    let answer = cdp(
        &fake,
        ViewId(1),
        "DOM.getDocument",
        "",
        Duration::from_secs(1),
    )
    .await
    .expect("sent");
    assert_eq!(answer, r#"{"echo":"DOM.getDocument"}"#);
    assert_eq!(fake.calls()[0].2, json!({}));
    for params in ["[1]", "1", "not json"] {
        let failure = cdp(
            &fake,
            ViewId(1),
            "DOM.getDocument",
            params,
            Duration::from_secs(1),
        )
        .await
        .expect_err(params);
        assert_eq!(failure.code(), Some(codes::INVALID), "{params}");
    }
}

#[test]
fn a_view_is_listed_with_its_engine_breakpoint_and_size() {
    let view = plugin_view(&info(1, EngineKind::Webkit));
    assert_eq!(view.id, 1);
    assert_eq!(view.engine, "webkit");
    assert_eq!(view.breakpoint, "bp1");
    assert_eq!((view.width, view.height), (375, 812));
    assert!((view.dpr - 3.0).abs() < f32::EPSILON);
    assert_eq!(view.url, "https://site.test/");
}

/// RB-094-13 (owner, diag33): `Page.getResourceContent` failed with "Agent is not enabled" in
/// every view - the host turns the Page domain on in the view's session first.
#[tokio::test]
async fn resource_content_turns_the_page_domain_on_first() {
    let fake = Fake::new(json!({}));
    cdp(
        &fake,
        ViewId(1),
        "Page.getResourceContent",
        r#"{"frameId":"F","url":"https://example.com/"}"#,
        Duration::from_secs(1),
    )
    .await
    .expect("sent");
    let methods: Vec<String> = fake
        .calls()
        .into_iter()
        .map(|(_, method, _)| method)
        .collect();
    assert_eq!(methods, ["Page.enable", "Page.getResourceContent"]);
    cdp(
        &fake,
        ViewId(1),
        "DOM.getDocument",
        "",
        Duration::from_secs(1),
    )
    .await
    .expect("sent");
    assert_eq!(fake.calls().len(), 3, "no other method turns a domain on");
}
