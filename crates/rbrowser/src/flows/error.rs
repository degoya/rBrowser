//! Why a flow does not run, with a stable code for MCP and the CLI.

/// The trust dialog, as every refusal of an untrusted workspace names it.
pub const TRUST_HINT: &str = "open Workspace trust in rBrowser (command trust.show, Ctrl+Shift+Space , T) and trust the project";

/// Why a flow does not run or cannot be listed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FlowError {
    /// No project is open.
    #[error("no project is open")]
    NoProject,
    /// The workspace is not trusted (K9): flows are listed, never run.
    #[error("the flows of this project are blocked until its workspace is trusted: {TRUST_HINT}")]
    Untrusted,
    /// `automation.flows.dir` leaves the protected folder.
    #[error(
        "automation.flows.dir {0:?} must be .rbrowser/flows or a folder inside it (workspace trust covers only that folder)"
    )]
    BadDir(String),
    /// Not a flow name.
    #[error("{0:?} is no flow name (letters, digits, - _ and . only)")]
    BadName(String),
    /// No such flow.
    #[error("there is no flow {name:?}; the project has: {}", list_or_none(.known))]
    NotFound {
        /// The name asked for.
        name: String,
        /// The flows the project has.
        known: Vec<String>,
    },
    /// No flow named, and the project has none or several.
    #[error("name the flow to run; the project has: {}", list_or_none(.0))]
    NoName(Vec<String>),
    /// No view to run in.
    #[error("no view to run the flow in: {0}")]
    NoViews(String),
    /// A flow runs already.
    #[error("the flow {0:?} is running; wait for it or cancel it (flows.cancel)")]
    Busy(String),
    /// No flow ran yet (`flows.rerun_last`, `flows.open_results`).
    #[error("no flow ran yet")]
    NothingYet,
    /// A file could not be read or written.
    #[error("{0}")]
    Io(String),
    /// The Node sidecar failed.
    #[error("the Node sidecar failed: {0}")]
    Sidecar(String),
    /// The views' CDP endpoints could not be opened.
    #[error("the views could not be reached: {0}")]
    Proxy(String),
}

impl FlowError {
    /// The stable code of MCP answers.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoProject => "flows.no_project",
            Self::Untrusted => "flows.untrusted",
            Self::BadDir(_) => "flows.bad_dir",
            Self::BadName(_) | Self::NotFound { .. } | Self::NoName(_) => "flows.not_found",
            Self::NoViews(_) => "flows.no_views",
            Self::Busy(_) => "flows.busy",
            Self::NothingYet => "flows.nothing_yet",
            Self::Io(_) => "flows.io",
            Self::Sidecar(_) => "flows.sidecar",
            Self::Proxy(_) => "flows.proxy",
        }
    }

    /// The setting that decides it, if one does.
    pub fn settings_key(&self) -> Option<&'static str> {
        match self {
            Self::BadDir(_) => Some("automation.flows.dir"),
            Self::Sidecar(_) => Some("engines.nodePath"),
            _ => None,
        }
    }
}

fn list_or_none(names: &[String]) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}
