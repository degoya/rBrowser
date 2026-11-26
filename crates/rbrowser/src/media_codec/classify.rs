//! Which format a failed media element misses (RB-099-29), a pure function of the agent's report
//! (`rb_sync::MediaMessage`): the codecs of a source's `type` first, then its container, then the
//! file name's extension name the format; the evidence that it is missing is the build's own
//! word - `canPlayType` said `""` to the type ([`Evidence::Refused`]), or Chromium's media
//! pipeline failed with a status that means "not supported" ([`Evidence::Decoder`]) - or, for a
//! failure without such a word (a `<source>` tried and dropped, a generic "Format error"), that
//! the build plays no codec of that kind of file ([`Evidence::Likely`]): a build with the
//! proprietary codecs (RB-110-20) is told nothing then. The open formats - WebM, VP8, VP9, AV1,
//! Opus, Vorbis, FLAC, MP3, WAV - are never missing.

use rb_sync::{MediaMessage, MediaPlays, MediaTag};

/// Chromium's pipeline statuses (the start of `MediaError.message`) that mean the build cannot
/// read the file's container or streams.
const NOT_SUPPORTED: [&str; 3] = [
    "DEMUXER_ERROR_NO_SUPPORTED_STREAMS",
    "DEMUXER_ERROR_COULD_NOT_OPEN",
    "DECODER_ERROR_NOT_SUPPORTED",
];
/// `MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED`.
const SRC_NOT_SUPPORTED: u16 = 4;
/// The longest extension read from a file name.
const MAX_EXTENSION: usize = 5;

/// A format the build does not play.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    /// H.264 video (`avc1`, `avc3`).
    H264,
    /// HEVC video (`hvc1`, `hev1`).
    Hevc,
    /// AAC audio (`mp4a.40.*`, `.m4a`, `.aac`).
    Aac,
    /// Dolby AC-3 and E-AC-3 audio.
    Ac3,
    /// An MP4 file without named codecs: H.264 and AAC, as a rule.
    Mp4,
    /// A QuickTime file.
    Mov,
    /// Another format the build refused, by its media type or file extension.
    Other(String),
}

impl Format {
    /// The id the notice is held once per page under.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::H264 => "h264".to_owned(),
            Self::Hevc => "hevc".to_owned(),
            Self::Aac => "aac".to_owned(),
            Self::Ac3 => "ac3".to_owned(),
            Self::Mp4 => "mp4".to_owned(),
            Self::Mov => "mov".to_owned(),
            Self::Other(name) => format!("other:{}", name.to_ascii_lowercase()),
        }
    }

    /// The format's name in a text (a technical name, the same in every language).
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::H264 => "H.264".to_owned(),
            Self::Hevc => "HEVC (H.265)".to_owned(),
            Self::Aac => "AAC".to_owned(),
            Self::Ac3 => "AC-3/E-AC-3".to_owned(),
            Self::Mp4 => "MP4 (H.264, AAC)".to_owned(),
            Self::Mov => "QuickTime (MOV)".to_owned(),
            Self::Other(name) => name.clone(),
        }
    }

    /// Whether a build that plays `plays` plays this format.
    fn played_by(&self, plays: MediaPlays) -> bool {
        match self {
            Self::H264 | Self::Mp4 | Self::Mov => plays.h264,
            Self::Hevc => plays.hevc,
            Self::Aac => plays.aac,
            Self::Ac3 | Self::Other(_) => false,
        }
    }
}

/// Why a format counts as missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evidence {
    /// `canPlayType` said `""` to the source's type.
    Refused,
    /// The media pipeline failed with a "not supported" status.
    Decoder,
    /// The element failed without a verdict, and the build plays no codec of its kind.
    Likely,
}

impl Evidence {
    /// The name in a finding's details.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::Decoder => "decoder",
            Self::Likely => "likely",
        }
    }
}

/// A missing format and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    pub format: Format,
    pub evidence: Evidence,
}

/// What the report's type, container or file name says.
#[derive(Debug, PartialEq, Eq)]
enum Named {
    /// A format the standard build lacks.
    Proprietary(Format),
    /// A format every build plays.
    Open,
    /// Nothing known.
    Unknown,
}

/// The missing format of `media`; `None` when it plays elsewhere or nothing says a format is
/// missing.
#[must_use]
pub fn classify(media: &MediaMessage) -> Option<Missing> {
    let (essence, codecs) = media.mime_type.as_deref().map(parse_type).unzip();
    let ext = extension(&media.src);
    let named = by_codecs(codecs.as_deref().unwrap_or_default())
        .or_else(|| essence.as_deref().and_then(by_essence))
        .or_else(|| ext.as_deref().and_then(by_extension))
        .unwrap_or(Named::Unknown);
    let refused = media.mime_type.is_some() && media.can_play.as_deref() == Some("");
    let decoder = media.tag != MediaTag::Source
        && media.code == Some(SRC_NOT_SUPPORTED)
        && media.message.as_deref().is_some_and(|message| {
            NOT_SUPPORTED
                .iter()
                .any(|status| message.starts_with(status))
        });
    let evidence = if refused {
        Some(Evidence::Refused)
    } else if decoder {
        Some(Evidence::Decoder)
    } else {
        None
    };
    let format = match named {
        Named::Open => return None,
        Named::Proprietary(format) => {
            if evidence.is_none() && format.played_by(media.plays) {
                return None;
            }
            format
        }
        // An unknown format counts only on the build's own word.
        Named::Unknown => evidence.and_then(|_| {
            essence
                .filter(|essence| !essence.is_empty())
                .or_else(|| ext.map(|ext| ext.to_ascii_uppercase()))
                .map(Format::Other)
        })?,
    };
    Some(Missing {
        format,
        evidence: evidence.unwrap_or(Evidence::Likely),
    })
}

/// A media type's essence and its `codecs`, lower case, without quotes and spaces.
#[must_use]
pub fn parse_type(text: &str) -> (String, Vec<String>) {
    let mut parts = text.split(';');
    let essence = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
    let codecs = parts
        .filter_map(|part| part.split_once('='))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("codecs"))
        .map(|(_, value)| {
            value
                .trim()
                .trim_matches('"')
                .split(',')
                .map(|codec| codec.trim().to_ascii_lowercase())
                .filter(|codec| !codec.is_empty())
                .collect()
        })
        .unwrap_or_default();
    (essence, codecs)
}

/// The extension of the last segment of `src`'s path, lower case.
#[must_use]
pub fn extension(src: &str) -> Option<String> {
    let path = match src.split_once("://") {
        Some((_, rest)) => &rest[rest.find('/')?..],
        None if src.starts_with('/') => src,
        None => return None,
    };
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let segment = path.rsplit('/').next().unwrap_or_default();
    let (stem, ext) = segment.rsplit_once('.')?;
    let fits = !stem.is_empty()
        && (1..=MAX_EXTENSION).contains(&ext.len())
        && ext.bytes().all(|byte| byte.is_ascii_alphanumeric());
    fits.then(|| ext.to_ascii_lowercase())
}

/// What the codecs say: the first proprietary one, open when every one is open.
fn by_codecs(codecs: &[String]) -> Option<Named> {
    let mut all_open = !codecs.is_empty();
    for codec in codecs {
        if let Some(format) = proprietary_codec(codec) {
            return Some(Named::Proprietary(format));
        }
        all_open &= open_codec(codec);
    }
    all_open.then_some(Named::Open)
}

fn proprietary_codec(codec: &str) -> Option<Format> {
    let family = codec.split('.').next().unwrap_or_default();
    match family {
        "avc1" | "avc3" => Some(Format::H264),
        "hvc1" | "hev1" => Some(Format::Hevc),
        "ac-3" | "ec-3" => Some(Format::Ac3),
        // `mp4a.69`/`mp4a.6b` is MP3, `mp4a.a5`/`mp4a.a6` Dolby, the rest AAC.
        "mp4a" => match codec.strip_prefix("mp4a.") {
            Some("69" | "6b") => None,
            Some("a5" | "a6") => Some(Format::Ac3),
            _ => Some(Format::Aac),
        },
        _ => None,
    }
}

fn open_codec(codec: &str) -> bool {
    let family = codec.split('.').next().unwrap_or_default();
    matches!(
        family,
        "vp8" | "vp9" | "vp09" | "av01" | "av1" | "opus" | "vorbis" | "flac" | "mp3" | "theora"
    ) || codec == "mp4a.69"
        || codec == "mp4a.6b"
        || codec.starts_with("pcm")
}

fn by_essence(essence: &str) -> Option<Named> {
    match essence {
        "video/mp4" | "video/x-m4v" => Some(Named::Proprietary(Format::Mp4)),
        "audio/mp4" | "audio/x-m4a" | "audio/m4a" | "audio/aac" | "audio/x-aac" => {
            Some(Named::Proprietary(Format::Aac))
        }
        "video/quicktime" => Some(Named::Proprietary(Format::Mov)),
        "video/webm" | "audio/webm" | "video/ogg" | "audio/ogg" | "audio/opus" | "audio/mpeg"
        | "audio/mp3" | "audio/flac" | "audio/x-flac" | "audio/wav" | "audio/x-wav"
        | "audio/wave" => Some(Named::Open),
        _ => None,
    }
}

fn by_extension(ext: &str) -> Option<Named> {
    match ext {
        "mp4" | "m4v" => Some(Named::Proprietary(Format::Mp4)),
        "m4a" | "aac" => Some(Named::Proprietary(Format::Aac)),
        "mov" => Some(Named::Proprietary(Format::Mov)),
        "webm" | "ogg" | "ogv" | "oga" | "opus" | "mp3" | "flac" | "wav" => Some(Named::Open),
        _ => None,
    }
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
