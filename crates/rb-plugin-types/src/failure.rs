//! A failure a plugin or the host reports: a class, a stable code and its parameters.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-types/src/failure.rs`: the classes
//! mirror the WIT `failure-kind` of `rbrowser:plugin@0.1.0` one to one; rCommander's file-manager
//! classes (`exists`, `wrong-password`) are gone, `limit` is new (a rate limit or a size limit).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Translation parameters by name, sorted so a failure's text is reproducible.
pub type MessageParams = BTreeMap<String, String>;

/// The class of a failure, as the WIT enum `failure-kind` names it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    NotFound,
    AccessDenied,
    NotSupported,
    BadData,
    Cancelled,
    Io,
    Limit,
    Other,
}

impl FailureKind {
    /// Every class, in the WIT enum's order.
    pub const ALL: [Self; 8] = [
        Self::NotFound,
        Self::AccessDenied,
        Self::NotSupported,
        Self::BadData,
        Self::Cancelled,
        Self::Io,
        Self::Limit,
        Self::Other,
    ];

    /// The snake-case name, which a failure without a code shows.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::AccessDenied => "access_denied",
            Self::NotSupported => "not_supported",
            Self::BadData => "bad_data",
            Self::Cancelled => "cancelled",
            Self::Io => "io",
            Self::Limit => "limit",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for FailureKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A failure: its class, a stable code (an i18n key) and the code's parameters. No English text:
/// the shell and MCP translate the code.
#[derive(Clone, Debug, Deserialize, Eq, thiserror::Error, PartialEq, Serialize)]
#[error("{}", self.code.as_deref().unwrap_or(self.kind.as_str()))]
pub struct Failure {
    pub kind: FailureKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: MessageParams,
}

impl Failure {
    /// A failure without a code: the class says everything.
    #[must_use]
    pub fn new(kind: FailureKind) -> Self {
        Self {
            kind,
            code: None,
            params: BTreeMap::new(),
        }
    }

    /// A failure carrying a stable code.
    #[must_use]
    pub fn coded(kind: FailureKind, code: &str) -> Self {
        Self {
            kind,
            code: Some(code.to_owned()),
            params: BTreeMap::new(),
        }
    }

    /// Attaches a translation parameter.
    #[must_use]
    pub fn with_param(mut self, key: &str, value: impl ToString) -> Self {
        self.params.insert(key.to_owned(), value.to_string());
        self
    }

    /// The code, if any.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// The parameter `key`, if set.
    #[must_use]
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str)
    }

    /// `plugins.error.not_available` for view `view`, whose engine (`webkit`) cannot serve
    /// `service` (`evaluate`, `cdp`): the host's answer for such a view, and a check plugin's for
    /// one it cannot check. The check runner shows that view skipped, not failed (RB-080-98).
    #[must_use]
    pub fn engine_unserved(service: &str, view: crate::ViewId, engine: &str) -> Self {
        Self::coded(FailureKind::NotSupported, crate::codes::NOT_AVAILABLE)
            .with_param("service", service)
            .with_param("view", view)
            .with_param("engine", engine)
    }

    /// The engine an [`Self::engine_unserved`] failure names when it refused `view` itself;
    /// `None` for every other failure, and for a refusal of another view.
    #[must_use]
    pub fn unserved_engine_in(&self, view: crate::ViewId) -> Option<&str> {
        let own = self.param("view") == Some(view.to_string().as_str());
        if self.code() == Some(crate::codes::NOT_AVAILABLE) && own {
            self.param("engine")
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "failure_tests.rs"]
mod tests;
