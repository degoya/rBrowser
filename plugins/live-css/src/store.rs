//! Where the draft is kept: `.rbrowser/out/live-css/draft.json`, and the whole draft as one
//! stylesheet beside it, `draft.css`, to open or copy into the project.
//!
//! A component lives for one call, so every call reads the file again. The project's output folder
//! is kept out of git (`.rbrowser/.gitignore`), but a repository can still commit the file - CSS
//! that changes what every view shows, or reads attribute values out through `url()` - so it is
//! read and written only in a trusted workspace (as image-overlay's and storage-panel's state):
//! the host's answer for a folder workspace trust protects tells whether the workspace is trusted.

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Value, json};

use crate::coded;
use crate::draft::{Block, Draft};

/// The draft, relative to the plugin's out folder (`out.write`) and to the project.
pub const DRAFT_FILE: &str = "draft.json";
pub const DRAFT_PATH: &str = ".rbrowser/out/live-css/draft.json";
/// The draft as one stylesheet, the same two ways.
pub const SHEET_FILE: &str = "draft.css";
pub const SHEET_PATH: &str = ".rbrowser/out/live-css/draft.css";
/// The version of `draft.json` this plugin writes and reads.
pub const VERSION: u64 = 1;
/// A folder workspace trust protects: the host refuses to list it while the workspace is not
/// trusted, before it looks it up (`docs/plugins.md`, `project.list`).
pub const TRUST_PROBE: &str = ".rbrowser/flows";

/// Whether the draft can be read and written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Trusted,
    Untrusted,
    NoProject,
}

impl Access {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::Untrusted => "untrusted",
            Self::NoProject => "noProject",
        }
    }

    /// Why a change cannot be kept; none in a trusted workspace.
    #[must_use]
    pub fn refusal(self) -> Option<Failure> {
        match self {
            Self::Trusted => None,
            Self::Untrusted => Some(coded(FailureKind::AccessDenied, "errors.untrusted")),
            Self::NoProject => Some(coded(FailureKind::NotSupported, "errors.no_project")),
        }
    }
}

/// The workspace as the host's answer for [`TRUST_PROBE`] tells it: a listing or `not_found` is
/// trust, `not_available` no project, anything else no trust.
pub fn access(host: &mut dyn PluginHost) -> Access {
    match host.project_list(TRUST_PROBE) {
        Ok(_) => Access::Trusted,
        Err(failure) => match failure.code.as_deref() {
            Some(codes::NOT_FOUND) => Access::Trusted,
            Some(codes::NOT_AVAILABLE) => Access::NoProject,
            _ => Access::Untrusted,
        },
    }
}

fn parse(bytes: &[u8]) -> Option<Draft> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    if value.get("version")?.as_u64()? != VERSION {
        return None;
    }
    let enabled = value.get("enabled")?.as_bool()?;
    let blocks = value
        .get("blocks")?
        .as_array()?
        .iter()
        .map(Block::from_json)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let draft = Draft { enabled, blocks };
    draft.check().ok()?;
    Some(draft)
}

/// The stored draft; a new one without a file, and one that does not read is left out (logged).
/// Call it in a trusted workspace only ([`access`]).
pub fn read(host: &mut dyn PluginHost) -> Draft {
    let bytes = match host.project_read(DRAFT_PATH) {
        Ok(bytes) => bytes,
        Err(failure) if failure.kind == FailureKind::NotFound => return Draft::default(),
        Err(failure) => {
            let code = failure.code.unwrap_or_default();
            host.log("warn", &format!("draft.json not read: {code}"));
            return Draft::default();
        }
    };
    parse(&bytes).unwrap_or_else(|| {
        host.log("warn", "draft.json is unusable and left out");
        Draft::default()
    })
}

/// Writes the draft and its stylesheet. Call it in a trusted workspace only ([`access`]).
///
/// # Errors
///
/// The host's: no project, or the write failed.
pub fn write(host: &mut dyn PluginHost, draft: &Draft, stylesheet: &str) -> Result<(), Failure> {
    let blocks: Vec<Value> = draft.blocks.iter().map(Block::to_json).collect();
    let file = json!({ "version": VERSION, "enabled": draft.enabled, "blocks": blocks });
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|_| Failure::coded(FailureKind::Other, codes::INVALID))?;
    host.out_write(DRAFT_FILE, &bytes)?;
    host.out_write(SHEET_FILE, stylesheet.as_bytes())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
