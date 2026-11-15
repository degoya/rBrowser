//! The core's messages: answers and pushed events ([`CoreToShell`]).

use std::collections::BTreeMap;

use rb_commands::WhichKey;
use rb_config::DevicePreset;
use rb_engine::{Emulation, ViewId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::state::{SettingValue, StateSnapshot};
use crate::lists::{FlowList, ProjectShortcutList};
use crate::payloads::{
    AboutInfo, BindingCheck, BreakpointProposal, CommandInfo, ExtensionPrompt, Notification,
    ProjectOff, SettingIssue, TrustInfo, TrustPrompt, WebStoreExtension,
};
use crate::status::StatusMessage;

/// A message of the core: the answer to a request, or an event pushed to the shell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CoreToShell {
    /// The request was done; nothing to return.
    Ack,
    /// The whole state the toolbar and the cell labels show; also pushed on every change.
    State(Box<StateSnapshot>),
    /// Setting values, and the schema when asked.
    Settings {
        /// Every key's value and origin.
        values: Vec<SettingValue>,
        /// The merged JSON schema with its `x-*` annotations.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema: Option<Value>,
    },
    /// The command list.
    Commands {
        /// The leader in text form (`Ctrl+Shift+Space`).
        leader: String,
        /// Every command.
        commands: Vec<CommandInfo>,
        /// The title messages of the plugin commands from their plugins' catalogues, by title
        /// key: the shell's catalogue is the core's only (RB-080-82).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        texts: BTreeMap<String, String>,
    },
    /// A message for the user.
    Notification(Notification),
    /// The project asks for workspace trust (RB-010-09).
    TrustPrompt(TrustPrompt),
    /// Version information.
    About(AboutInfo),
    /// Run a command that lives in the shell (`view.focus_address_bar`, the palette, the
    /// shortcut editor): the core resolved a shortcut or MCP to it.
    UiCommand {
        /// The command id.
        id: String,
    },
    /// Show or update the which-key overlay after the leader (RB-010-17).
    WhichKey(WhichKey),
    /// Hide the which-key overlay: a command ran, the chord was cancelled or timed out.
    WhichKeyHide,
    /// The answer to `checkBinding`.
    BindingCheck(BindingCheck),
    /// The answer to `exportBindings`.
    BindingsExport {
        /// The shortcut file, pretty JSON.
        json: String,
    },
    /// The answer to `getEmulation` (RB-010-05).
    Emulation {
        /// The view.
        view: ViewId,
        /// Its emulation now.
        emulation: Emulation,
    },
    /// The answer to `getTrust` and `revokeTrust` (RB-010-09).
    Trust(TrustInfo),
    /// The answer to `pickPath`: the chosen path, absent when cancelled (RB-010-08).
    PickedPath {
        /// The path.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    /// "Save diagnostics..." finished (RB-080-73): the ZIP's path, absent when the dialog was
    /// cancelled or the write failed (`failed`).
    DiagnosticsSaved {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(default)]
        failed: bool,
    },
    /// The answer to `getSettingSources` (RB-080-72): per `x-options` source its values,
    /// `{ options: [{ value, label }], open?, loading? }`.
    SettingSources {
        sources: serde_json::Map<String, Value>,
    },
    /// A source's values changed (the check rules were enumerated): the settings window asks
    /// `getSettingSources` again.
    SettingSourcesChanged,
    /// The answer to `getDevices` (RB-010-12).
    Devices {
        /// The presets in effect, built-in and the user's.
        items: Vec<DevicePreset>,
    },
    /// The analyser's result, pushed after `analyzeBreakpoints` (RB-010-13).
    BreakpointProposal(BreakpointProposal),
    /// Settings changed (in any window, by a file change or a command): every open shell page
    /// reads them again (RB-010-08).
    SettingsChanged {
        /// The dotted keys.
        keys: Vec<String>,
    },
    /// The answer to a `setSetting` that was not written: why, per field (RB-010-08).
    SettingRejected {
        /// The key asked for.
        key: String,
        /// The problems.
        issues: Vec<SettingIssue>,
    },
    /// A finished check run, pushed after `runChecks` or `checks.run_all` (RB-060-01).
    Findings {
        /// The run: an `rb_plugin_types::FindingsFile` (`schema/findings.v1.json`), as JSON so
        /// this crate depends on no plugin crate.
        run: Value,
        /// The texts of the plugin message keys its findings name, unformatted, by key.
        messages: BTreeMap<String, String>,
        /// What the project's settings turned off over the user's; left out when nothing.
        #[serde(default, skip_serializing_if = "ProjectOff::is_empty")]
        project_off: ProjectOff,
        /// The run so far, while it goes: the plugins through so far (RB-080-89).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        partial: bool,
    },
    /// The answer to `listFlows` (RB-060-11).
    Flows(FlowList),
    /// The answer to `listProjectShortcuts` (RB-010-16, the shortcut editor's conflict lines).
    ProjectShortcuts(ProjectShortcutList),
    /// The answer to `pluginPanel`, or an event for the plugin panels (`{"event": {"plugin",
    /// "view"?, "message"}}`), written by `rbrowser::plugins` (RB-060-12).
    PluginPanel {
        /// The answer or the event.
        message: Value,
    },
    /// A message for the status bar (RB-080-15): every notification, numbered and stamped by
    /// the core's log; sent beside `notification`.
    StatusMessage(StatusMessage),
    /// The answer to `getStatusMessages`: the log, oldest first (RB-080-15).
    StatusMessages {
        /// The messages.
        items: Vec<StatusMessage>,
    },
    /// The answer to `getPortal`, with the open pairing (RB-080-05).
    Portal(Box<crate::PortalState>),
    /// The portal or its devices changed: the panel asks `getPortal` again (RB-080-05).
    PortalChanged,
    /// An extension from the Web Store waits for the person's word before it is installed or
    /// updated (RB-080-23) or removed (RB-080-30); answered with `extensionDecision`.
    ExtensionPrompt(ExtensionPrompt),
    /// The extensions installed from the Chrome Web Store, by id: after
    /// `listWebStoreExtensions` and after every change of the store (RB-080-30).
    WebStoreExtensions {
        /// The installed extensions.
        items: Vec<WebStoreExtension>,
    },
    /// The canvas while a gesture moves it (RB-080-50: the middle button's pan, the wheel's
    /// zoom): what the board draws for the camera being tried out, so its frames and labels
    /// stay on the views. Nothing is kept yet; the next `state` is the truth again.
    CanvasLive {
        /// The board as the camera on trial shows it.
        board: crate::BoardState,
    },
    /// A check run's progress (RB-080-89): pushed while it goes and when it ends; the answer
    /// to `getChecksProgress`.
    ChecksProgress(crate::ChecksProgress),
}
