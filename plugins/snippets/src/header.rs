//! The snippet header (RB-070-04): the comment lines a snippet starts with.
//!
//! ```js
//! // @name Feature flag on
//! // @description Sets the beta flag and reloads
//! // @world main            (main | isolated; default isolated)
//! // @target all            (all | active; where the snippet runs by default)
//! // @timeoutMs 2000        (default `snippets.defaultTimeoutMs`)
//! localStorage.setItem('beta', '1');
//! location.reload();
//! ```
//!
//! The header is every leading line that starts with `//` (blank lines between them included);
//! the first other line ends it. A tag is `@<name> <value>`; unknown tags and plain comment lines
//! are skipped, a known tag with a value that does not read is an error naming the tag (the
//! snippet is listed as invalid and never runs).

use crate::run::RunTarget;
use crate::text::cut;

/// The longest name kept.
pub const MAX_NAME_CHARS: usize = 80;
/// The longest description kept.
pub const MAX_DESCRIPTION_CHARS: usize = 300;
/// The shortest timeout a snippet may ask for.
pub const MIN_TIMEOUT_MS: u32 = 100;
/// The longest timeout: a panel call is answered within the shell's 10 s, and one run in every
/// view must stay inside the component's own time (`run::BUDGET_SECONDS`).
pub const MAX_TIMEOUT_MS: u32 = 9_000;

/// The JavaScript world a snippet runs in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum World {
    /// The plugin's isolated world `rbplugin-snippets`: the DOM is the page's, its globals are not.
    #[default]
    Isolated,
    /// The page's own world: its globals, its frameworks - and it sees the snippet.
    Main,
}

impl World {
    /// `main` or `isolated`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "isolated" => Some(Self::Isolated),
            "main" => Some(Self::Main),
            _ => None,
        }
    }

    /// The name the header and the answers use.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Isolated => "isolated",
            Self::Main => "main",
        }
    }
}

/// What a header says; `None` for a tag it leaves out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    pub name: Option<String>,
    pub description: Option<String>,
    pub world: Option<World>,
    pub target: Option<RunTarget>,
    pub timeout_ms: Option<u32>,
}

/// A known tag whose value does not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderError {
    pub tag: String,
}

fn bad(tag: &str) -> HeaderError {
    HeaderError {
        tag: tag.to_owned(),
    }
}

/// A one-line text: trimmed, cut to `max` characters; `None` when empty or with a control
/// character.
pub(crate) fn line_text(value: &str, max: usize) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    Some(cut(value, max))
}

/// The header of `source`.
///
/// # Errors
///
/// A known tag whose value does not read.
pub fn parse(source: &str) -> Result<Header, HeaderError> {
    let mut header = Header::default();
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(comment) = line.strip_prefix("//") else {
            break;
        };
        let Some(tagged) = comment.trim_start().strip_prefix('@') else {
            continue;
        };
        let (tag, value) = tagged
            .split_once(char::is_whitespace)
            .unwrap_or((tagged, ""));
        let value = value.trim();
        match tag {
            "name" => {
                header.name = Some(line_text(value, MAX_NAME_CHARS).ok_or_else(|| bad(tag))?);
            }
            "description" => {
                header.description =
                    Some(line_text(value, MAX_DESCRIPTION_CHARS).ok_or_else(|| bad(tag))?);
            }
            "world" => header.world = Some(World::parse(value).ok_or_else(|| bad(tag))?),
            "target" => header.target = Some(RunTarget::parse(value).ok_or_else(|| bad(tag))?),
            "timeoutMs" => {
                let ms = value
                    .parse::<u32>()
                    .ok()
                    .filter(|ms| (MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(ms))
                    .ok_or_else(|| bad(tag))?;
                header.timeout_ms = Some(ms);
            }
            _ => {}
        }
    }
    Ok(header)
}

#[cfg(test)]
#[path = "header_tests.rs"]
mod tests;
