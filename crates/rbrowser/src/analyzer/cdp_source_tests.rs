use std::sync::{Arc, Mutex, Weak};

use rb_engine::EngineError;
use serde_json::{Value, json};

use super::{SETTLE, collect, wait_for_sheets};
use crate::cdp::{CdpEvent, CdpRouter, CdpTransport, lock};

/// A browser that answers the analyser's commands and loads a page with three sheets: one of
/// another origin, one inline, one of the browser's own.
#[derive(Default)]
struct Scripted {
    router: Mutex<Weak<CdpRouter>>,
    sent: Mutex<Vec<Value>>,
    refuse_hidden: bool,
}

impl Scripted {
    fn reply(&self, message: Value) {
        if let Some(router) = lock(&self.router).upgrade() {
            router.on_message(message);
        }
    }

    fn event(&self, method: &str, params: Value) {
        self.reply(json!({ "method": method, "params": params, "sessionId": "S1" }));
    }

    fn answer(&self, message: &Value) -> Value {
        match message["method"].as_str().unwrap_or_default() {
            "Target.createTarget" if self.refuse_hidden && message["params"]["hidden"] == true => {
                json!({ "error": { "code": -32602, "message": "hidden is not supported" } })
            }
            "Target.createTarget" => json!({ "result": { "targetId": "T9" } }),
            "Target.attachToTarget" => json!({ "result": { "sessionId": "S1" } }),
            "CSS.getStyleSheetText" => {
                let text = match message["params"]["styleSheetId"].as_str() {
                    Some("a") => "@media (min-width: 900px) {}",
                    Some("b") => "@media (min-width: 600px) {}",
                    _ => "ua {}",
                };
                json!({ "result": { "text": text } })
            }
            "Runtime.evaluate" => {
                json!({ "result": { "result": { "type": "number", "value": 18 } } })
            }
            _ => json!({ "result": {} }),
        }
    }
}

impl CdpTransport for Scripted {
    fn send(&self, frame: Vec<u8>) -> Result<(), EngineError> {
        let body = frame.strip_suffix(&[0]).unwrap_or(&frame);
        let message: Value = serde_json::from_slice(body).expect("json");
        lock(&self.sent).push(message.clone());
        let mut reply = self.answer(&message);
        reply["id"] = message["id"].clone();
        self.reply(reply);
        if message["method"] == "Page.navigate" {
            let sheet = |id: &str, url: &str, inline: bool, origin: &str| json!({ "header": { "styleSheetId": id, "sourceURL": url, "isInline": inline, "origin": origin } });
            self.event(
                "CSS.styleSheetAdded",
                sheet("a", "http://cdn.other.test/a.css", false, "regular"),
            );
            self.event(
                "CSS.styleSheetAdded",
                sheet("b", "http://site.test/", true, "regular"),
            );
            self.event("CSS.styleSheetAdded", sheet("c", "", false, "user-agent"));
            self.reply(json!({ "method": "CSS.styleSheetAdded", "sessionId": "OTHER", "params": { "header": { "styleSheetId": "z", "origin": "regular" } } }));
            self.event("Page.loadEventFired", json!({ "timestamp": 1 }));
        }
        Ok(())
    }
}

fn browser(refuse_hidden: bool) -> (Arc<Scripted>, Arc<CdpRouter>) {
    let scripted = Arc::new(Scripted {
        refuse_hidden,
        ..Scripted::default()
    });
    let router = CdpRouter::new(scripted.clone());
    *lock(&scripted.router) = Arc::downgrade(&router);
    (scripted, router)
}

fn methods(scripted: &Scripted) -> Vec<String> {
    lock(&scripted.sent)
        .iter()
        .map(|message| message["method"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[tokio::test]
async fn every_site_sheet_is_read_and_the_target_closed() {
    let (scripted, router) = browser(false);
    let collected = collect(&router, "http://site.test/")
        .await
        .expect("collected");
    let sheets: Vec<(&str, &str)> = collected
        .sheets
        .iter()
        .map(|sheet| (sheet.url.as_str(), sheet.text.as_str()))
        .collect();
    assert_eq!(
        sheets,
        vec![
            (
                "http://cdn.other.test/a.css",
                "@media (min-width: 900px) {}"
            ),
            ("http://site.test/#inline-1", "@media (min-width: 600px) {}"),
        ]
    );
    assert_eq!(collected.root_font_size_px, Some(18.0));
    let sent = methods(&scripted);
    assert_eq!(
        sent.first().map(String::as_str),
        Some("Target.createTarget")
    );
    assert_eq!(sent.last().map(String::as_str), Some("Target.closeTarget"));
    assert!(sent.iter().any(|method| method == "CSS.enable"));
    let created = lock(&scripted.sent)[0]["params"].clone();
    assert_eq!(created["hidden"], true);
}

#[tokio::test]
async fn without_hidden_targets_a_background_target_is_used() {
    let (scripted, router) = browser(true);
    let collected = collect(&router, "http://site.test/")
        .await
        .expect("collected");
    assert_eq!(collected.sheets.len(), 2);
    let creates = lock(&scripted.sent)
        .iter()
        .filter(|message| message["method"] == "Target.createTarget")
        .count();
    assert_eq!(creates, 2);
}

/// The runtime's clock is paused and moves only through the sleeps: the settle takes exactly
/// [`SETTLE`] when nothing moves the timer, and the noise alone would have held it for
/// 20 * SETTLE / 6.
#[tokio::test(start_paused = true)]
async fn events_of_other_sessions_do_not_hold_the_settle_timer() {
    let (sender, mut events) = tokio::sync::broadcast::channel(64);
    let event = |method: &str, session: &str| CdpEvent {
        method: method.to_owned(),
        params: json!({}),
        session_id: Some(session.to_owned()),
    };
    sender
        .send(event("Page.loadEventFired", "S1"))
        .expect("sent");
    let noise = tokio::spawn(async move {
        // Another view keeps talking for well over the settle time.
        for _ in 0..20 {
            tokio::time::sleep(SETTLE / 6).await;
            let _ = sender.send(event("Network.requestWillBeSent", "OTHER"));
        }
    });
    let started = tokio::time::Instant::now();
    let headers = wait_for_sheets(&mut events, "S1").await;
    let took = started.elapsed();
    noise.abort();
    assert!(headers.is_empty());
    assert_eq!(took, SETTLE, "settled after {took:?}");
}
