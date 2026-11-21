use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{DISCARD_DEADLINE, PROBE_DEADLINE, ProbeDriver, ProbeError, probe};
use crate::plugins::consent::args::ProbeRequest;

const READ: &str = "read()";
const REJECT: &str = "reject()";

/// What the fake did, in order.
type Journal = Arc<Mutex<Vec<String>>>;

/// A probe session in memory: a page with a banner, a third-party script before consent and a
/// tracker cookie; after the click one more request and the consent cookie.
struct Fake {
    journal: Journal,
    fresh: bool,
    clicked: bool,
    fail_open: bool,
    fail_load: bool,
    hang_open: bool,
    hang_load: bool,
    hang_discard: bool,
    requests: Vec<Value>,
}

impl Fake {
    fn new(journal: &Journal) -> Self {
        Self {
            journal: Arc::clone(journal),
            fresh: true,
            clicked: false,
            fail_open: false,
            fail_load: false,
            hang_open: false,
            hang_load: false,
            hang_discard: false,
            requests: Vec::new(),
        }
    }

    fn note(&self, step: &str) {
        self.journal.lock().expect("journal").push(step.to_owned());
    }
}

fn sent(url: &str, kind: &str) -> Value {
    json!({ "request": { "url": url }, "type": kind })
}

#[async_trait]
impl ProbeDriver for Fake {
    async fn open(&mut self) -> Result<bool, ProbeError> {
        self.note("open");
        if self.hang_open {
            std::future::pending::<()>().await;
        }
        if self.fail_open {
            return Err(ProbeError("no window"));
        }
        Ok(self.fresh)
    }

    async fn load(&mut self, url: &str) -> Result<(), ProbeError> {
        self.note(&format!("load {url}"));
        if self.hang_load {
            std::future::pending::<()>().await;
        }
        if self.fail_load {
            return Err(ProbeError("the page did not load"));
        }
        self.requests.push(sent(url, "Document"));
        self.requests
            .push(sent("https://tracker.example.net/t.js", "Script"));
        Ok(())
    }

    async fn settle(&mut self) {
        self.note("settle");
    }

    async fn evaluate(&mut self, expression: &str) -> Result<Value, ProbeError> {
        self.note(&format!("evaluate {expression}"));
        if expression == REJECT {
            self.clicked = true;
            self.requests
                .push(sent("https://consent.example.net/log", "Ping"));
            return Ok(json!({ "clicked": true, "storage": ["early"] }));
        }
        let storage = if self.clicked {
            json!(["theme", "consent"])
        } else {
            json!(["theme"])
        };
        Ok(json!({
            "storage": storage,
            "banner": { "cmp": "onetrust", "visible": !self.clicked, "rejectable": true },
            "clicked": false,
        }))
    }

    fn take_requests(&mut self) -> Vec<Value> {
        self.note("requests");
        std::mem::take(&mut self.requests)
    }

    async fn cookies(&mut self) -> Result<Value, ProbeError> {
        self.note("cookies");
        let mut cookies = vec![json!({ "name": "_ga", "value": "secret-ga",
            "domain": ".example.test", "expires": 1821536000, "session": false })];
        if self.clicked {
            cookies.push(json!({ "name": "OptanonConsent", "value": "secret-consent",
                "domain": ".example.test", "expires": -1, "session": true }));
        }
        Ok(json!({ "cookies": cookies }))
    }

    async fn discard(&mut self) -> bool {
        self.note("discard");
        if self.hang_discard {
            std::future::pending::<()>().await;
        }
        true
    }

    fn abandon(&mut self) {
        self.note("abandon");
    }
}

fn request(reject: bool) -> ProbeRequest {
    ProbeRequest {
        url: "https://example.test/".to_owned(),
        read: READ.to_owned(),
        reject: reject.then(|| REJECT.to_owned()),
    }
}

fn steps(journal: &Journal) -> Vec<String> {
    journal.lock().expect("journal").clone()
}

#[tokio::test]
async fn a_probe_captures_before_and_after_reject_then_discards() {
    let journal = Journal::default();
    let mut fake = Fake::new(&journal);
    let capture = probe(&mut fake, &request(true), 1_790_000_000)
        .await
        .expect("captured");
    let value = capture.to_value();
    assert_eq!(value["url"], "https://example.test/");
    assert_eq!(value["capturedAt"], 1_790_000_000);
    assert_eq!(
        value["session"],
        json!({ "fresh": true, "discarded": true })
    );
    assert_eq!(
        value["banner"],
        json!({ "cmp": "onetrust", "visible": true, "rejected": true })
    );
    assert_eq!(
        value["before"]["requests"],
        json!([
            { "url": "https://example.test/", "resourceType": "Document" },
            { "url": "https://tracker.example.net/t.js", "resourceType": "Script" },
        ])
    );
    assert_eq!(value["before"]["storage"], json!(["theme"]));
    assert_eq!(value["before"]["cookies"].as_array().map(Vec::len), Some(1));
    // Only what came after the click.
    assert_eq!(
        value["afterReject"]["requests"],
        json!([{ "url": "https://consent.example.net/log", "resourceType": "Ping" }])
    );
    assert_eq!(value["afterReject"]["storage"], json!(["theme", "consent"]));
    assert_eq!(
        value["afterReject"]["cookies"].as_array().map(Vec::len),
        Some(2)
    );
    let text = value.to_string();
    assert!(!text.contains("secret-ga") && !text.contains("secret-consent"));
    assert_eq!(
        steps(&journal),
        [
            "open",
            "load https://example.test/",
            "evaluate read()",
            "cookies",
            "requests",
            "evaluate reject()",
            "settle",
            "evaluate read()",
            "requests",
            "cookies",
            "discard",
        ]
    );
}

#[tokio::test]
async fn without_the_reject_step_nothing_is_clicked() {
    let journal = Journal::default();
    let mut fake = Fake::new(&journal);
    let capture = probe(&mut fake, &request(false), 1)
        .await
        .expect("captured");
    assert!(capture.after_reject.is_none());
    assert_eq!(capture.banner["rejected"], false);
    assert!(
        !steps(&journal)
            .iter()
            .any(|step| step == "evaluate reject()")
    );
    assert_eq!(steps(&journal).last().map(String::as_str), Some("discard"));
}

#[tokio::test]
async fn a_session_that_was_not_empty_is_reported() {
    let journal = Journal::default();
    let mut fake = Fake::new(&journal);
    fake.fresh = false;
    let capture = probe(&mut fake, &request(false), 1)
        .await
        .expect("captured");
    assert!(!capture.fresh);
}

#[tokio::test]
async fn a_failed_probe_still_discards_its_session() {
    let journal = Journal::default();
    let mut fake = Fake::new(&journal);
    fake.fail_load = true;
    assert_eq!(
        probe(&mut fake, &request(true), 1).await.err(),
        Some(ProbeError("the page did not load"))
    );
    assert_eq!(steps(&journal).last().map(String::as_str), Some("discard"));

    let journal = Journal::default();
    let mut fake = Fake::new(&journal);
    fake.fail_open = true;
    assert!(probe(&mut fake, &request(true), 1).await.is_err());
    assert_eq!(steps(&journal), ["open", "discard"]);
}

#[tokio::test(start_paused = true)]
async fn a_page_that_never_loads_is_given_up_and_discarded() {
    let journal = Journal::default();
    let mut fake = Fake::new(&journal);
    fake.hang_load = true;
    let started = tokio::time::Instant::now();
    assert!(probe(&mut fake, &request(true), 1).await.is_err());
    assert!(started.elapsed() >= PROBE_DEADLINE - Duration::from_millis(1));
    assert_eq!(steps(&journal).last().map(String::as_str), Some("discard"));
}

#[tokio::test(start_paused = true)]
async fn a_probe_whose_open_hangs_is_given_up_and_discarded() {
    let journal = Journal::default();
    let mut fake = Fake::new(&journal);
    fake.hang_open = true;
    let started = tokio::time::Instant::now();
    assert!(probe(&mut fake, &request(true), 1).await.is_err());
    assert!(started.elapsed() >= PROBE_DEADLINE - Duration::from_millis(1));
    assert_eq!(steps(&journal), ["open", "discard"]);
}

#[tokio::test(start_paused = true)]
async fn a_dropped_probe_abandons_its_session() {
    // A caller that gives up (an MCP cancel) drops the probe mid-way: the session still goes.
    let journal = Journal::default();
    let mut fake = Fake::new(&journal);
    fake.hang_load = true;
    let cancelled =
        tokio::time::timeout(Duration::from_secs(1), probe(&mut fake, &request(true), 1)).await;
    assert!(cancelled.is_err());
    assert_eq!(
        steps(&journal),
        ["open", "load https://example.test/", "abandon"]
    );
}

#[tokio::test(start_paused = true)]
async fn a_discard_that_hangs_is_bounded_and_abandons() {
    let journal = Journal::default();
    let mut fake = Fake::new(&journal);
    fake.hang_discard = true;
    let started = tokio::time::Instant::now();
    let capture = probe(&mut fake, &request(false), 1)
        .await
        .expect("captured");
    assert!(!capture.discarded);
    assert!(started.elapsed() >= DISCARD_DEADLINE - Duration::from_millis(1));
    assert_eq!(steps(&journal).last().map(String::as_str), Some("abandon"));
}

#[tokio::test]
async fn a_finished_probe_does_not_abandon() {
    let journal = Journal::default();
    let mut fake = Fake::new(&journal);
    probe(&mut fake, &request(true), 1).await.expect("captured");
    assert!(!steps(&journal).iter().any(|step| step == "abandon"));
}
