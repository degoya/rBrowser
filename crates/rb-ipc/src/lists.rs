//! The lists the shell asks for on its own: the project's flows for the command palette
//! (RB-060-11) and the project's breakpoint shortcuts for the shortcut editor (RB-010-16).

use serde::{Deserialize, Serialize};

/// The answer to `listFlows`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowList {
    /// The project's flows by name; empty without a project.
    pub items: Vec<FlowSummary>,
}

/// A flow of the project (`.rbrowser/flows/<name>.ts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSummary {
    /// The file name without `.ts`: what `flows.run` takes as `name`.
    pub name: String,
    /// Its `// @description`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// The answer to `listProjectShortcuts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectShortcutList {
    /// Every `breakpoints[].shortcut` of the config, in config order.
    pub items: Vec<ProjectShortcut>,
}

/// A project breakpoint shortcut: in force, or a conflict with its reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectShortcut {
    /// The breakpoint whose `shortcut` it is.
    pub breakpoint: String,
    /// The combination as written in the config.
    pub binding: String,
    /// Why it is not in force: an i18n key (`shortcuts.reserved.*`, `shortcuts.rejected.*`,
    /// `sizes.conflict.chord`); absent while it is in force.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_key: Option<String>,
    /// The command or breakpoint that holds the combination, for a conflict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other: Option<String>,
}
