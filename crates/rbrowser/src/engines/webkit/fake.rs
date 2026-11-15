//! A fake sidecar for the WebKit engine's tests: it plays the `webkit` module - views, history,
//! the `navigated` notification, the agent - records every request, and lets a test fail a
//! method, close a window or end the process.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_sidecar::SidecarError;
use rb_sidecar::rpc::Notification;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::port::{Notifications, SidecarPort};
use super::rpc;
use crate::cdp::lock;

#[derive(Default)]
struct State {
    running: bool,
    calls: Vec<(String, Value)>,
    /// Errors the next calls of a method answer with: (code, message).
    failures: HashMap<String, VecDeque<(i64, String)>>,
    /// What `webkit.view.agent` answers, in order; `{ status: "ok", x: 10, y: 20 }` when empty.
    agent_answers: VecDeque<Value>,
    views: Vec<u32>,
    subscribers: Vec<mpsc::UnboundedSender<Notification>>,
    launches: u32,
    /// The next `webkit.view.emulate` answers `recreated: true`.
    recreate_next: bool,
}

/// The fake sidecar.
#[derive(Default)]
pub struct FakeSidecar {
    state: Mutex<State>,
}

struct Receiver(mpsc::UnboundedReceiver<Notification>);

#[async_trait]
impl Notifications for Receiver {
    async fn recv(&mut self) -> Option<Notification> {
        self.0.recv().await
    }
}

impl FakeSidecar {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Every request so far: method and parameters.
    pub fn calls(&self) -> Vec<(String, Value)> {
        lock(&self.state).calls.clone()
    }

    /// The parameters of every call of `method`.
    pub fn calls_of(&self, method: &str) -> Vec<Value> {
        self.calls()
            .into_iter()
            .filter(|(name, _)| name == method)
            .map(|(_, params)| params)
            .collect()
    }

    /// The next call of `method` fails with `code` and `message`.
    pub fn fail_next(&self, method: &str, code: i64, message: &str) {
        lock(&self.state)
            .failures
            .entry(method.to_owned())
            .or_default()
            .push_back((code, message.to_owned()));
    }

    /// The next `webkit.view.agent` answers `answer`.
    pub fn answer_agent(&self, answer: Value) {
        lock(&self.state).agent_answers.push_back(answer);
    }

    /// How often `webkit.launch` ran.
    pub fn launches(&self) -> u32 {
        lock(&self.state).launches
    }

    /// The next `webkit.view.emulate` builds a new context (a new window).
    pub fn recreate_next(&self) {
        lock(&self.state).recreate_next = true;
    }

    /// The process id the fake gives view `view`'s browser (one per view, RB-080-40).
    pub fn pid_of(view: u32) -> u32 {
        4000 + view
    }

    /// Sends a notification as the sidecar would.
    pub fn notify(&self, method: &str, params: Value) {
        let subscribers = lock(&self.state).subscribers.clone();
        for subscriber in subscribers {
            let _ = subscriber.send(Notification {
                method: method.to_owned(),
                params: params.clone(),
            });
        }
    }

    /// The process ends: every subscription ends, the next request starts a new one.
    pub fn crash(&self) {
        let mut state = lock(&self.state);
        state.running = false;
        state.views.clear();
        state.subscribers.clear();
    }

    fn answer(&self, method: &str, params: &Value) -> Result<Value, SidecarError> {
        let mut state = lock(&self.state);
        state.running = true;
        state.calls.push((method.to_owned(), params.clone()));
        if let Some((code, message)) = state.failures.get_mut(method).and_then(VecDeque::pop_front)
        {
            return Err(SidecarError::Remote {
                method: method.to_owned(),
                code,
                message,
                data: None,
            });
        }
        let view = params
            .get("view")
            .and_then(Value::as_u64)
            .and_then(|view| u32::try_from(view).ok());
        let needs_view = method.starts_with("webkit.view.") && method != rpc::CREATE;
        if needs_view
            && method != rpc::CLOSE
            && view.is_some_and(|view| !state.views.contains(&view))
        {
            return Err(SidecarError::Remote {
                method: method.to_owned(),
                code: -32005,
                message: "no WebKit view".to_owned(),
                data: None,
            });
        }
        let answer = match method {
            rpc::LAUNCH => {
                state.launches += 1;
                json!({ "version": "27.2" })
            }
            rpc::CREATE => {
                state.views.extend(view);
                json!({ "pid": view.map(Self::pid_of), "version": "27.2" })
            }
            rpc::CLOSE => {
                state.views.retain(|known| Some(*known) != view);
                json!({})
            }
            rpc::EMULATE => {
                let recreated = std::mem::take(&mut state.recreate_next);
                json!({ "recreated": recreated })
            }
            rpc::SCREENSHOT => json!({ "pngBase64": "iVBORw==" }),
            rpc::COOKIES => {
                let added = params["cookies"].as_array().map_or(0, Vec::len);
                json!({ "added": added, "refused": 0 })
            }
            rpc::AGENT => state.agent_answers.pop_front().unwrap_or_else(
                || json!({ "status": "ok", "x": 10.0, "y": 20.0, "scrolled": false }),
            ),
            rpc::SHUTDOWN => {
                state.views.clear();
                json!({})
            }
            _ => json!({}),
        };
        Ok(answer)
    }
}

#[async_trait]
impl SidecarPort for Arc<FakeSidecar> {
    async fn request(&self, method: &str, params: Value) -> Result<Value, SidecarError> {
        let answer = self.answer(method, &params)?;
        // The module announces the committed URL of a navigation.
        if method == rpc::NAVIGATE || (method == rpc::CREATE && params["url"].is_string()) {
            self.notify(
                rpc::EVENT,
                json!({ "view": params["view"], "kind": "navigated", "url": params["url"] }),
            );
        }
        Ok(answer)
    }

    async fn request_if_running(
        &self,
        method: &str,
        params: Value,
    ) -> Option<Result<Value, SidecarError>> {
        if !lock(&self.state).running {
            return None;
        }
        Some(self.answer(method, &params))
    }

    async fn subscribe(&self) -> Result<Box<dyn Notifications>, SidecarError> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut state = lock(&self.state);
        state.running = true;
        state.subscribers.push(sender);
        Ok(Box::new(Receiver(receiver)))
    }
}
