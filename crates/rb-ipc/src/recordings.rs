//! The Recorder and Flows panels' payloads (RB-090-60): the stored recordings (`getRecordings`,
//! `saveRecording`), an export's files (`exportRecording`, a preview when nothing is written)
//! and the flow run going on with the last result (`getFlowRun`). `recordingsChanged` and
//! `flowRunChanged` tell the panels to ask again; `flowStep` pushes each step of a run as it
//! starts and ends (RB-090-84). TypeScript mirror: `ui/src/ipc/recordings.ts`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The stored recordings of the open project, newest first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingList {
    /// A project is open: recordings need one.
    pub project: bool,
    /// The recordings' folder (`automation.recorder.dir`), relative to the project root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Why the folder could not be read; the list is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The recordings, newest first.
    pub recordings: Vec<RecordingEntry>,
    /// The name of the recording running now, if one runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_now: Option<String>,
}

/// One stored recording.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingEntry {
    /// Its name, also its file name.
    pub name: String,
    /// Its steps; 0 when the file could not be read.
    pub steps: u32,
    /// When the file was last written, ms since the epoch.
    pub modified_ms: u64,
}

/// What an export makes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingExportFormat {
    /// The rBrowser flow.
    Flow,
    /// The Playwright test (and its config).
    Spec,
    /// Both.
    #[default]
    Both,
}

/// An export's files: rendered only (a preview) or written.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingExport {
    /// The files were written.
    pub written: bool,
    /// Where they were written, relative to the project root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// The files: their path relative to the project root once written, else their name.
    pub files: Vec<RecordingExportFile>,
    /// A preview with a flow: where "Save" puts it, relative to the project root
    /// (`.rbrowser/flows/<name>.ts`, RB-090-66).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow_file: Option<String>,
    /// A flow of that name exists: "Save" replaces it only with `overwrite`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flow_exists: bool,
}

/// One file of an export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingExportFile {
    /// Its path or name.
    pub path: String,
    /// Its text.
    pub content: String,
}

/// The flow run going on and the last one's result, for the Flows panel.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowRunState {
    /// A project is open.
    pub project: bool,
    /// The run going on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running: Option<FlowRunning>,
    /// The last run's `result.json` (`rbrowser::flows::result::RunResult`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<Value>,
}

/// A run going on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowRunning {
    /// Its id, also its folder's name.
    pub run_id: String,
    /// The flow.
    pub flow: String,
    /// The views' statuses so far: every view of the run `queued` and every view left out
    /// `skipped` at the start, then as the runner reports them (`running`, `passed`, ...).
    pub views: Vec<FlowViewStatus>,
    /// When it started, ms since the epoch (the panel's elapsed time).
    #[serde(default)]
    pub started_at_ms: u64,
    /// The flows of a `flows.run_all` still waiting to run after this one, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued: Vec<String>,
}

/// One view's status in a run going on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowViewStatus {
    /// The view.
    pub view: u32,
    /// Its status.
    pub status: String,
}

/// A step of a flow run started or ended (RB-090-84): the runner's `step(name, ...)` as the
/// sidecar reports it, pushed so the Recorder panel follows a played recording.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowStep {
    /// The run's id.
    pub run_id: String,
    /// The flow; a played recording's name.
    pub flow: String,
    /// The view it runs in.
    pub view: u32,
    /// The step's name (`Step 3: click` in a played recording).
    pub name: String,
    /// The recording's step id, for a name `Step <id>: ...`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<u32>,
    /// `running`, `passed` or `failed`.
    pub state: String,
    /// Why a failed step failed: its error's first line, masked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// The pause before a step of a recording at the pace `recorded` (RB-090-84), as the
/// `recorder-export` plugin writes it into the flow: `getRecording` answers them with the
/// recording, so the Recorder panel shows what Play waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepPause {
    /// The step's id.
    pub step: u32,
    /// The pause before it, in ms.
    pub ms: u64,
}
