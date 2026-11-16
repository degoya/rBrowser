//! A browser end for the capture tests: answers each method from a script (a result or a CDP
//! error), `{}` for the rest, and records every message in order.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use rb_engine::EngineError;
use serde_json::{Value, json};

use crate::cdp::{CdpRouter, CdpTransport, lock};

#[derive(Default)]
pub(crate) struct Scripted {
    router: Mutex<Weak<CdpRouter>>,
    answers: Mutex<HashMap<String, Result<Value, String>>>,
    sent: Mutex<Vec<Value>>,
}

impl Scripted {
    pub(crate) fn connect() -> (Arc<Self>, Arc<CdpRouter>) {
        let script = Arc::new(Self::default());
        let router = CdpRouter::new(script.clone());
        *lock(&script.router) = Arc::downgrade(&router);
        (script, router)
    }

    pub(crate) fn answer(&self, method: &str, result: Value) {
        lock(&self.answers).insert(method.to_owned(), Ok(result));
    }

    pub(crate) fn fail(&self, method: &str, message: &str) {
        lock(&self.answers).insert(method.to_owned(), Err(message.to_owned()));
    }

    /// The methods sent, in order.
    pub(crate) fn methods(&self) -> Vec<String> {
        lock(&self.sent)
            .iter()
            .filter_map(|message| message["method"].as_str().map(str::to_owned))
            .collect()
    }

    /// Every message sent, in order.
    pub(crate) fn sent(&self) -> Vec<Value> {
        lock(&self.sent).clone()
    }

    /// The parameters of every message sent with `method`, in order.
    pub(crate) fn params(&self, method: &str) -> Vec<Value> {
        lock(&self.sent)
            .iter()
            .filter(|message| message["method"] == method)
            .map(|message| message["params"].clone())
            .collect()
    }

    /// Delivers an event as the browser would.
    pub(crate) fn emit(&self, message: Value) {
        if let Some(router) = lock(&self.router).upgrade() {
            router.on_message(message);
        }
    }
}

impl CdpTransport for Scripted {
    fn send(&self, frame: Vec<u8>) -> Result<(), EngineError> {
        let body = frame.strip_suffix(&[0]).unwrap_or(&frame);
        let message: Value = serde_json::from_slice(body)
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        lock(&self.sent).push(message.clone());
        let method = message["method"].as_str().unwrap_or_default();
        let reply = match lock(&self.answers).get(method) {
            Some(Ok(result)) => json!({ "id": message["id"], "result": result }),
            Some(Err(text)) => {
                json!({ "id": message["id"], "error": { "code": -32000, "message": text } })
            }
            None => json!({ "id": message["id"], "result": {} }),
        };
        if let Some(router) = lock(&self.router).upgrade() {
            router.on_message(reply);
        }
        Ok(())
    }
}
