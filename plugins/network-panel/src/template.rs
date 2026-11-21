//! An endpoint's path with its ids as placeholders (RB-096-06): `/api/users/42` is
//! `/api/users/{id}`. A segment of digits is `{id}`, a UUID `{uuid}`, a hex hash of 16 or more
//! characters with a digit or a random-looking token (20 or more letters, digits, `-` and `_`,
//! with a digit and both cases) `{hash}`; words, slugs and versions stay. The person's own
//! patterns (setting `endpoints.patterns`, `/api/products/{slug}`: `{name}` one segment, the rest
//! literal) come first. The query and the fragment are never part of an endpoint, so a token in
//! them never reaches one.

use rb_util::url::origin_of;

/// The longest pattern of the settings, in bytes.
pub const MAX_PATTERN: usize = 512;
/// The most patterns read from the settings.
pub const MAX_PATTERNS: usize = 64;
/// The most segments of a pattern.
const MAX_SEGMENTS: usize = 64;

/// One segment of a pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Literal(String),
    Any,
}

/// A path pattern of the settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    parts: Vec<Part>,
    text: String,
}

fn segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn join(segments: &[&str]) -> String {
    format!("/{}", segments.join("/"))
}

fn placeholder_name(segment: &str) -> Option<&str> {
    let name = segment.strip_prefix('{')?.strip_suffix('}')?;
    let valid = (1..=32).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    valid.then_some(name)
}

impl Template {
    /// `text` as a pattern: starting with `/`, at most [`MAX_PATTERN`] bytes; `None` otherwise or
    /// with a malformed placeholder.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if !text.starts_with('/') || text.len() > MAX_PATTERN {
            return None;
        }
        let found = segments(text);
        if found.len() > MAX_SEGMENTS {
            return None;
        }
        let mut parts = Vec::with_capacity(found.len());
        for segment in &found {
            if segment.contains(['{', '}']) {
                placeholder_name(segment)?;
                parts.push(Part::Any);
            } else {
                parts.push(Part::Literal((*segment).to_owned()));
            }
        }
        Some(Self {
            parts,
            text: join(&found),
        })
    }

    /// The pattern as an endpoint shows it (empty segments dropped).
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    fn matches(&self, segments: &[&str]) -> bool {
        self.parts.len() == segments.len()
            && self
                .parts
                .iter()
                .zip(segments)
                .all(|(part, segment)| match part {
                    Part::Any => true,
                    Part::Literal(literal) => literal.as_str() == *segment,
                })
    }
}

/// The valid patterns of `texts`, at most [`MAX_PATTERNS`].
#[must_use]
pub fn templates(texts: &[String]) -> Vec<Template> {
    texts
        .iter()
        .filter_map(|text| Template::parse(text.trim()))
        .take(MAX_PATTERNS)
        .collect()
}

fn is_uuid(segment: &str) -> bool {
    segment.len() == 36
        && segment.char_indices().all(|(at, c)| match at {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

fn is_hash(segment: &str) -> bool {
    segment.len() >= 16
        && segment.chars().all(|c| c.is_ascii_hexdigit())
        && segment.chars().any(|c| c.is_ascii_digit())
}

fn is_token(segment: &str) -> bool {
    segment.len() >= 20
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && segment.chars().any(|c| c.is_ascii_digit())
        && segment.chars().any(|c| c.is_ascii_lowercase())
        && segment.chars().any(|c| c.is_ascii_uppercase())
}

/// The placeholder of a path segment that is an id; `None` for one that stays.
#[must_use]
pub fn placeholder(segment: &str) -> Option<&'static str> {
    if !segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit()) {
        Some("{id}")
    } else if is_uuid(segment) {
        Some("{uuid}")
    } else if is_hash(segment) || is_token(segment) {
        Some("{hash}")
    } else {
        None
    }
}

/// The origin and the path of an `http(s)` URL, without query and fragment (`/` for none).
#[must_use]
pub fn split(url: &str) -> Option<(String, String)> {
    let origin = origin_of(url)?;
    let (_, rest) = url.split_once("://")?;
    let path = rest
        .find(['/', '?', '#'])
        .map_or("", |at| &rest[at..])
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    let path = if path.is_empty() { "/" } else { path };
    Some((origin, path.to_owned()))
}

/// `path` as an endpoint: the first of `own` that matches, else each segment with its
/// placeholder; empty segments dropped.
#[must_use]
pub fn templated(path: &str, own: &[Template]) -> String {
    let found = segments(path);
    if let Some(template) = own.iter().find(|template| template.matches(&found)) {
        return template.text.clone();
    }
    let parts: Vec<&str> = found
        .iter()
        .map(|segment| placeholder(segment).unwrap_or(*segment))
        .collect();
    join(&parts)
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
