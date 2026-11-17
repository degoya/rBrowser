//! The MCP tool `reload` (RB-030-01 registers it): its name, description, arguments and answer.
//! The commands `reload.all` and `reload.css_only` take the same arguments, so the toolbar, the
//! shortcuts, `rbrowser --reload` and MCP share one path.

use rb_engine::{Target, ViewId};
use serde::{Deserialize, Serialize};

/// The tool's name.
pub const TOOL_NAME: &str = "reload";
/// The tool's description for the model.
pub const TOOL_DESCRIPTION: &str = "Reload the views. mode \"full\" (default) reloads the pages; \
     mode \"css\" swaps every stylesheet in place without a reload, so JS state, scroll position \
     and form values stay. target is \"all\" (default) or a view id; ignoreCache bypasses the \
     cache of a full reload. Pages with their own HMR client are reloaded too.";

/// How to reload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReloadKind {
    /// Reload the page.
    #[default]
    Full,
    /// Swap the stylesheets only.
    Css,
}

/// The arguments of `reload` (and of the commands `reload.all`, `reload.css_only`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReloadArgs {
    /// `"all"` or a view id.
    #[serde(default = "all")]
    pub target: Target,
    /// `"full"` or `"css"`.
    #[serde(default)]
    pub mode: ReloadKind,
    /// A full reload bypasses the cache.
    #[serde(default)]
    pub ignore_cache: bool,
}

impl Default for ReloadArgs {
    fn default() -> Self {
        Self {
            target: Target::All,
            mode: ReloadKind::Full,
            ignore_cache: false,
        }
    }
}

impl ReloadArgs {
    /// The arguments in `value`; `null` (a shortcut, the toolbar) gives the defaults.
    ///
    /// # Errors
    ///
    /// The reason, for arguments that do not read.
    pub fn from_value(value: &serde_json::Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())
    }

    /// Whether `value` asks for a full reload: `reload.all` then goes to navigation sync, which
    /// reads its arguments - `view` (a decoupled view's address field) included - and reloads
    /// the views (RB-020-06). Only `mode` decides here; `mode: "css"`, a mode that does not read
    /// and arguments that are no object stay hot reload's, which refuses what it cannot read.
    pub fn is_full(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Null => true,
            serde_json::Value::Object(fields) => match fields.get("mode") {
                None | Some(serde_json::Value::Null) => true,
                Some(mode) => serde_json::from_value::<ReloadKind>(mode.clone())
                    .is_ok_and(|mode| mode == ReloadKind::Full),
            },
            _ => false,
        }
    }
}

fn all() -> Target {
    Target::All
}

/// What happened in one view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ViewAction {
    /// The page was reloaded.
    Reloaded,
    /// Stylesheets and images were swapped in place.
    Swapped {
        /// How many links, images and rules.
        count: u64,
    },
    /// The page's own HMR client updates it (`reload.respectHmr`); hot reload left it alone.
    LeftToHmr,
    /// It failed.
    Failed {
        /// Why.
        error: String,
    },
}

/// One view of the answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewReload {
    /// The view.
    pub view: ViewId,
    /// What happened there.
    #[serde(flatten)]
    pub action: ViewAction,
}

/// The answer of `reload`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReloadAnswer {
    /// The mode that ran.
    pub mode: ReloadKind,
    /// Each target view.
    pub views: Vec<ViewReload>,
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
