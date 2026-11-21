//! The search result preview (RB-090-95): how the page shows as a result of Google's search on
//! a desktop and on a phone - the site name, the URL as a breadcrumb, the title and the
//! description, with the pixel limits the panel cuts them at (`limits`). Built from the page
//! `seo_social_preview` already loaded (`page::facts`); the favicon is fetched once
//! (`favicon`). The cut itself is the panel's: it measures the text in its font.

use serde::Serialize;

use crate::page::{Facts, says_noindex};
use crate::social::Confidence;
use crate::url::Url;

pub mod favicon;

// The limits of the result, in CSS pixels. The search publishes none: its documentation says only
// that titles and snippets are cut to fit the device's width (sources below). The numbers are the
// ones the result was measured at, as SEO tools use them: on a desktop one title line of 600 px
// at 20 px and two description lines of 600 px at 14 px; on a phone 360 px wide the result card
// leaves 328 px (16 px margins), with up to two title lines (20 px) and three description lines
// (14 px). Checked 2026-11-20 - unverified: the search changes them without notice.

/// When the limits were last checked.
pub const CHECKED: &str = "2026-11-20";
/// Where the search says it cuts by the device's width.
pub const SOURCE: &str = "https://developers.google.com/search/docs/appearance/title-link";
/// The desktop result.
pub const DESKTOP: Device = Device {
    width: 600,
    title_font_px: 20,
    title_lines: 1,
    description_font_px: 14,
    description_lines: 2,
};
/// The result on a phone 360 px wide.
pub const MOBILE: Device = Device {
    width: 328,
    title_font_px: 20,
    title_lines: 2,
    description_font_px: 14,
    description_lines: 3,
};

/// One device's limits: the text's width in pixels, each text's font size and most lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub width: u32,
    pub title_font_px: u32,
    pub title_lines: u32,
    pub description_font_px: u32,
    pub description_lines: u32,
}

/// The limits of both devices, how sure they are, when and where they were checked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub confidence: Confidence,
    pub checked: &'static str,
    pub source: &'static str,
    pub desktop: Device,
    pub mobile: Device,
}

/// The limits as [`DESKTOP`] and [`MOBILE`] state them.
#[must_use]
pub fn limits() -> Limits {
    Limits {
        confidence: Confidence::Unverified,
        checked: CHECKED,
        source: SOURCE,
        desktop: DESKTOP,
        mobile: MOBILE,
    }
}

/// A text of the result and the tag (or `domain`) it came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    pub text: String,
    pub source: String,
}

/// The URL as the result shows it: the host, then the path's segments, decoded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Breadcrumb {
    pub host: String,
    pub segments: Vec<String>,
}

/// The search result of one page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Serp {
    /// `<title>`, else `og:title`; `None` without either.
    pub title: Option<Text>,
    /// The meta description, else `og:description`; `None`: the search shows a passage of the
    /// page instead.
    pub description: Option<Text>,
    pub site_name: Text,
    /// The final URL the result links to.
    pub url: String,
    pub breadcrumb: Breadcrumb,
    /// The robots meta or `X-Robots-Tag` says `noindex`: the page shows in no result.
    pub noindex: bool,
    pub limits: Limits,
}

/// The first non-empty of `own` (as `own_source`), else the social tag `fallback`.
fn text_of(facts: &Facts, own: &[String], own_source: &str, fallback: &str) -> Option<Text> {
    own.iter()
        .find(|text| !text.trim().is_empty())
        .map(|text| Text {
            text: text.clone(),
            source: own_source.to_owned(),
        })
        .or_else(|| {
            facts
                .social
                .get(fallback)
                .filter(|text| !text.trim().is_empty())
                .map(|text| Text {
                    text: text.clone(),
                    source: fallback.to_owned(),
                })
        })
}

/// The site name: `og:site_name`, else the host without `www.`.
#[must_use]
pub fn site_name(facts: &Facts, page: &Url) -> Text {
    match facts
        .social
        .get("og:site_name")
        .filter(|name| !name.trim().is_empty())
    {
        Some(name) => Text {
            text: name.clone(),
            source: "og:site_name".to_owned(),
        },
        None => Text {
            text: page
                .host()
                .strip_prefix("www.")
                .unwrap_or(page.host())
                .to_owned(),
            source: "domain".to_owned(),
        },
    }
}

fn hex(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(16)
        .and_then(|digit| u8::try_from(digit).ok())
}

/// The byte the two hex digits at `at` stand for.
fn escape_at(bytes: &[u8], at: usize) -> Option<u8> {
    let high = hex(*bytes.get(at)?)?;
    let low = hex(*bytes.get(at + 1)?)?;
    Some((high << 4) | low)
}

/// `segment` with its `%XX` escapes decoded as UTF-8 (a byte that is no UTF-8 becomes U+FFFD);
/// a `%` without two hex digits stays as written.
#[must_use]
pub fn decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let escaped = if byte == b'%' {
            escape_at(bytes, at + 1)
        } else {
            None
        };
        match escaped {
            Some(decoded) => {
                out.push(decoded);
                at += 3;
            }
            None => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The breadcrumb of `page`: its host and its non-empty path segments, decoded; no query.
#[must_use]
pub fn breadcrumb(page: &Url) -> Breadcrumb {
    Breadcrumb {
        host: page.host().to_owned(),
        segments: page
            .path()
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(decode)
            .collect(),
    }
}

/// The result of the page at `page` with `facts`; `robots_headers` are its `X-Robots-Tag`s.
#[must_use]
pub fn serp(facts: &Facts, page: &Url, robots_headers: &[&str]) -> Serp {
    let noindex = facts.robots.iter().any(|value| says_noindex(value))
        || robots_headers.iter().any(|value| says_noindex(value));
    Serp {
        title: text_of(facts, &facts.titles, "title", "og:title"),
        description: text_of(facts, &facts.descriptions, "description", "og:description"),
        site_name: site_name(facts, page),
        url: page.to_string(),
        breadcrumb: breadcrumb(page),
        noindex,
        limits: limits(),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
