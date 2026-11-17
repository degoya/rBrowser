//! The recorder (RB-050-01): the leader's captured events as a cleaned, versioned
//! [`Recording`] - the model ([`model`], `schema/recording.v1.json`), the recording state
//! machine ([`record`]), the clean-up rules ([`cleanup`]), secret fields ([`secrets`]), the
//! store ([`store`], `.rbrowser/out/recordings/<name>.json`), the boundary to the exporters
//! of RB-050-02 ([`export`]) and the request to the `recorder-export` plugin with the folder its
//! answers go to ([`exports`], `.rbrowser/out/exports/<name>/`). The app feeds it through
//! [`crate::Observer`].

mod cleanup;
pub mod export;
pub mod exports;
mod keys;
pub mod model;
pub mod record;
pub mod secrets;
pub mod store;
#[cfg(test)]
pub(crate) mod testing;

pub use export::{ExportError, Exporter, JsonExporter, from_json, to_json};
pub use exports::{
    DEFAULT_EXPORTS_DIR, ExportOptions, ExportProject, ExportRequest, Exported, ExportsDir,
    ExportsError,
};
pub use model::{
    Action, Assertion, Locator, RECORDING_SCHEMA_FILE, RECORDING_VERSION, RecordedBreakpoint,
    Recording, Selector, Step, candidates, secret_placeholder,
};
pub use record::{
    MAX_STEPS, NAVIGATION_WINDOW_MS, Recorder, RecorderError, RecorderOptions, RecorderState,
    RecordingView, StartInfo,
};
pub use store::{DEFAULT_DIR, RecordingStore, StoreError, check_name};

/// The JSON schema of [`Recording`] as `schema/recording.v1.json` holds it.
pub fn recording_schema() -> String {
    let schema = schemars::schema_for!(Recording);
    let mut text = serde_json::to_string_pretty(&schema).expect("a schema serialises");
    text.push('\n');
    text
}
