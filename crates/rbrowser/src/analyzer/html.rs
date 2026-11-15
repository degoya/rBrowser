//! What the fetch source reads from a page without a browser (`rbrowser init`): the
//! stylesheets an HTML document names - `<link rel="stylesheet" href>` with its `media`, and
//! `<style>` blocks - and the `@import`s of a stylesheet, with their URLs resolved against the
//! document. A tolerant scan, not an HTML parser: it only has to find what a browser would load.

/// A stylesheet a document names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SheetRef {
    /// `<link rel="stylesheet" href="...">`, the URL resolved.
    Link { url: String, media: Option<String> },
    /// The text of a `<style>` block.
    Inline { text: String, media: Option<String> },
}

/// The stylesheets of `html` (at `base`), in document order.
pub fn stylesheets(html: &str, base: &str) -> Vec<SheetRef> {
    let lower = html.to_ascii_lowercase();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(offset) = lower[at..].find('<') {
        let start = at + offset;
        if lower[start..].starts_with("<!--") {
            at = lower[start..]
                .find("-->")
                .map_or(lower.len(), |end| start + end + 3);
            continue;
        }
        let Some(end) = lower[start..].find('>').map(|end| start + end) else {
            break;
        };
        let tag = &html[start + 1..end];
        let name = tag_name(tag);
        if name == "link" {
            let rel = attribute(tag, "rel")
                .unwrap_or_default()
                .to_ascii_lowercase();
            let href = attribute(tag, "href");
            if rel
                .split_ascii_whitespace()
                .any(|word| word == "stylesheet")
                && let Some(href) = href
                && let Some(url) = resolve(base, &href)
            {
                found.push(SheetRef::Link {
                    url,
                    media: attribute(tag, "media"),
                });
            }
        } else if name == "style" {
            let close = lower[end..]
                .find("</style")
                .map_or(lower.len(), |close| end + close);
            found.push(SheetRef::Inline {
                text: html[end + 1..close].to_owned(),
                media: attribute(tag, "media"),
            });
            at = close;
            continue;
        } else if name == "script" {
            at = lower[end..]
                .find("</script")
                .map_or(lower.len(), |close| end + close);
            continue;
        }
        at = end + 1;
    }
    found
}

fn tag_name(tag: &str) -> String {
    tag.split(|c: char| c.is_ascii_whitespace() || c == '/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The value of attribute `name` in the inside of a tag (quoted or not), entities `&amp;` decoded.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut search = 0;
    while let Some(offset) = lower[search..].find(name) {
        let start = search + offset;
        search = start + name.len();
        let before_ok = start > 0 && bytes[start - 1].is_ascii_whitespace();
        let rest = lower[search..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let value_start = tag.len() - rest.len() + 1;
        let value = tag[value_start..].trim_start();
        let raw = match value.chars().next() {
            Some(quote @ ('"' | '\'')) => value[1..].split(quote).next().unwrap_or_default(),
            _ => value
                .split(|c: char| c.is_ascii_whitespace() || c == '>')
                .next()
                .unwrap_or_default(),
        };
        return Some(raw.replace("&amp;", "&"));
    }
    None
}

/// The URLs of a stylesheet's `@import`s, resolved against the sheet's own URL.
pub fn imports(css: &str, base: &str) -> Vec<String> {
    let mut found = Vec::new();
    let lower = css.to_ascii_lowercase();
    let mut at = 0;
    while let Some(offset) = lower[at..].find("@import") {
        let start = at + offset + "@import".len();
        let end = lower[start..]
            .find(';')
            .map_or(lower.len(), |end| start + end);
        let prelude = css[start..end].trim();
        let target = prelude
            .strip_prefix("url(")
            .or_else(|| prelude.strip_prefix("URL("))
            .and_then(|rest| rest.split(')').next())
            .unwrap_or(prelude);
        let target = target.trim().trim_matches(|c| c == '"' || c == '\'');
        let target = target.split(['"', '\'']).next().unwrap_or_default();
        if let Some(url) = resolve(base, target.trim()) {
            found.push(url);
        }
        at = end;
    }
    found
}

/// `reference` resolved against `base` (an absolute `http`/`https`/`file` URL); `None` for an
/// empty reference, a `data:` or `javascript:` URL or anything not loaded over those schemes.
pub fn resolve(base: &str, reference: &str) -> Option<String> {
    let reference = reference.trim();
    if reference.is_empty() || reference.starts_with('#') {
        return None;
    }
    if let Some((scheme, _)) = reference.split_once(':')
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        let scheme = scheme.to_ascii_lowercase();
        return ["http", "https", "file"]
            .contains(&scheme.as_str())
            .then(|| reference.to_owned());
    }
    let (scheme, rest) = base.split_once("://")?;
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let (authority, path) = rest.split_at(authority_end);
    let path = path.split(['?', '#']).next().unwrap_or_default();
    if let Some(network) = reference.strip_prefix("//") {
        return Some(format!("{scheme}://{network}"));
    }
    let joined = if reference.starts_with('/') {
        reference.to_owned()
    } else {
        let directory = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        format!("{directory}/{reference}")
    };
    let (joined, suffix) = match joined.find(['?', '#']) {
        Some(split) => joined.split_at(split),
        None => (joined.as_str(), ""),
    };
    let mut segments: Vec<&str> = Vec::new();
    for segment in joined.split('/').skip(1) {
        match segment {
            "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    Some(format!(
        "{scheme}://{authority}/{}{suffix}",
        segments.join("/")
    ))
}

#[cfg(test)]
#[path = "html_tests.rs"]
mod tests;
