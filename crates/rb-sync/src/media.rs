//! The agent's media report (RB-099-29, `kind: "media"`): a `<video>` or `<audio>` that failed
//! with `MediaError` code 4 (`MEDIA_ERR_SRC_NOT_SUPPORTED`), or a `<source>` that failed, with
//! what the page said (`type`) and what the build said to it (`canPlayType`), once per document
//! and source. Whether a format is missing is the core's (`rbrowser::media_codec`). The
//! addresses come without query and fragment, a data URL as its media type. Mirror:
//! `agent/src/protocol.ts` (`MediaFailure`).

use serde::{Deserialize, Serialize};

/// The element the failure was fired at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaTag {
    /// A `<video>`.
    Video,
    /// An `<audio>`.
    Audio,
    /// A `<source>` of either.
    Source,
}

/// What the build plays of the codecs the standard CEF build lacks, as `canPlayType` says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaPlays {
    /// H.264 in MP4 (`avc1`).
    pub h264: bool,
    /// AAC in MP4 (`mp4a.40.2`).
    pub aac: bool,
    /// HEVC in MP4 (`hvc1`).
    pub hevc: bool,
}

/// A media failure (agent -> core); every frame sends one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMessage {
    /// The protocol version.
    pub v: u32,
    /// Whether the agent runs in the top frame.
    pub top: bool,
    /// As [`crate::Ready::frame`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Vec<String>>,
    /// The reporting document's address.
    pub url: String,
    /// The element.
    pub tag: MediaTag,
    /// `MediaError.code` of a media element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<u16>,
    /// `MediaError.message`, cut by the agent (Chromium's status: `DEMUXER_ERROR_...`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The media's address.
    pub src: String,
    /// A source's `type` attribute.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// What `canPlayType` says to [`Self::mime_type`]: `""`, `"maybe"` or `"probably"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_play: Option<String>,
    /// What the build plays.
    pub plays: MediaPlays,
    /// The element's CSS path.
    pub path: String,
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;
