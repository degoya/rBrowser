//! The payloads of the Videos panel's requests (RB-110-12, RB-090-52) - `deleteVideo`,
//! `revealVideo`, `convertVideo` - apart from `shell_to_core.rs` so that file stays under the
//! length limit (RB-093-04); re-exported there. Internally tagged newtype variants of plain
//! structs: the JSON is the same as with the fields in the variant.

use serde::{Deserialize, Serialize};

/// A recording, or one of its files: what `deleteVideo` and `revealVideo` name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFileRef {
    /// The recording's id (its folder name).
    pub recording: String,
    /// One of its files; absent: the whole recording (its folder).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

/// What `convertVideo` makes: video `file` of recording `recording` into `to` beside it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertVideoRequest {
    /// The recording's id.
    pub recording: String,
    /// The video, `mobile.webm`.
    pub file: String,
    /// What it is made into.
    pub to: crate::VideoTarget,
}
