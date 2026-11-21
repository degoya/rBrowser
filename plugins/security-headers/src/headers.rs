//! The response headers of a document as the plugin reads them: a foreign server's answer, so
//! held to bounds before anything looks at it - at most [`MAX_HEADERS`] headers, names that are
//! HTTP tokens of at most [`MAX_NAME_CHARS`] characters (anything else dropped), values without
//! control characters cut to [`MAX_VALUE_CHARS`] characters. Names are compared in lower case.

/// The most headers kept of one answer.
pub const MAX_HEADERS: usize = 256;
/// The longest header name kept.
pub const MAX_NAME_CHARS: usize = 128;
/// The longest header value kept, in characters (a long CSP fits).
pub const MAX_VALUE_CHARS: usize = 16_384;

/// The headers of one answer, names in lower case, in the order received.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Headers {
    entries: Vec<(String, String)>,
}

/// Whether `name` is an HTTP token (RFC 9110, `tchar`) of a bounded length.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_CHARS
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/// `text` without control characters (a tab kept), at most `max` characters.
#[must_use]
pub fn bounded(text: &str, max: usize) -> String {
    text.chars()
        .filter(|c| *c == '\t' || !c.is_control())
        .take(max)
        .collect()
}

/// `text` as a finding or an answer shows it: no control character at all, at most `max`
/// characters.
#[must_use]
pub fn shown(text: &str, max: usize) -> String {
    text.chars().filter(|c| !c.is_control()).take(max).collect()
}

impl Headers {
    /// The headers of `pairs` as an answer carries them, held to the bounds.
    #[must_use]
    pub fn from_pairs(pairs: &[(String, String)]) -> Self {
        let entries = pairs
            .iter()
            .filter(|(name, _)| valid_name(name))
            .take(MAX_HEADERS)
            .map(|(name, value)| (name.to_ascii_lowercase(), bounded(value, MAX_VALUE_CHARS)))
            .collect();
        Self { entries }
    }

    /// Every value of `name`, in the order received.
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.entries
            .iter()
            .filter(move |(own, _)| own.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The value of `name`, repeated headers joined with `, ` as HTTP reads them; `None` when
    /// the answer has none.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<String> {
        let values: Vec<&str> = self.all(name).collect();
        (!values.is_empty()).then(|| values.join(", "))
    }

    /// Whether the answer has `name`.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.all(name).next().is_some()
    }

    /// The headers without those named `name`.
    #[must_use]
    pub fn without(&self, name: &str) -> Self {
        let entries = self
            .entries
            .iter()
            .filter(|(own, _)| !own.eq_ignore_ascii_case(name))
            .cloned()
            .collect();
        Self { entries }
    }

    /// How many headers are kept.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no header is kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
#[path = "headers_tests.rs"]
mod tests;
