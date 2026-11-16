//! The tests' [`ReplayPort`]: answers the prologue with a resolution set per view, records and
//! timestamps every command, and can take a fixed time per command like a real round trip.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rb_engine::{EngineError, ViewId};
use serde_json::{Value, json};

use super::port::{ReplayPort, ViewTraits};
use crate::cdp::lock;

/// One command the fake got.
#[derive(Clone, Debug)]
pub(crate) struct Sent {
    pub view: ViewId,
    /// `Runtime.evaluate` for the prologue, else the `Input.*` method.
    pub method: String,
    /// The prologue's expression, or the parameters.
    pub params: Value,
    pub at: Instant,
}

pub(crate) struct FakePort {
    pub round_trip: Duration,
    pub touch: Mutex<Vec<ViewId>>,
    pub context_menu: bool,
    pub resolved: Mutex<HashMap<ViewId, Value>>,
    /// What `apply` and `focus` answer per view (form sync, RB-020-05); `done` without.
    pub applied: Mutex<HashMap<ViewId, Value>>,
    pub viewport: Mutex<Value>,
    pub sent: Mutex<Vec<Sent>>,
}

impl Default for FakePort {
    fn default() -> Self {
        Self {
            round_trip: Duration::ZERO,
            touch: Mutex::new(Vec::new()),
            context_menu: false,
            resolved: Mutex::new(HashMap::new()),
            applied: Mutex::new(HashMap::new()),
            viewport: Mutex::new(json!([0.0, 0.0, 1.0])),
            sent: Mutex::new(Vec::new()),
        }
    }
}

impl FakePort {
    pub(crate) fn with_round_trip(round_trip: Duration) -> Self {
        Self {
            round_trip,
            ..Self::default()
        }
    }

    /// `view`'s agent answers `resolved` to every `resolve`.
    pub(crate) fn resolve_to(&self, view: ViewId, resolved: Value) {
        lock(&self.resolved).insert(view, resolved);
    }

    pub(crate) fn sent(&self) -> Vec<Sent> {
        lock(&self.sent).clone()
    }

    pub(crate) fn sent_to(&self, view: ViewId) -> Vec<Sent> {
        self.sent()
            .into_iter()
            .filter(|sent| sent.view == view)
            .collect()
    }

    /// `method:type` of every command to `view`.
    pub(crate) fn kinds(&self, view: ViewId) -> Vec<String> {
        self.sent_to(view)
            .iter()
            .map(|sent| match sent.params["type"].as_str() {
                Some(kind) => format!("{}:{kind}", sent.method),
                None => sent.method.clone(),
            })
            .collect()
    }

    async fn round_trip(&self) {
        if !self.round_trip.is_zero() {
            tokio::time::sleep(self.round_trip).await;
        }
    }

    fn push(&self, view: ViewId, method: &str, params: Value) {
        lock(&self.sent).push(Sent {
            view,
            method: method.to_owned(),
            params,
            at: Instant::now(),
        });
    }
}

#[async_trait]
impl ReplayPort for FakePort {
    fn traits(&self, view: ViewId) -> Option<ViewTraits> {
        (view.0 < 100).then(|| ViewTraits {
            touch: lock(&self.touch).contains(&view),
        })
    }

    fn context_menu(&self) -> bool {
        self.context_menu
    }

    async fn world(&self, view: ViewId, expression: String) -> Result<Value, EngineError> {
        self.push(view, "Runtime.evaluate", json!(expression));
        self.round_trip().await;
        if expression.contains(r#""cmd":"apply""#) || expression.contains(r#""cmd":"focus""#) {
            let applied = lock(&self.applied)
                .get(&view)
                .cloned()
                .unwrap_or_else(|| json!({ "status": "done" }));
            return Ok(json!({ "applied": applied }));
        }
        let resolved = if expression.contains(r#""cmd":"resolve""#) {
            lock(&self.resolved).get(&view).cloned().unwrap_or_else(
                || json!({ "status": "ok", "x": 100.0, "y": 50.0, "scrolled": false }),
            )
        } else {
            Value::Null
        };
        Ok(json!({ "resolved": resolved, "viewport": lock(&self.viewport).clone() }))
    }

    async fn send(
        &self,
        view: ViewId,
        method: &'static str,
        params: Value,
    ) -> Result<Value, EngineError> {
        self.push(view, method, params);
        self.round_trip().await;
        if view.0 == 99 {
            return Err(EngineError::Cdp {
                method: method.to_owned(),
                code: -32000,
                message: "Target closed".to_owned(),
            });
        }
        Ok(json!({}))
    }
}
