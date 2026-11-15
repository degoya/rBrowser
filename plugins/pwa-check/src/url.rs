//! Just enough of a URL for the PWA check: `http` and `https` only, no user name or password,
//! the fragment dropped, the default port dropped. The page, the manifest link, the manifest's
//! `start_url`, `scope` and icon `src`, a redirect's `Location` and the service worker's script
//! are all read through [`Url::parse`] or [`Url::join`], so nothing else ever reaches a fetch.
//! Adapted from `plugins/seo/src/url.rs`.

use std::fmt;

/// The longest URL taken.
pub const MAX_URL_CHARS: usize = 2048;

/// An absolute `http` or `https` URL.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Url {
    scheme: String,
    authority: String,
    path: String,
    query: String,
}

fn plain(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_URL_CHARS
        && !text.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// `path` with its `.` and `..` segments resolved; always starts with `/`.
fn normalize(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let segments: Vec<&str> = path.split('/').skip(1).collect();
    let last = segments.len().saturating_sub(1);
    let mut trailing = false;
    for (index, segment) in segments.iter().enumerate() {
        match *segment {
            "." => trailing = index == last,
            ".." => {
                out.pop();
                trailing = index == last;
            }
            other => {
                out.push(other);
                trailing = false;
            }
        }
    }
    let mut joined = format!("/{}", out.join("/"));
    if trailing && !joined.ends_with('/') {
        joined.push('/');
    }
    joined
}

impl Url {
    /// `text` as an absolute `http(s)` URL; `None` for any other scheme, a URL with credentials,
    /// without a host, with white space or longer than [`MAX_URL_CHARS`].
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if !plain(text) {
            return None;
        }
        let text = text.split('#').next().unwrap_or_default();
        let (scheme, rest) = text.split_once("://")?;
        let scheme = scheme.to_ascii_lowercase();
        let default_port = match scheme.as_str() {
            "http" => ":80",
            "https" => ":443",
            _ => return None,
        };
        let end = rest.find(['/', '?']).unwrap_or(rest.len());
        let (authority, tail) = rest.split_at(end);
        if authority.is_empty() || authority.contains('@') || authority.starts_with(':') {
            return None;
        }
        let authority = authority.to_ascii_lowercase();
        let authority = authority
            .strip_suffix(default_port)
            .unwrap_or(&authority)
            .to_owned();
        let (path, query) = match tail.find('?') {
            Some(at) => tail.split_at(at),
            None => (tail, ""),
        };
        let path = if path.is_empty() { "/" } else { path };
        Some(Self {
            scheme,
            authority,
            path: normalize(path),
            query: query.to_owned(),
        })
    }

    /// `reference` (absolute, scheme-relative, absolute-path, query or relative-path) resolved
    /// against `self`.
    #[must_use]
    pub fn join(&self, reference: &str) -> Option<Self> {
        let reference = reference.trim();
        if reference.is_empty() {
            return Some(self.clone());
        }
        if !plain(reference) {
            return None;
        }
        if let Some((scheme, _)) = reference.split_once(':')
            && !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            && !reference.starts_with('/')
        {
            return Self::parse(reference);
        }
        if let Some(rest) = reference.strip_prefix("//") {
            return Self::parse(&format!("{}://{rest}", self.scheme));
        }
        let reference = reference.split('#').next().unwrap_or_default();
        if reference.is_empty() {
            return Some(self.clone());
        }
        let joined = if reference.starts_with('/') {
            reference.to_owned()
        } else if reference.starts_with('?') {
            format!("{}{reference}", self.path)
        } else {
            let dir = self.path.rsplit_once('/').map_or("", |(dir, _)| dir);
            format!("{dir}/{reference}")
        };
        Self::parse(&format!("{}{joined}", self.origin()))
    }

    /// `scheme://host[:port]`.
    #[must_use]
    pub fn origin(&self) -> String {
        format!("{}://{}", self.scheme, self.authority)
    }

    /// Whether `other` has the same scheme, host and port.
    #[must_use]
    pub fn same_origin(&self, other: &Self) -> bool {
        self.scheme == other.scheme && self.authority == other.authority
    }

    /// Whether `self` lies within the manifest scope `scope`: the same origin and a path that
    /// starts with the scope's path (the Web App Manifest's "within scope").
    #[must_use]
    pub fn within(&self, scope: &Self) -> bool {
        self.same_origin(scope) && self.path.starts_with(&scope.path)
    }

    /// The path, always starting with `/`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The URL without its last path segment and query: the default scope of a manifest.
    #[must_use]
    pub fn directory(&self) -> Self {
        let dir = self.path.rsplit_once('/').map_or("", |(dir, _)| dir);
        Self {
            scheme: self.scheme.clone(),
            authority: self.authority.clone(),
            path: format!("{dir}/"),
            query: String::new(),
        }
    }
}

impl fmt::Display for Url {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}://{}{}{}",
            self.scheme, self.authority, self.path, self.query
        )
    }
}

#[cfg(test)]
#[path = "url_tests.rs"]
mod tests;
