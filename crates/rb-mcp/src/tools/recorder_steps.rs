//! The recording editor's steps (RB-092-01): `recorder_step_add`, `recorder_step_update`,
//! `recorder_step_remove`, `recorder_step_toggle` and `recorder_step_try`, all run by the host
//! (`crates/rbrowser/src/recorder/steps.rs`). They change a stored recording's file, checked
//! against `recording.v1`; an open Recorder panel with unsaved edits keeps its draft. No tool
//! deletes a recording (the owner's decision, 2026-11-20) and none answers a recorded value: only
//! the recording's name, the step's id and the count.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve_one};

/// `recorder_step_add`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderStepAddParams {
    /// The stored recording (`recorder_stop` answers its name).
    pub recording_id: String,
    /// The step's action as `recording.v1` writes it, e.g. `{"type":"click","target":{...}}`,
    /// `{"type":"wait","ms":800}` or `{"type":"assert","assertion":{"kind":"visible"}}`; with `x`
    /// and `y` its element (`target`) is the one at that point.
    pub action: Value,
    /// After this step's id; `0` before the first; the end when absent.
    #[serde(default)]
    pub after: Option<u32>,
    /// A name for the step.
    #[serde(default)]
    pub name: Option<String>,
    /// The breakpoints it runs in; all when absent (with `x`/`y`: the views whose page has the
    /// element).
    #[serde(default)]
    pub only_in: Option<Vec<String>>,
    /// The pause before it at the pace `recorded`, ms (at most 10000);
    /// `automation.recorder.insertedStepPauseMs` when absent.
    #[serde(default)]
    pub pause_ms: Option<u32>,
    /// Add it switched off.
    #[serde(default)]
    pub disabled: Option<bool>,
    /// The view `x`/`y` are in (a view id or a breakpoint id shown once); the active one when
    /// absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// CSS px of the page: the element there becomes the step's.
    #[serde(default)]
    pub x: Option<f64>,
    /// See `x`.
    #[serde(default)]
    pub y: Option<f64>,
}

/// `recorder_step_update`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderStepUpdateParams {
    /// The stored recording.
    pub recording_id: String,
    /// The step's id.
    pub step_id: u32,
    /// What changes: any of `name` (null removes it), `action`, `onlyIn`, `disabled`, `pauseMs`
    /// (null: the recorded pause); an absent field stays.
    pub patch: Value,
}

/// `recorder_step_remove` and `recorder_step_toggle`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderStepParams {
    /// The stored recording.
    pub recording_id: String,
    /// The step's id.
    pub step_id: u32,
    /// `recorder_step_toggle` only: switched off (`true`) or on; flipped when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

/// `recorder_step_try`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderStepTryParams {
    /// The stored recording.
    pub recording_id: String,
    /// The step's id.
    pub step_id: u32,
    /// The view to try it in (a view id or a breakpoint id shown once); the active one when
    /// absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

#[tool_router(router = recorder_steps_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Add a step to a stored recording (recordingId) after the step `after` (0: before the first; the end when absent). action is the step as recording.v1 writes it (click, dblclick, hover, fill, select, check, uncheck, press, scroll, navigate, waitForUrl, waitFor, wait { ms <= 10000 }, assert { visible | text | value | url }); with x and y (CSS px of the page in the view `target`, the active one when absent) the element at that point becomes the action's target, named as a recorded click names it, and onlyIn the breakpoints whose views have it. pauseMs is the pause before it at the pace recorded (automation.recorder.insertedStepPauseMs when absent). A secret field's fill (secret: true) is the recorder's own and refused (recorder.secret_step). The file is written over, checked against recording.v1; an open Recorder panel with unsaved edits keeps its draft. Answers the recording, the new step's id and the step count."
    )]
    pub(crate) async fn recorder_step_add(
        &self,
        Parameters(params): Parameters<RecorderStepAddParams>,
    ) -> ToolResult {
        let result = async {
            let view = match params.target.as_ref() {
                Some(target) => {
                    let views = self.read(|backend| backend.views()).await?;
                    resolve_one(Some(target), &views)?
                }
                None => None,
            };
            let args = object([
                ("recordingId", Some(Value::String(params.recording_id))),
                ("action", Some(params.action)),
                ("after", params.after.map(|after| json!(after))),
                ("name", params.name.map(Value::String)),
                ("onlyIn", params.only_in.map(|ids| json!(ids))),
                ("pauseMs", params.pause_ms.map(|ms| json!(ms))),
                ("disabled", params.disabled.map(Value::Bool)),
                ("view", view.map(|view| json!(view.0))),
                ("x", params.x.map(|x| json!(x))),
                ("y", params.y.map(|y| json!(y))),
            ]);
            self.step_tool("recorder_step_add", args).await
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Change one step of a stored recording: patch names what changes - name (null removes it), action (as recording.v1 writes it), onlyIn, disabled, pauseMs (null: the recorded pause; at most 10000). An action that adds or changes a secret field's fill is refused (recorder.secret_step). The file is written over, checked against recording.v1. Answers the recording and the step's id."
    )]
    pub(crate) async fn recorder_step_update(
        &self,
        Parameters(params): Parameters<RecorderStepUpdateParams>,
    ) -> ToolResult {
        let result = async {
            let args = serde_json::to_value(&params)
                .map_err(|error| ToolFailure::new("internal", error.to_string()))?;
            self.step_tool("recorder_step_update", args).await
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Take one step out of a stored recording; the recording itself stays (no tool deletes a recording). Answers the recording, the step's id and the step count left."
    )]
    pub(crate) async fn recorder_step_remove(
        &self,
        Parameters(params): Parameters<RecorderStepParams>,
    ) -> ToolResult {
        let result = async {
            let args = json!({ "recordingId": params.recording_id, "stepId": params.step_id });
            self.step_tool("recorder_step_remove", args).await
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Switch one step of a stored recording off (disabled: true) or on (false); flipped when disabled is absent. A step switched off stays in the recording but neither plays nor is exported. Answers the recording, the step's id and whether it is off."
    )]
    pub(crate) async fn recorder_step_toggle(
        &self,
        Parameters(params): Parameters<RecorderStepParams>,
    ) -> ToolResult {
        let result = async {
            let args = serde_json::to_value(&params)
                .map_err(|error| ToolFailure::new("internal", error.to_string()))?;
            self.step_tool("recorder_step_toggle", args).await
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Try one step of a stored recording alone in one view (target; the active one when absent), at the pace fast, as recorder_play plays it - even a step switched off. The page must be where the step expects it. Answers the run's result as recorder_play does; refused with flows.untrusted until a person trusts the workspace."
    )]
    pub(crate) async fn recorder_step_try(
        &self,
        Parameters(params): Parameters<RecorderStepTryParams>,
    ) -> ToolResult {
        let result = async {
            let views = self.read(|backend| backend.views()).await?;
            let view = resolve_one(params.target.as_ref(), &views)?;
            let args = object([
                ("recordingId", Some(Value::String(params.recording_id))),
                ("stepId", Some(json!(params.step_id))),
                ("view", view.map(|view| json!(view.0))),
            ]);
            self.step_tool("recorder_step_try", args).await
        }
        .await;
        respond(result)
    }
}

impl RbMcpServer {
    /// The host tool `name`, its answer masked like the log.
    async fn step_tool(&self, name: &str, args: Value) -> Result<Value, ToolFailure> {
        let answer = self.backend.host_tool(name, args).await?;
        Ok(self.backend.mask(&answer))
    }
}

#[cfg(test)]
#[path = "recorder_steps_tests.rs"]
mod tests;
