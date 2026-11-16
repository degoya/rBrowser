//! The little URL handling the link check needs: the page hands resolved absolute URLs
//! (`a.href`), so only a fragment is dropped, a redirect's `Location` joined and a local host
//! recognised (no hop from a public site to this machine, as the seo crawl keeps it).

/// The longest URL checked, in characters.
pub const MAX_URL_CHARS: usize = 2048;

/// `url` without its `#fragment`.
#[must_use]
pub fn without_fragment(url: &str) -> String {
    url.split('#').next().unwrap_or_default().to_owned()
}

/// Whether `url` is an `http` or `https` URL of a usable length without control characters or
/// credentials.
#[must_use]
pub fn is_checkable(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"));
    let Some(rest) = rest else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty()
        && !authority.contains('@')
        && url.chars().count() <= MAX_URL_CHARS
        && !url.chars().any(|c| c.is_control() || c == ' ')
}

/// The scheme and authority of `url` (`https://example.test:8080`).
#[must_use]
pub fn origin(url: &str) -> Option<&str> {
    let start = url.find("://")? + 3;
    let end = url[start..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |at| start + at);
    Some(&url[..end])
}

/// The host of `url`, lower case, without port and brackets.
#[must_use]
pub fn host(url: &str) -> Option<String> {
    let origin = origin(url)?;
    let authority = &origin[origin.find("://")? + 3..];
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    Some(host.to_ascii_lowercase())
}

/// Whether `url` points at this machine or a private network by its host name or literal
/// address (the host checks the resolved address again).
#[must_use]
pub fn is_local(url: &str) -> bool {
    let Some(host) = host(url) else {
        return false;
    };
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00
            }
        };
    }
    false
}

/// `location` (a redirect's header) resolved against `base`; `None` when it is none we follow.
#[must_use]
pub fn join(base: &str, location: &str) -> Option<String> {
    let location = location.trim();
    if location.is_empty() {
        return None;
    }
    let joined = if location.contains("://") {
        location.to_owned()
    } else if let Some(rest) = location.strip_prefix("//") {
        let scheme = &base[..base.find("://")?];
        format!("{scheme}://{rest}")
    } else if location.starts_with('/') {
        format!("{}{location}", origin(base)?)
    } else {
        let origin = origin(base)?;
        let path = base[origin.len()..]
            .split(['?', '#'])
            .next()
            .unwrap_or_default();
        let directory = path.rfind('/').map_or("/", |at| &path[..=at]);
        format!("{origin}{directory}{location}")
    };
    is_checkable(&joined).then(|| without_fragment(&joined))
}

#[cfg(test)]
#[path = "url_tests.rs"]
mod tests;
