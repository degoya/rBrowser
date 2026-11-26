//! The payloads of the findings panel's requests (RB-060-01) - `runChecks`, `highlightFinding` -
//! apart from `shell_to_core.rs` so that file stays under the length limit; re-exported there.
//! Internally tagged newtype variants of plain structs: the JSON is the same as with the fields
//! in the variant.

use rb_engine::{Target, ViewId};
use serde::{Deserialize, Serialize};

/// `runChecks`: run every enabled check plugin over the views of `target`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunChecksRequest {
    /// `"all"` or a view id.
    pub target: Target,
    /// Only these check plugins (ids).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Vec<String>>,
    /// The page to check instead of each view's own (`http(s)`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Only the views of these breakpoints: `"visible"` or a list of breakpoint ids
    /// (RB-099-10); every view of `target` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<rb_config::ViewScope>,
}

/// `highlightFinding`: mark a finding's element in `views` and scroll it into view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightFindingRequest {
    /// The finding's id.
    pub finding_id: String,
    /// Its element's CSS selector.
    pub selector: String,
    /// The views to mark it in.
    pub views: Vec<ViewId>,
}
