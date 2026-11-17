//! One outside client over the shared pipe (ADR 0001 Decision 4). Every client gets a flat
//! session of its own - a browser session (`Target.attachToBrowserTarget`) for
//! `/devtools/browser/<id>`, the page's session for `/devtools/page/<targetId>` - and talks to
//! it as if it had the connection to itself:
//!
//! - a message without `sessionId` goes to the client's root session; one with a `sessionId`
//!   only to a child session this client attached (`Target.attachToTarget`, flat) or was
//!   announced (`Target.attachedToTarget` of its auto-attach);
//! - its answers carry its own ids (the app's router numbers the pipe's);
//! - [`crate::policy`] answers or refuses some methods; every `targetId` a client names is
//!   checked against the [`TargetFilter`] first ([`Call::target_check`]), and a target it was
//!   shown as hidden is refused at once with Chromium's "No target with given id found";
//! - `Target.getTargets` and every `Target.*` event lose the hidden targets; a hidden target the
//!   client's auto-attach attached is detached again silently ([`ToClient::DetachHidden`]),
//!   which also releases `waitForDebuggerOnStart`;
//! - `Input.*` on a page session is scaled to its view ([`crate::input`]).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use crate::input::scale_input;
use crate::policy::{Verdict, verdict};
use crate::target_filter::{TargetFilter, hides_info};

/// CDP's server error: a refused or failed call.
pub const SERVER_ERROR: i64 = -32000;
/// An unknown session, as Chromium answers it.
pub const NO_SESSION: i64 = -32001;
/// Invalid parameters; also Chromium's code for an unknown target.
pub const INVALID_PARAMS: i64 = -32602;
/// A message that is no call.
pub const INVALID_REQUEST: i64 = -32600;
/// Chromium's message for a target id it does not know.
pub const NO_TARGET: &str = "No target with given id found";

/// A failed call, as the browser answered it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallError {
    /// The CDP error code; 0 when the call failed without one (timeout, closed pipe).
    pub code: i64,
    /// The message.
    pub message: String,
}

/// One call of the client, to send on the pipe.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    /// The client's id, for the answer.
    pub id: Value,
    /// The method.
    pub method: String,
    /// Its parameters (input already scaled).
    pub params: Value,
    /// The session on the pipe: the client's root or one of its children.
    pub session: String,
    /// The child session the client named; the answer carries it back.
    pub child: Option<String>,
    /// A `targetId` to check against the filter before the call is sent; when it is hidden
    /// the answer is [`ClientSession::no_target`].
    pub target_check: Option<String>,
}

/// What a client message becomes.
#[derive(Clone, Debug, PartialEq)]
pub enum FromClient {
    /// A call to send.
    Call(Call),
    /// An answer for the client at once.
    Answer(Value),
    /// Nothing at all: not even an id to answer.
    Drop,
}

/// What a browser event becomes.
#[derive(Clone, Debug, PartialEq)]
pub enum ToClient {
    /// The message for the client.
    Forward(Value),
    /// Not the client's, or hidden.
    Skip,
    /// A hidden target was auto-attached for the client: detach `session` on the session `on`,
    /// without a word to the client.
    DetachHidden {
        /// The session the attach was announced on.
        on: String,
        /// The hidden target's new session.
        session: String,
    },
    /// The client's root session is gone: the connection ends.
    Gone,
}

/// The state of one client connection.
#[derive(Debug)]
pub struct ClientSession {
    root: String,
    /// The root's target for a page connection.
    root_target: Option<String>,
    /// Child session to target id.
    children: BTreeMap<String, String>,
    /// The hidden targets this client was shown and denied.
    hidden: BTreeSet<String>,
    /// Every target the client attached, whose emulation is re-applied when it leaves.
    touched: BTreeSet<String>,
}

impl ClientSession {
    /// A client of `/devtools/browser/<id>` over the browser session `root`.
    #[must_use]
    pub fn browser(root: String) -> Self {
        Self::new(root, None)
    }

    /// A client of `/devtools/page/<target>` over the page session `root`.
    #[must_use]
    pub fn page(root: String, target: String) -> Self {
        let mut session = Self::new(root, Some(target.clone()));
        session.touched.insert(target);
        session
    }

    fn new(root: String, root_target: Option<String>) -> Self {
        Self {
            root,
            root_target,
            children: BTreeMap::new(),
            hidden: BTreeSet::new(),
            touched: BTreeSet::new(),
        }
    }

    /// The client's root session on the pipe.
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    /// The child sessions the client holds now.
    #[must_use]
    pub fn children(&self) -> Vec<String> {
        self.children.keys().cloned().collect()
    }

    /// Every target the client attached.
    #[must_use]
    pub fn touched(&self) -> Vec<String> {
        self.touched.iter().cloned().collect()
    }

    /// Reads one text message of the client; `scale` gives a target's scale (CSS px to DIP).
    pub fn read_message(&self, text: &str, scale: &dyn Fn(&str) -> Option<f64>) -> FromClient {
        let Ok(Value::Object(message)) = serde_json::from_str::<Value>(text) else {
            return FromClient::Drop;
        };
        let Some(id) = message.get("id").filter(|id| id.is_u64()).cloned() else {
            return FromClient::Drop;
        };
        let child = message
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let refuse =
            |code: i64, text: &str| FromClient::Answer(error(&id, child.as_deref(), code, text));
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return refuse(INVALID_REQUEST, "Invalid request");
        };
        let mut params = match message.get("params") {
            None | Some(Value::Null) => Value::Object(Map::new()),
            Some(params @ Value::Object(_)) => params.clone(),
            Some(_) => return refuse(INVALID_REQUEST, "Invalid parameters"),
        };
        let (session, target) = match &child {
            None => (self.root.clone(), self.root_target.as_deref()),
            Some(child) => match self.children.get(child) {
                Some(target) => (child.clone(), Some(target.as_str())),
                None => return refuse(NO_SESSION, "Session with given id not found."),
            },
        };
        match verdict(method) {
            Verdict::Forward => {}
            Verdict::Owned => {
                return FromClient::Answer(success(&id, child.as_deref(), json!({})));
            }
            Verdict::Refuse(reason) => return refuse(SERVER_ERROR, reason),
        }
        if let Some(refused) = self.method_rule(method, &params) {
            return refuse(refused.0, refused.1);
        }
        let target_check = params
            .get("targetId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if target_check
            .as_ref()
            .is_some_and(|t| self.hidden.contains(t))
        {
            return refuse(INVALID_PARAMS, NO_TARGET);
        }
        if let Some(scale) = target.and_then(scale) {
            scale_input(method, &mut params, scale);
        }
        FromClient::Call(Call {
            id,
            method: method.to_owned(),
            params,
            session,
            child,
            target_check,
        })
    }

    /// The rules of single methods: only flat attaches, detaches only of own children.
    fn method_rule(&self, method: &str, params: &Value) -> Option<(i64, &'static str)> {
        match method {
            "Target.attachToTarget" if params.get("flatten") != Some(&Value::Bool(true)) => Some((
                SERVER_ERROR,
                "only flat sessions are supported (attachToTarget with flatten: true)",
            )),
            "Target.detachFromTarget" => match params.get("sessionId").and_then(Value::as_str) {
                Some(session) if self.children.contains_key(session) => None,
                Some(_) => Some((NO_SESSION, "Session with given id not found.")),
                None => Some((INVALID_PARAMS, "detachFromTarget needs the sessionId")),
            },
            _ => None,
        }
    }

    /// The answer to `call` when its target is hidden or unknown.
    #[must_use]
    pub fn no_target(call: &Call) -> Value {
        error(&call.id, call.child.as_deref(), INVALID_PARAMS, NO_TARGET)
    }

    /// The answer to `call`: `Target.getTargets` without hidden targets, the session an attach
    /// gave recorded as the client's.
    pub fn answer(
        &mut self,
        call: &Call,
        reply: Result<Value, CallError>,
        filter: &dyn TargetFilter,
    ) -> Value {
        let mut result = match reply {
            Ok(result) => result,
            Err(CallError { code, message }) => {
                let code = if code == 0 { SERVER_ERROR } else { code };
                return error(&call.id, call.child.as_deref(), code, &message);
            }
        };
        match call.method.as_str() {
            "Target.getTargets" => {
                if let Some(infos) = result.get_mut("targetInfos").and_then(Value::as_array_mut) {
                    infos.retain(|info| self.judge(info, filter));
                }
            }
            "Target.attachToTarget" => {
                let session = result.get("sessionId").and_then(Value::as_str);
                let target = call.params.get("targetId").and_then(Value::as_str);
                if let (Some(session), Some(target)) = (session, target) {
                    self.children.insert(session.to_owned(), target.to_owned());
                    self.touched.insert(target.to_owned());
                }
            }
            _ => {}
        }
        success(&call.id, call.child.as_deref(), result)
    }

    /// Whether the `TargetInfo` `info` may be shown; a hidden one is remembered.
    fn judge(&mut self, info: &Value, filter: &dyn TargetFilter) -> bool {
        let id = info
            .get("targetId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if self.hidden.contains(id) || hides_info(filter, info) {
            self.hidden.insert(id.to_owned());
            return false;
        }
        true
    }

    /// Reads one browser event: `method` with `params`, from `session` (`None` for the app's
    /// own browser-level connection).
    pub fn read_event(
        &mut self,
        method: &str,
        params: &Value,
        session: Option<&str>,
        filter: &dyn TargetFilter,
    ) -> ToClient {
        let announced = params.get("sessionId").and_then(Value::as_str);
        let Some(session) = session else {
            let gone =
                method == "Target.detachedFromTarget" && announced == Some(self.root.as_str());
            return if gone { ToClient::Gone } else { ToClient::Skip };
        };
        let own = session == self.root;
        if !own && !self.children.contains_key(session) {
            return ToClient::Skip;
        }
        if let Some(info) = params.get("targetInfo")
            && !self.judge(info, filter)
        {
            return match (method, announced) {
                ("Target.attachedToTarget", Some(hidden)) => ToClient::DetachHidden {
                    on: session.to_owned(),
                    session: hidden.to_owned(),
                },
                _ => ToClient::Skip,
            };
        }
        if let Some(target) = params.get("targetId").and_then(Value::as_str)
            && (self.hidden.contains(target) || filter.is_hidden(target, ""))
        {
            if method == "Target.targetDestroyed" {
                self.hidden.remove(target);
            }
            return ToClient::Skip;
        }
        match (method, announced) {
            ("Target.attachedToTarget", Some(child)) => {
                let target = params
                    .pointer("/targetInfo/targetId")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                self.children.insert(child.to_owned(), target.to_owned());
                self.touched.insert(target.to_owned());
            }
            // A side effect in the guard: the child is forgotten once it is detached.
            ("Target.detachedFromTarget", Some(child)) if self.children.remove(child).is_none() => {
                return ToClient::Skip;
            }
            _ => {}
        }
        let mut message = json!({ "method": method, "params": params });
        if !own {
            message["sessionId"] = json!(session);
        }
        ToClient::Forward(message)
    }
}

fn success(id: &Value, child: Option<&str>, result: Value) -> Value {
    let mut message = json!({ "id": id, "result": result });
    if let Some(child) = child {
        message["sessionId"] = json!(child);
    }
    message
}

fn error(id: &Value, child: Option<&str>, code: i64, text: &str) -> Value {
    let mut message = json!({ "id": id, "error": { "code": code, "message": text } });
    if let Some(child) = child {
        message["sessionId"] = json!(child);
    }
    message
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
