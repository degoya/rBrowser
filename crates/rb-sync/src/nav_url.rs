//! The URLs of navigation sync (RB-020-06): when two views show the same page, which pages are
//! followed at all, and the URL a follower loads - the leader's, or its path under the
//! follower's environment when the two views are in different environments of the project
//! (the rewrite of the environment switch, RB-010-12: keep path, query and hash, swap the base).

/// The page identity of `url`: scheme and host in lower case, the default port dropped, an
/// empty path as `/`, path and query as written, the fragment only with `include_hash` (an
/// empty fragment is none). URLs outside http(s) stay as written, less the fragment.
pub fn normalize(url: &str, include_hash: bool) -> String {
    let url = url.trim();
    let (base, fragment) = match url.split_once('#') {
        Some((base, fragment)) => (base, Some(fragment)),
        None => (url, None),
    };
    let mut out = match split_origin(base) {
        Some((origin, rest)) => {
            let origin = canonical_origin(origin);
            if rest.is_empty() || rest.starts_with('?') {
                format!("{origin}/{rest}")
            } else {
                format!("{origin}{rest}")
            }
        }
        None => base.to_owned(),
    };
    if include_hash && let Some(fragment) = fragment.filter(|fragment| !fragment.is_empty()) {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

/// Whether `a` and `b` are the same page ([`normalize`]).
pub fn same_page(a: &str, b: &str, include_hash: bool) -> bool {
    normalize(a, include_hash) == normalize(b, include_hash)
}

/// Whether `a` and `b` are http(s) URLs of the same origin.
pub fn same_origin(a: &str, b: &str) -> bool {
    match (split_origin(a), split_origin(b)) {
        (Some((a, _)), Some((b, _))) => canonical_origin(a) == canonical_origin(b),
        _ => false,
    }
}

/// The origin of an http(s) URL (`http://example.com`), in lower case without a default port:
/// `rb_util::url::origin_of`, under the path the sync and the portal always used.
pub use rb_util::url::origin_of;

/// Whether the views follow a leader on `url`: http, https and file pages only - never an
/// error page (`chrome-error:`), `about:`, `data:`, `javascript:` or a browser page.
pub fn followable(url: &str) -> bool {
    let Some((scheme, _)) = url.trim().split_once(':') else {
        return false;
    };
    ["http", "https", "file"]
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
}

/// The URL a follower showing `follower_url` loads to follow the leader to `leader_url`. When
/// the two lie under different environments of `environments` (their base URLs), the leader's
/// path, query and hash under the follower's environment; otherwise the leader's URL.
pub fn follow_url(leader_url: &str, follower_url: &str, environments: &[String]) -> String {
    match (
        environment_of(leader_url, environments),
        environment_of(follower_url, environments),
    ) {
        (Some((leader_base, rest)), Some((follower_base, _)))
            if normalize(leader_base, false) != normalize(follower_base, false) =>
        {
            format!("{follower_base}{rest}")
        }
        _ => leader_url.to_owned(),
    }
}

/// The environment base `url` lies under (without a trailing slash) and the rest of `url`.
fn environment_of<'a, 'b>(url: &'a str, environments: &'b [String]) -> Option<(&'b str, &'a str)> {
    environments.iter().find_map(|base| {
        let base = base.trim_end_matches('/');
        rest_under(url, base).map(|rest| (base, rest))
    })
}

/// `scheme://host[:port]` in lower case, without the scheme's default port.
fn canonical_origin(origin: &str) -> String {
    let lower = origin.to_ascii_lowercase();
    for (scheme, port) in [("http://", ":80"), ("https://", ":443")] {
        if lower.starts_with(scheme)
            && let Some(stripped) = lower.strip_suffix(port)
        {
            return stripped.to_owned();
        }
    }
    lower
}

/// Splits an http(s) URL into its origin and the rest (path, query, hash; may be empty).
fn split_origin(url: &str) -> Option<(&str, &str)> {
    let scheme_end = url.find("://")?;
    let scheme = &url[..scheme_end];
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return None;
    }
    let authority = scheme_end + "://".len();
    let end = url[authority..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |offset| authority + offset);
    Some(url.split_at(end))
}

/// The part of `url` after `base` when `url` lies under it: the same origin, then the base
/// path up to a segment boundary.
fn rest_under<'a>(url: &'a str, base: &str) -> Option<&'a str> {
    let (origin, rest) = split_origin(url)?;
    let (base_origin, base_path) = split_origin(base)?;
    if canonical_origin(origin) != canonical_origin(base_origin) {
        return None;
    }
    let after = rest.strip_prefix(base_path)?;
    let boundary = after.is_empty() || after.starts_with(['/', '?', '#']);
    boundary.then_some(after)
}

#[cfg(test)]
#[path = "nav_url_tests.rs"]
mod tests;
