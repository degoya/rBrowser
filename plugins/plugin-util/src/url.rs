//! Just enough of a URL for a crawl: `http` and `https` only, no user name or password, the
//! fragment dropped. A page URL, a link, a canonical, a sitemap's `<loc>`, a `Sitemap:` line and a
//! redirect's `Location` are all read through [`Url::parse`] or [`Url::join`], so nothing else
//! ever reaches a fetch or a page. Once the copies of `plugins/sitemap-capture` and `plugins/seo`
//! (RB-094-11).

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// The longest URL taken (the sitemap protocol allows 2048 characters; so does the SEO check).
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
        if scheme != "http" && scheme != "https" {
            return None;
        }
        let end = rest.find(['/', '?']).unwrap_or(rest.len());
        let (authority, tail) = rest.split_at(end);
        if authority.is_empty() || authority.contains('@') || authority.starts_with(':') {
            return None;
        }
        let (path, query) = match tail.find('?') {
            Some(at) => tail.split_at(at),
            None => (tail, ""),
        };
        let path = if path.is_empty() { "/" } else { path };
        Some(Self {
            scheme,
            authority: authority.to_ascii_lowercase(),
            path: normalize(path),
            query: query.to_owned(),
        })
    }

    /// `reference` (absolute, scheme-relative, absolute-path, query or relative-path) resolved
    /// against `self`.
    #[must_use]
    pub fn join(&self, reference: &str) -> Option<Self> {
        let reference = reference.trim();
        if !plain(reference) {
            return None;
        }
        if let Some((scheme, _)) = reference.split_once(':')
            && !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            && !reference.starts_with('/')
            && !scheme.contains('/')
            && !scheme.contains('?')
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

    /// `http` or `https`.
    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// `scheme://host[:port]`.
    #[must_use]
    pub fn origin(&self) -> String {
        format!("{}://{}", self.scheme, self.authority)
    }

    /// The host without port and brackets (`www.example.com`, `::1`).
    #[must_use]
    pub fn host(&self) -> &str {
        match self.authority.strip_prefix('[') {
            Some(ipv6) => ipv6.split(']').next().unwrap_or_default(),
            None => self.authority.split(':').next().unwrap_or_default(),
        }
    }

    /// Whether the host is this machine or the local network, where a public site's sitemap
    /// must never send a fetch or a page: `localhost` and `*.localhost`, `*.local`, a name
    /// without a dot (an intranet name, or an address written as one number), a loopback,
    /// private, link-local, shared (`100.64.0.0/10`) or unspecified address, and an IPv4
    /// address in a form other than four decimal parts (`0x7f.1`, which a browser reads as
    /// one).
    #[must_use]
    pub fn is_local(&self) -> bool {
        let host = self.host().trim_end_matches('.');
        if let Ok(address) = host.parse::<IpAddr>() {
            return local_address(address);
        }
        let last = host.rsplit('.').next().unwrap_or_default();
        let numeric = !last.is_empty()
            && (last.chars().all(|c| c.is_ascii_digit())
                || last
                    .strip_prefix("0x")
                    .is_some_and(|hex| hex.chars().all(|c| c.is_ascii_hexdigit())));
        host == "localhost"
            || host.ends_with(".localhost")
            || host.ends_with(".local")
            || !host.contains('.')
            || numeric
    }

    /// The path, always starting with `/`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The path and the query (`/a?b=1`).
    #[must_use]
    pub fn path_and_query(&self) -> String {
        format!("{}{}", self.path, self.query)
    }

    /// The same path and query below `origin` (`http://localhost:8080`); `None` when `origin`
    /// is no `http(s)` origin.
    #[must_use]
    pub fn with_origin(&self, origin: &str) -> Option<Self> {
        let base = Self::parse(origin)?;
        Some(Self {
            scheme: base.scheme,
            authority: base.authority,
            path: self.path.clone(),
            query: self.query.clone(),
        })
    }
}

fn local_v4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_unspecified()
        || address.is_broadcast()
        || (first == 100 && (64..128).contains(&second))
}

fn local_v6(address: Ipv6Addr) -> bool {
    if let Some(v4) = address.to_ipv4_mapped() {
        return local_v4(v4);
    }
    let first = address.segments()[0];
    address.is_loopback()
        || address.is_unspecified()
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
}

fn local_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => local_v4(v4),
        IpAddr::V6(v6) => local_v6(v6),
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
