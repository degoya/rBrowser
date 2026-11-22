//! A script sandbox for tests (RB-098-01): the "script" is JSON - the tool calls to make, then
//! what to answer - so `script_run`, its bridge and its guards are tested without the engine.
//!
//! `{"calls": [["navigate", {...}], ...], "catch": false, "return": "answers" | <value>,
//! "loop": false}`: each call goes to the bridge in order; a refusal ends the run as the engine's
//! uncaught `ToolError` (line = the call's place, from 1) unless `catch`, which keeps it among the
//! answers as `{"refused": <refusal>}`. `"return": "answers"` answers the calls' answers, any
//! other value itself. `"loop": true` never ends. A text that is no JSON is a syntax failure.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rb_util::lock;
use serde_json::{Value, json};

use crate::scripts::{ScriptRequest, ScriptRunner, ScriptStop, ScriptTools};

/// The fake's slot for a runner, printable as the fake is.
#[derive(Default)]
pub struct ScriptSlot(Mutex<Option<Arc<dyn ScriptRunner>>>);

impl ScriptSlot {
    /// Sets the runner.
    pub fn set(&self, runner: Arc<dyn ScriptRunner>) {
        *lock(&self.0) = Some(runner);
    }

    /// The runner, if set.
    #[must_use]
    pub fn get(&self) -> Option<Arc<dyn ScriptRunner>> {
        lock(&self.0).clone()
    }
}

impl std::fmt::Debug for ScriptSlot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("ScriptSlot")
            .field(&lock(&self.0).is_some())
            .finish()
    }
}

/// The JSON-program sandbox.
#[derive(Debug, Default)]
pub struct FakeScripts {
    runs: Mutex<Vec<ScriptRequest>>,
}

impl FakeScripts {
    /// The runs asked for so far.
    #[must_use]
    pub fn runs(&self) -> Vec<ScriptRequest> {
        lock(&self.runs).clone()
    }
}

#[async_trait]
impl ScriptRunner for FakeScripts {
    async fn run(
        &self,
        request: ScriptRequest,
        tools: Arc<dyn ScriptTools>,
    ) -> Result<String, ScriptStop> {
        lock(&self.runs).push(request.clone());
        let Ok(program) = serde_json::from_str::<Value>(&request.code) else {
            return Err(ScriptStop::Script(
                json!({ "code": "script.syntax", "name": "SyntaxError",
                        "message": "the fake's script is JSON", "line": 1 })
                .to_string(),
            ));
        };
        match tokio::time::timeout(request.timeout, interpret(&program, tools.as_ref())).await {
            Ok(outcome) => outcome,
            Err(_) => Err(ScriptStop::Timeout),
        }
    }
}

async fn interpret(program: &Value, tools: &dyn ScriptTools) -> Result<String, ScriptStop> {
    if program["loop"].as_bool().unwrap_or(false) {
        loop {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    let catch = program["catch"].as_bool().unwrap_or(false);
    let mut answers = Vec::new();
    let calls = program["calls"].as_array().cloned().unwrap_or_default();
    for (index, call) in calls.iter().enumerate() {
        let tool = call[0].as_str().unwrap_or_default();
        let arguments = call.get(1).cloned().unwrap_or_else(|| json!({}));
        match tools.call(tool, &arguments.to_string()).await {
            Ok(answer) => answers.push(serde_json::from_str(&answer).unwrap_or(Value::Null)),
            Err(refusal) => {
                let refusal: Value = serde_json::from_str(&refusal).unwrap_or(Value::Null);
                if catch {
                    answers.push(json!({ "refused": refusal }));
                    continue;
                }
                return Err(ScriptStop::Script(
                    json!({ "code": refusal["code"], "name": "ToolError",
                            "message": refusal["message"], "tool": tool, "line": index + 1 })
                    .to_string(),
                ));
            }
        }
    }
    let value = match &program["return"] {
        Value::String(text) if text == "answers" => Value::Array(answers),
        other => other.clone(),
    };
    Ok(json!({ "value": value, "logs": [] }).to_string())
}
