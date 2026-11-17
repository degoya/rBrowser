//! The agent's framework report (RB-080-24, `kind: "framework"`): what the main-world probe
//! found of Vue and React in the top document, relayed by the agent whenever it changes. The
//! core offers the framework's DevTools panel for the view and, in `auto` mode, puts the hook
//! into the view's next document. Mirror: `agent/src/protocol.ts` (`FrameworkReport`).

use serde::{Deserialize, Serialize};

/// The frameworks whose DevTools rBrowser brings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Framework {
    /// Vue 3 (`@vue/devtools-kit`).
    Vue,
    /// React (`react-devtools-inline`).
    React,
}

impl Framework {
    /// Both, in display order.
    pub const ALL: [Framework; 2] = [Framework::Vue, Framework::React];

    /// The id used in settings keys, commands and panel ids.
    pub const fn id(self) -> &'static str {
        match self {
            Framework::Vue => "vue",
            Framework::React => "react",
        }
    }
}

/// What the probe found of one framework.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameworkInfo {
    /// The framework's version, when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// rBrowser's DevTools hook is in this document, so the panel can connect; else the view
    /// needs a reload once the hook is registered.
    pub hook: bool,
    /// The framework talks to DevTools: false for a Vue production build without
    /// `__VUE_PROD_DEVTOOLS__` and for React loaded before any hook.
    pub devtools: bool,
}

/// The agent's framework report (agent -> core); only the top frame sends one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameworkMessage {
    /// The protocol version.
    pub v: u32,
    /// Whether the agent runs in the top frame.
    pub top: bool,
    /// As [`crate::Ready::frame`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Vec<String>>,
    /// Vue, when the document runs it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vue: Option<FrameworkInfo>,
    /// React, when the document runs it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub react: Option<FrameworkInfo>,
}

impl FrameworkMessage {
    /// What the report says about `framework`.
    pub fn info(&self, framework: Framework) -> Option<&FrameworkInfo> {
        match framework {
            Framework::Vue => self.vue.as_ref(),
            Framework::React => self.react.as_ref(),
        }
    }

    /// The frameworks the document runs, in display order.
    pub fn detected(&self) -> impl Iterator<Item = Framework> + '_ {
        Framework::ALL
            .into_iter()
            .filter(|framework| self.info(*framework).is_some())
    }
}

#[cfg(test)]
#[path = "framework_tests.rs"]
mod tests;
