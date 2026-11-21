//! What `rbrowser init`'s analyser gives (RB-091-08): its proposal with the stylesheets it read
//! ([`Analysed`]), or why it had none ([`NoProposal`]) - a [`Reason`] the setup wizard names to the
//! person (`firstrun.project.reason.<code>`), its values, and the English detail the command line
//! prints on stderr before it writes the mobile + desktop fallback. The fallback stays; it is never
//! silent any more.

use std::collections::BTreeMap;

use rb_config::ProjectConfig;
use rb_plugin_types::codes;

use crate::analyzer::run::AnalyzerError;

pub use rb_ipc::SetupReasonCode as Reason;

/// The analyser's proposal: the config, and how many stylesheets it read.
#[derive(Clone, Debug)]
pub struct Analysed {
    pub config: ProjectConfig,
    pub sheets: usize,
}

/// Why the analyser had no proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoProposal {
    pub reason: Reason,
    /// Values for the wizard's text (a plugin's `code`).
    pub params: BTreeMap<String, String>,
    /// English, for the command line and the log.
    pub detail: String,
}

/// What the analyser gives `init`.
pub type Analysis = Result<Analysed, NoProposal>;

/// A plugin's failure that means it is not loaded: missing, refused at the load, or a project
/// plugin without workspace trust.
const NOT_LOADED: [&str; 9] = [
    codes::NOT_AVAILABLE,
    codes::UNSIGNED,
    codes::SIGNATURE_INVALID,
    codes::KEY_UNTRUSTED,
    codes::KEY_MISMATCH,
    codes::API_VERSION,
    codes::NEEDS_NEWER_APP,
    codes::WORLD_MISSING,
    codes::UNTRUSTED_WORKSPACE,
];

impl NoProposal {
    /// `reason` with `detail` and no values.
    pub fn new(reason: Reason, detail: impl Into<String>) -> Self {
        Self {
            reason,
            params: BTreeMap::new(),
            detail: detail.into(),
        }
    }

    /// This with the value `key`.
    #[must_use]
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }

    /// No page could be read: `error` is `analyzer::fetch::collect`'s; a timeout is named as one.
    pub fn unreachable(error: &str) -> Self {
        let lower = error.to_ascii_lowercase();
        let reason = if lower.contains("timeout") || lower.contains("timed out") {
            Reason::Timeout
        } else {
            Reason::Unreachable
        };
        Self::new(reason, AnalyzerError::Collect(error.to_owned()).to_string())
    }

    /// `error` of the analysis once the pages were read: no stylesheet, the plugin, the config.
    pub fn of(error: &AnalyzerError) -> Self {
        let detail = error.to_string();
        match error {
            AnalyzerError::Collect(_) => Self::new(Reason::NoCss, detail),
            AnalyzerError::Plugin(plugin) => {
                let code = plugin.code.as_str();
                let reason = if NOT_LOADED.contains(&code) {
                    Reason::PluginMissing
                } else if code == codes::BAD_OUTPUT {
                    Reason::BadOutput
                } else {
                    Reason::PluginFailed
                };
                Self::new(reason, detail).with("code", code)
            }
            AnalyzerError::Config(_) => Self::new(Reason::Invalid, detail),
            AnalyzerError::NoUrl => Self::new(Reason::NoPage, detail),
            AnalyzerError::NoProject | AnalyzerError::Busy | AnalyzerError::NoProposal => {
                Self::new(Reason::PluginFailed, detail)
            }
        }
    }

    /// The line `rbrowser init` prints on stderr before it writes the fallback.
    pub fn sentence(&self) -> String {
        let why = match self.reason {
            Reason::NoPage => "there is no page to analyse (pass --url)",
            Reason::Unreachable => "the page could not be reached",
            Reason::Timeout => "the page did not answer in time",
            Reason::NoCss => "the page has no stylesheet to read",
            Reason::PluginMissing => "the breakpoint-analyzer plugin is not loaded",
            Reason::PluginFailed => "the breakpoint-analyzer plugin failed",
            Reason::BadOutput => {
                "the breakpoint-analyzer plugin gave an answer that cannot be used"
            }
            Reason::NothingFound => "the stylesheets have no media query breakpoints",
            Reason::Invalid => "its proposal does not pass the config's checks",
        };
        format!(
            "rbrowser init: the analyser had no proposal: {why} ({}); writing a mobile and a \
             desktop view",
            self.detail
        )
    }
}

#[cfg(test)]
#[path = "init_reason_tests.rs"]
mod tests;
