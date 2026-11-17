use std::time::Duration;

use rb_engine::EngineKind;
use serde_json::json;

use super::*;
use crate::cdp::fake::FakeBrowser;

struct OneView;

impl Window for OneView {
    fn sessions(&self) -> Vec<(ViewId, String)> {
        vec![(ViewId(1), "S1".to_owned())]
    }

    fn urls(&self) -> Vec<(ViewId, String)> {
        vec![(
            ViewId(1),
            "http://127.0.0.1:8080/network/index.html".to_owned(),
        )]
    }
}

struct Running {
    fake: Arc<FakeBrowser>,
    rules: Arc<Rules>,
    _views: broadcast::Sender<ViewEvent>,
    stop: watch::Sender<bool>,
}

fn start() -> Running {
    start_with(|_, _| {}, PAUSED_CAPACITY)
}

/// As [`start`], `before` run on the fake and the rules before the loop starts, its paused
/// requests on a channel of `capacity`.
fn start_with(before: impl FnOnce(&FakeBrowser, &Rules), capacity: usize) -> Running {
    let (fake, router) = FakeBrowser::connect();
    let rules = Arc::new(Rules::new(
        Box::new(|| vec![(ViewId(1), EngineKind::Chromium)]),
        "s".to_owned(),
    ));
    rules.set_trusted(true);
    let paused = router.tap(PAUSED_METHOD, capacity);
    before(&fake, &rules);
    let (views, receiver) = broadcast::channel(16);
    let (stop, stopped) = watch::channel(false);
    tokio::spawn(run_tapped(
        Arc::clone(&rules),
        router,
        Arc::new(OneView),
        paused,
        receiver,
        stopped,
    ));
    Running {
        fake,
        rules,
        _views: views,
        stop,
    }
}

fn mock_user(delay_ms: u64) -> Value {
    json!({ "rules": [{ "id": "user", "origin": "project",
        "match": { "urlPattern": "*/api/user" },
        "action": { "type": "mock", "status": 500, "delayMs": delay_ms } }] })
}

async fn until(what: impl Fn() -> bool) {
    for _ in 0..2000 {
        if what() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("not reached in time");
}

fn paused(fake: &FakeBrowser, id: &str, url: &str) {
    fake.emit(json!({
        "method": "Fetch.requestPaused",
        "sessionId": "S1",
        "params": { "requestId": id, "resourceType": "XHR",
                    "request": { "url": url, "method": "GET", "headers": {} } },
    }));
}

#[tokio::test(flavor = "multi_thread")]
async fn rules_enable_fetch_and_a_mock_fulfils_a_paused_request() {
    let running = start();
    running
        .rules
        .apply(
            "network-panel",
            &json!({ "rules": [{ "id": "user", "origin": "project",
                "match": { "urlPattern": "*/api/user" },
                "action": { "type": "mock", "status": 500, "delayMs": 30 } }] }),
        )
        .expect("reads");
    let fake = &running.fake;
    until(|| !fake.sent("Fetch.enable").is_empty()).await;
    assert_eq!(
        fake.sent("Fetch.enable")[0]["params"]["patterns"],
        json!([{ "urlPattern": "*/api/user", "requestStage": "Request" }])
    );
    paused(fake, "R1", "http://127.0.0.1:8080/api/user");
    paused(fake, "R2", "http://127.0.0.1:8080/other");
    until(|| fake.sent("Fetch.fulfillRequest").len() == 1).await;
    until(|| fake.sent("Fetch.continueRequest").len() == 1).await;
    assert_eq!(
        fake.sent("Fetch.fulfillRequest")[0]["params"]["requestId"],
        "R1"
    );
    assert_eq!(fake.sent("Fetch.fulfillRequest")[0]["sessionId"], "S1");
    assert_eq!(
        fake.sent("Fetch.continueRequest")[0]["params"]["requestId"],
        "R2"
    );
    assert_eq!(running.rules.state("network-panel")["hits"]["user"], 1);
    assert_eq!(
        running.rules.state("network-panel")["views"][0]["intercepting"],
        true
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn no_rules_left_disables_fetch() {
    let running = start();
    let block = json!({ "rules": [{ "id": "b", "origin": "settings",
        "match": { "urlPattern": "*/tracking.js" }, "action": { "type": "block" } }] });
    running.rules.apply("p", &block).expect("reads");
    let fake = &running.fake;
    until(|| !fake.sent("Fetch.enable").is_empty()).await;
    running
        .rules
        .apply("p", &json!({ "rules": [] }))
        .expect("reads");
    until(|| !fake.sent("Fetch.disable").is_empty()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_cache_switch_and_a_clearing_reach_the_session() {
    let running = start();
    running
        .rules
        .apply("p", &json!({ "cacheDisabled": true, "clearSiteData": 1 }))
        .expect("reads");
    let fake = &running.fake;
    until(|| !fake.sent("Network.setCacheDisabled").is_empty()).await;
    assert_eq!(
        fake.sent("Network.setCacheDisabled")[0]["params"],
        json!({ "cacheDisabled": true })
    );
    assert!(fake.sent("Storage.clearDataForOrigin").is_empty());
    running
        .rules
        .apply("p", &json!({ "cacheDisabled": false, "clearSiteData": 2 }))
        .expect("reads");
    until(|| fake.sent("Network.setCacheDisabled").len() == 2).await;
    until(|| !fake.sent("Storage.clearDataForOrigin").is_empty()).await;
    assert_eq!(
        fake.sent("Storage.clearDataForOrigin")[0]["params"],
        json!({ "origin": "http://127.0.0.1:8080", "storageTypes": "all" })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_fulfil_continues_the_request_and_a_failed_continue_fails_it() {
    let running = start();
    running.rules.apply("p", &mock_user(0)).expect("reads");
    let fake = &running.fake;
    until(|| !fake.sent("Fetch.enable").is_empty()).await;
    fake.fail("Fetch.fulfillRequest", true);
    paused(fake, "R1", "http://127.0.0.1:8080/api/user");
    until(|| fake.sent("Fetch.continueRequest").len() == 1).await;
    assert_eq!(
        fake.sent("Fetch.continueRequest")[0]["params"],
        json!({ "requestId": "R1" }),
        "never left paused (review M1, Charge 7)"
    );
    fake.fail("Fetch.continueRequest", true);
    paused(fake, "R2", "http://127.0.0.1:8080/api/user");
    until(|| fake.sent("Fetch.failRequest").len() == 1).await;
    assert_eq!(
        fake.sent("Fetch.failRequest")[0]["params"],
        json!({ "requestId": "R2", "errorReason": "Failed" })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn paused_requests_lost_on_a_full_channel_are_released_by_restarting_fetch() {
    let running = start_with(
        |fake, rules| {
            rules.apply("p", &mock_user(0)).expect("reads");
            // One fits; the other two are lost before the loop reads any.
            for id in ["R1", "R2", "R3"] {
                paused(fake, id, "http://127.0.0.1:8080/api/user");
            }
        },
        1,
    );
    let fake = &running.fake;
    until(|| fake.sent("Fetch.enable").len() == 2).await;
    let id = |message: &Value| message["id"].as_u64().unwrap_or_default();
    let (enables, disables) = (fake.sent("Fetch.enable"), fake.sent("Fetch.disable"));
    assert_eq!(disables.len(), 1);
    assert!(
        id(&enables[0]) < id(&disables[0]) && id(&disables[0]) < id(&enables[1]),
        "disable releases what was lost, enable intercepts again"
    );
    until(|| fake.sent("Fetch.fulfillRequest").len() == 1).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restart_whose_enable_fails_no_longer_counts_as_intercepting() {
    // Charge 8 review L2: the restart ignored `Fetch.enable`'s failure, so the view stayed
    // `intercepting` while no rule reached it.
    let running = start_with(
        |fake, rules| {
            rules.apply("p", &mock_user(0)).expect("reads");
            fake.hold("Fetch.disable", None);
            for id in ["R1", "R2", "R3"] {
                paused(fake, id, "http://127.0.0.1:8080/api/user");
            }
        },
        1,
    );
    let fake = &running.fake;
    until(|| fake.sent("Fetch.disable").len() == 1).await;
    assert_eq!(running.rules.state("p")["views"][0]["intercepting"], true);
    fake.fail("Fetch.enable", true);
    fake.release();
    until(|| fake.sent("Fetch.enable").len() == 2).await;
    until(|| running.rules.state("p")["views"][0]["intercepting"] == false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_disables_fetch_so_nothing_stays_paused() {
    let running = start();
    running.rules.apply("p", &mock_user(0)).expect("reads");
    let fake = &running.fake;
    until(|| !fake.sent("Fetch.enable").is_empty()).await;
    assert!(fake.sent("Fetch.disable").is_empty());
    running.stop.send(true).expect("running");
    until(|| fake.sent("Fetch.disable").len() == 1).await;
    assert_eq!(fake.sent("Fetch.disable")[0]["sessionId"], "S1");
}
