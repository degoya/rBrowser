//! The setup wizard's messages (RB-091-05): `{ "type": "setup", "request": { "op": ... } }` from
//! the pages window and `{ "type": "setup", "answer": { "op": ... } }` back. `info` and `close`
//! are answered at once; `preview`, `write` and `basics` are answered with `ack` and their result
//! follows as a `setup` event, since the analyser reads the site and the files may lie in WSL.
//! Those three carry the page's `requestId`, which their event echoes: the core runs them one
//! after the other, and the page takes only the answer to its newest request.
//!
//! RB-091-08: a proposal of the analyser template says what the analyser did - the stylesheets it
//! read (`sheets`) or, when it had no proposal and the views are the mobile + desktop fallback,
//! why (`reason`: a [`SetupReasonCode`] and its values; the text is the page's,
//! `firstrun.project.reason.<code>`). A step that panics answers `failed` with
//! `firstrun.error.step_failed` instead of no answer.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A request of the setup wizard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SetupRequest {
    /// Whether the wizard is switched on and the open project; answered with `setup` `info`.
    Info,
    /// The config the project step would write into `root`; the `proposal` event follows.
    Preview {
        /// The page's number of this request; its answer carries it back.
        request_id: u32,
        /// The project folder, as the folder dialog chose it (a WSL path as `\\wsl.localhost\...`).
        root: String,
        /// The dev server's address; the project's `startUrl`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        /// Where the breakpoints come from.
        template: SetupTemplate,
        /// The device presets of [`SetupTemplate::Devices`], by id.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        devices: Vec<String>,
    },
    /// Writes the proposal `proposal_id` exactly as it was shown; the `written` event follows.
    Write {
        /// The page's number of this request; its answer carries it back.
        request_id: u32,
        /// The proposal's number.
        proposal_id: u32,
    },
    /// The basics step's layout and sync, written into the project the wizard set up (else the
    /// open one); the `applied` event follows. The theme is a user setting (`setSetting`).
    Basics {
        /// The page's number of this request; its answer carries it back.
        request_id: u32,
        /// Grid or canvas; absent: unchanged.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layout: Option<SetupLayout>,
        /// Input sync on or off; absent: unchanged.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sync: Option<bool>,
    },
    /// Closes the wizard's window; answered with `ack`.
    Close,
}

/// Where the project step's breakpoints come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetupTemplate {
    /// The breakpoint analyser's proposal for the dev server, as `rbrowser init` writes it.
    Analyzer,
    /// A mobile and a desktop view, `rbrowser init`'s config without the analyser.
    Default,
    /// One view per chosen device preset.
    Devices,
}

/// Where a proposal's breakpoints came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetupSource {
    /// The analyser read the site.
    Analyzer,
    /// A mobile and a desktop view (also when the analyser had no proposal).
    Default,
    /// The chosen device presets.
    Devices,
}

/// Why the analyser had no proposal (RB-091-08).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupReasonCode {
    /// No web page to read: no address and no `startUrl`.
    NoPage,
    /// The page could not be fetched.
    Unreachable,
    /// The page did not answer in time.
    Timeout,
    /// The page has no stylesheet that could be read.
    NoCss,
    /// The breakpoint analyser plugin is not loaded (not installed, refused or not loaded yet).
    PluginMissing,
    /// The plugin's call failed.
    PluginFailed,
    /// The plugin answered with something that cannot be used.
    BadOutput,
    /// The stylesheets name no breakpoint.
    NothingFound,
    /// The proposal does not pass the config's checks.
    Invalid,
}

/// Why the analyser had no proposal, with values for the text (`url`, a plugin's `code`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupReason {
    /// The reason.
    pub code: SetupReasonCode,
    /// Its values.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

/// The layout the basics step chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetupLayout {
    /// Every view in the grid.
    Grid,
    /// Views placed freely on the canvas.
    Canvas,
}

/// The step a failure belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetupStep {
    /// `preview`.
    Preview,
    /// `write`.
    Write,
    /// `basics`.
    Basics,
}

/// The open project as the wizard sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupProject {
    /// Its folder.
    pub root: String,
    /// Whether it has `.rbrowser/config.json`.
    pub has_config: bool,
}

/// An answer or event of the setup wizard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SetupAnswer {
    /// The answer to `info`.
    Info {
        /// `ui.firstRunWizard`.
        enabled: bool,
        /// The open project; absent without one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<SetupProject>,
        /// The folder the wizard wrote a config into since the app started.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        written: Option<String>,
    },
    /// The config `preview` proposes, nothing written yet.
    Proposal {
        /// The request it answers.
        request_id: u32,
        /// What `write` names.
        proposal_id: u32,
        /// The project folder.
        root: String,
        /// The config exactly as it will be written.
        config: Value,
        /// Where its breakpoints came from.
        source: SetupSource,
        /// The stylesheets the analyser read, when its proposal is the source.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sheets: Option<u32>,
        /// Why the analyser had none, when it was asked and the source is the fallback.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<SetupReason>,
    },
    /// `write` wrote the config.
    Written {
        /// The request it answers.
        request_id: u32,
        /// The project folder.
        root: String,
        /// The config file.
        path: String,
        /// Whether it is the open project, which reloaded its config; otherwise it is a recent
        /// project now that `project.open_recent` opens.
        open: bool,
    },
    /// `basics` wrote the layout and sync into the project at `root`.
    Applied {
        /// The request it answers.
        request_id: u32,
        /// The project folder.
        root: String,
    },
    /// A step failed: the i18n key of the reason and its values.
    Failed {
        /// The request it answers.
        request_id: u32,
        /// The step.
        step: SetupStep,
        /// The reason's i18n key.
        message_key: String,
        /// Values for the reason's placeholders.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        args: BTreeMap<String, String>,
    },
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
