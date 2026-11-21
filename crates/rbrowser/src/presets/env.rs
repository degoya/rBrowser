//! Environments (RB-010-12): switching rewrites each view's URL to the new environment, keeping
//! path, query and hash; the start environment; the frame colour.

use rb_config::Environment;

/// The URL `current` in the environment at `to`: the part after the old environment's URL
/// (`from`, when `current` lies under it) or after `current`'s origin is appended to `to`. A
/// page outside http(s) (`about:blank`, a data URL, nothing loaded yet) opens `to` itself.
pub fn rewrite_url(current: &str, from: Option<&str>, to: &str) -> String {
    let base = to.trim_end_matches('/');
    let rest = from
        .and_then(|from| rest_under(current, from.trim_end_matches('/')))
        .or_else(|| split_origin(current).map(|(_, rest)| rest));
    match rest {
        Some(rest) if !rest.is_empty() => format!("{base}{rest}"),
        _ => to.to_owned(),
    }
}

/// The environment rBrowser starts in: the first whose URL `start_url` lies under, else the
/// first of the config; `None` without environments.
pub fn initial_environment<'a>(
    environments: &'a [Environment],
    start_url: Option<&str>,
) -> Option<&'a Environment> {
    start_url
        .and_then(|start| {
            environments.iter().find(|environment| {
                rest_under(start, environment.url.trim_end_matches('/')).is_some()
            })
        })
        .or_else(|| environments.first())
}

/// The environment after `current` in config order (the first after the last, or when
/// `current` is none or unknown).
pub fn next_environment<'a>(
    environments: &'a [Environment],
    current: Option<&str>,
) -> Option<&'a str> {
    let next = current
        .and_then(|current| {
            environments
                .iter()
                .position(|environment| environment.id == current)
        })
        .map_or(0, |index| (index + 1) % environments.len().max(1));
    environments
        .get(next)
        .map(|environment| environment.id.as_str())
}

/// The frame colour `#rrggbb` as an opaque ARGB value; `None` for anything else.
pub fn frame_argb(color: &str) -> Option<u32> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16)
        .ok()
        .map(|rgb| 0xFF00_0000 | rgb)
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

/// The part of `url` after `base` when `url` lies under it: the same origin (without case),
/// then the base path up to a segment boundary.
fn rest_under<'a>(url: &'a str, base: &str) -> Option<&'a str> {
    let (origin, rest) = split_origin(url)?;
    let (base_origin, base_path) = split_origin(base)?;
    if !origin.eq_ignore_ascii_case(base_origin) {
        return None;
    }
    let after = rest.strip_prefix(base_path)?;
    let boundary = after.is_empty() || after.starts_with(['/', '?', '#']);
    boundary.then_some(after)
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;
