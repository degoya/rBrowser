//! Where a font comes from (RB-070-13): the host of its file or stylesheet - Google Fonts, Adobe
//! Fonts, the page's own site, another host - an inline `data:` font, an installed one
//! (`local()` or no `@font-face` at all), or one made by script; and its formats.

use rb_util::url::host_of;

use crate::page::Face;

/// Google Fonts' hosts: the CSS API and the font files.
pub const GOOGLE_HOSTS: &[&str] = &["fonts.googleapis.com", "fonts.gstatic.com"];
/// Adobe Fonts' (Typekit) hosts.
pub const ADOBE_HOSTS: &[&str] = &["use.typekit.net", "p.typekit.net", "use.typekit.com"];
/// Second-level labels under which a country's registrable domains sit (`example.co.uk`).
const SECOND_LEVEL: &[&str] = &["co", "com", "org", "net", "ac", "gov", "edu", "or", "ne"];

/// Who serves a font.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Google,
    Adobe,
    /// The page's own site.
    Own,
    /// Any other host.
    External,
    /// A `data:` URL in the stylesheet.
    Inline,
    /// Installed on the system: `local()`, or a family without a web font.
    System,
    /// A `FontFace` of a script, or a sheet the page may not read and cannot be told.
    Unknown,
}

impl Provider {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Adobe => "adobe",
            Self::Own => "own",
            Self::External => "external",
            Self::Inline => "inline",
            Self::System => "system",
            Self::Unknown => "unknown",
        }
    }

    /// Whether the visitor's browser asks a third party for it.
    #[must_use]
    pub fn third_party(self) -> bool {
        matches!(self, Self::Google | Self::Adobe | Self::External)
    }
}

/// The registrable domain of `host`, approximately: its last two labels, three below a
/// country's `co.`, `com.`, ... (`example.co.uk`). An address is itself.
#[must_use]
pub fn site_of(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').collect();
    let numeric = labels.iter().all(|l| l.chars().all(|c| c.is_ascii_digit()));
    if labels.len() <= 2 || numeric || host.starts_with('[') {
        return host.to_owned();
    }
    let n = labels.len();
    let take = if labels[n - 1].len() == 2 && SECOND_LEVEL.contains(&labels[n - 2]) {
        3
    } else {
        2
    };
    labels[n.saturating_sub(take)..].join(".")
}

/// The provider of `url` on a page at `page_url`, and the host.
#[must_use]
pub fn classify(url: &str, page_url: &str) -> (Provider, String) {
    if url.starts_with("data:") {
        return (Provider::Inline, String::new());
    }
    let Some(host) = host_of(url) else {
        return (Provider::Unknown, String::new());
    };
    let provider = if GOOGLE_HOSTS.contains(&host.as_str()) {
        Provider::Google
    } else if ADOBE_HOSTS.contains(&host.as_str()) {
        Provider::Adobe
    } else if host_of(page_url).is_some_and(|page| page == host || site_of(&page) == site_of(&host))
    {
        Provider::Own
    } else {
        Provider::External
    };
    (provider, host)
}

/// The name shown for a provider and host: `Google Fonts`, `Adobe Fonts` or the host.
#[must_use]
pub fn provider_name(provider: Provider, host: &str) -> String {
    match provider {
        Provider::Google => "Google Fonts".to_owned(),
        Provider::Adobe => "Adobe Fonts".to_owned(),
        _ => host.to_owned(),
    }
}

/// The formats of a face's files, normalised (`woff2-variations` is `woff2`; an unknown hint
/// kept as it is).
#[must_use]
pub fn formats(face: &Face) -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    for src in face.src.iter().filter(|src| !src.local) {
        let format = src.format.trim().to_ascii_lowercase();
        let format = format
            .strip_suffix("-variations")
            .unwrap_or(&format)
            .to_owned();
        let format = if format.is_empty() {
            "unknown".to_owned()
        } else {
            format
        };
        if !list.contains(&format) {
            list.push(format);
        }
    }
    list
}

/// The URLs of a face's files (no `local()`).
pub fn urls(face: &Face) -> impl Iterator<Item = &str> {
    face.src
        .iter()
        .filter(|src| !src.local)
        .map(|src| src.url.as_str())
}

/// Where `face` comes from on a page at `page_url`: its first file, else its stylesheet, else
/// `local()` only (installed), else unknown.
#[must_use]
pub fn of_face(face: &Face, page_url: &str) -> (Provider, String) {
    if let Some(url) = urls(face).next() {
        return classify(url, page_url);
    }
    if !face.src.is_empty() {
        return (Provider::System, String::new());
    }
    if !face.sheet.is_empty() && face.sheet != "inline" {
        return classify(&face.sheet, page_url);
    }
    (Provider::Unknown, String::new())
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
