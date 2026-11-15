//! The folder name of a page's baselines (`visual.slugStrategy`): the URL's path by default
//! (`/shop/cart` is `shop-cart`, `/` is `index`), with the host in front (`hostPath`) or the
//! query behind (`pathQuery`). Lower-case ASCII letters, digits and `-` only, so the same page
//! has the same folder on every machine; a long one is cut and keeps a short hash of the whole,
//! and a Windows device name (`/con`) gets a `_` in front. Every slug is a [`valid_name`].
//! A user name and a password in the URL are never part of it.

use serde::{Deserialize, Serialize};

use crate::naming::is_device_name;

/// The longest breakpoint name a baseline file takes.
pub const MAX_NAME: usize = 64;
/// The longest slug: a slug is a baseline folder's name, so it is held to [`MAX_NAME`].
pub const MAX_SLUG: usize = MAX_NAME;
/// The slug of a page without a path.
pub const INDEX: &str = "index";

/// How a page's URL becomes its baseline folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlugStrategy {
    /// The path only: local and staging share their baselines.
    #[default]
    Path,
    /// The host, then the path: each host has its own.
    HostPath,
    /// The path, then the query: `?page=2` has its own.
    PathQuery,
}

impl SlugStrategy {
    /// The strategy named `name` (`path`, `hostPath`, `pathQuery`).
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "path" => Some(Self::Path),
            "hostPath" => Some(Self::HostPath),
            "pathQuery" => Some(Self::PathQuery),
            _ => None,
        }
    }
}

/// The parts of a URL a slug is made of.
struct Parts<'a> {
    host: &'a str,
    path: &'a str,
    query: &'a str,
}

fn parts(url: &str) -> Parts<'_> {
    let url = url.split('#').next().unwrap_or_default();
    let (url, query) = url.split_once('?').unwrap_or((url, ""));
    let Some((_, rest)) = url.split_once("://") else {
        // No scheme: all of it is a path (`about:blank` is `about-blank`).
        return Parts {
            host: "",
            path: url,
            query,
        };
    };
    let (authority, path) = match rest.find('/') {
        Some(at) => rest.split_at(at),
        None => (rest, ""),
    };
    // Never the user name or the password.
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    let host = match host_port.strip_prefix('[') {
        Some(ipv6) => ipv6.split(']').next().unwrap_or_default(),
        None => host_port.split(':').next().unwrap_or_default(),
    };
    Parts { host, path, query }
}

/// The slug of `url` under `strategy`.
pub fn slug(url: &str, strategy: SlugStrategy) -> String {
    let parts = parts(url);
    let mut words = String::new();
    if strategy == SlugStrategy::HostPath {
        words.push_str(parts.host);
        words.push('/');
    }
    words.push_str(parts.path);
    if strategy == SlugStrategy::PathQuery && !parts.query.is_empty() {
        words.push('/');
        words.push_str(parts.query);
    }
    let mut slug = String::with_capacity(words.len());
    for c in words.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        return INDEX.to_owned();
    }
    if slug.len() > MAX_SLUG {
        let hash = format!("{:08x}", fnv1a(&slug));
        slug.truncate(MAX_SLUG - hash.len() - 1);
        while slug.ends_with('-') {
            slug.pop();
        }
        slug.push('-');
        slug.push_str(&hash);
    }
    if is_device_name(&slug) {
        slug.insert(0, '_');
    }
    slug
}

/// The 32-bit FNV-1a hash of `text`.
fn fnv1a(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

/// Whether `name` may name a baseline file or folder: 1 to [`MAX_NAME`] ASCII letters, digits,
/// `.`, `_` and `-`, not starting or ending with `.`, no Windows device name (`con`, `lpt1.png`)
/// - a breakpoint id, `393x852` for a free size.
pub fn valid_name(name: &str) -> bool {
    (1..=MAX_NAME).contains(&name.len())
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !is_device_name(name)
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

#[cfg(test)]
#[path = "slug_tests.rs"]
mod tests;
