//! The test end of the CDP connection for "Force state" (RB-090-53): answers by method and
//! parameters, records every message.

use std::sync::{Arc, Mutex, Weak};

use rb_engine::EngineError;
use serde_json::{Value, json};

use super::cdp::CHAIN;
use crate::cdp::{CdpRouter, CdpTransport, lock};

type Answer = dyn Fn(&str, &Value) -> Result<Value, String> + Send + Sync;

/// A browser end that answers by method and parameters, and records every message.
pub(crate) struct Replying {
    router: Mutex<Weak<CdpRouter>>,
    answer: Box<Answer>,
    sent: Mutex<Vec<Value>>,
}

impl Replying {
    pub(crate) fn connect(
        answer: impl Fn(&str, &Value) -> Result<Value, String> + Send + Sync + 'static,
    ) -> (Arc<Self>, Arc<CdpRouter>) {
        let end = Arc::new(Self {
            router: Mutex::new(Weak::new()),
            answer: Box::new(answer),
            sent: Mutex::new(Vec::new()),
        });
        let router = CdpRouter::new(end.clone());
        *lock(&end.router) = Arc::downgrade(&router);
        (end, router)
    }

    /// `(session, method)` of every message, in order.
    pub(crate) fn calls(&self) -> Vec<(String, String)> {
        lock(&self.sent)
            .iter()
            .map(|message| {
                (
                    message["sessionId"].as_str().unwrap_or("-").to_owned(),
                    message["method"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect()
    }

    /// Every message, in order.
    pub(crate) fn sent(&self) -> Vec<Value> {
        lock(&self.sent).clone()
    }

    /// The parameters of every message with `method`, in order.
    pub(crate) fn params(&self, method: &str) -> Vec<Value> {
        lock(&self.sent)
            .iter()
            .filter(|message| message["method"] == method)
            .map(|message| message["params"].clone())
            .collect()
    }
}

impl CdpTransport for Replying {
    fn send(&self, frame: Vec<u8>) -> Result<(), EngineError> {
        let body = frame.strip_suffix(&[0]).unwrap_or(&frame);
        let message: Value = serde_json::from_slice(body)
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        lock(&self.sent).push(message.clone());
        let method = message["method"].as_str().unwrap_or_default();
        let reply = match (self.answer)(method, &message["params"]) {
            Ok(result) => json!({ "id": message["id"], "result": result }),
            Err(text) => {
                json!({ "id": message["id"], "error": { "code": -32000, "message": text } })
            }
        };
        if let Some(router) = lock(&self.router).upgrade() {
            router.on_message(reply);
        }
        Ok(())
    }
}

/// A page whose element `O-el` (backend 42) sits in `<li>` (node 9) in `<html>` (node 8): the
/// chain array answers its items out of order, with a non-index property between them.
pub(crate) fn page(method: &str, params: &Value) -> Result<Value, String> {
    Ok(match method {
        "Target.getTargetInfo" => json!({ "targetInfo": { "targetId": "T1" } }),
        "Target.attachToTarget" => json!({ "sessionId": "OWN" }),
        "DOM.getDocument" => json!({ "root": { "nodeId": 1 } }),
        "DOM.resolveNode" => json!({ "object": { "objectId": "O-el" } }),
        "Runtime.callFunctionOn" if params["functionDeclaration"] == CHAIN => {
            json!({ "result": { "type": "object", "subtype": "array", "objectId": "O-arr" } })
        }
        "Runtime.getProperties" => json!({ "result": [
            { "name": "2", "value": { "objectId": "O-html" } },
            { "name": "length", "value": { "type": "number", "value": 3 } },
            { "name": "0", "value": { "objectId": "O-el" } },
            { "name": "1", "value": { "objectId": "O-li" } },
        ] }),
        "DOM.requestNode" => match params["objectId"].as_str() {
            Some("O-el") => json!({ "nodeId": 10 }),
            Some("O-li") => json!({ "nodeId": 9 }),
            Some("O-html") => json!({ "nodeId": 8 }),
            _ => json!({ "nodeId": 0 }),
        },
        _ => json!({}),
    })
}
