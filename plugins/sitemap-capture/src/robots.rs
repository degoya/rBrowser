//! `robots.txt` (RFC 9309): its `Sitemap:` lines, and the `Allow`/`Disallow` rules of the group
//! for rBrowser's token (`rbrowser`) or, without one, of `*` - the longest matching pattern
//! wins, `Allow` on a tie, `*` and a final `$` as wildcards. Read as text; a line longer than
//! [`MAX_LINE`] and every line after [`MAX_LINES`] is ignored.

use crate::url::Url;

/// The product token rBrowser's capture answers to.
pub const TOKEN: &str = "rbrowser";
/// The most lines read.
pub const MAX_LINES: usize = 10_000;
/// The longest line read.
pub const MAX_LINE: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Rule {
    allow: bool,
    pattern: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Group {
    agents: Vec<String>,
    rules: Vec<Rule>,
}

/// A site's `robots.txt`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Robots {
    /// The `Sitemap:` lines, in order.
    pub sitemaps: Vec<Url>,
    rules: Vec<Rule>,
}

/// Whether `pattern` (with `*` and a final `$`) matches the start of `path`.
fn matches(pattern: &str, path: &str) -> bool {
    let (pattern, anchored) = match pattern.strip_suffix('$') {
        Some(rest) => (rest, true),
        None => (pattern, false),
    };
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut at = 0;
    for (index, part) in parts.iter().enumerate() {
        if index == 0 {
            if !path.starts_with(part) {
                return false;
            }
            at = part.len();
            continue;
        }
        let last = index == parts.len() - 1;
        if last && anchored {
            return path.len() >= at + part.len() && path.ends_with(part);
        }
        match path[at..].find(part) {
            Some(found) => at += found + part.len(),
            None => return false,
        }
    }
    !anchored || at == path.len()
}

impl Robots {
    /// `text` read; a relative `Sitemap:` line is resolved against `base` (the robots URL).
    #[must_use]
    pub fn parse(text: &str, base: &Url) -> Self {
        let mut sitemaps = Vec::new();
        let mut groups: Vec<Group> = Vec::new();
        let mut open = false;
        for line in text.lines().take(MAX_LINES) {
            if line.len() > MAX_LINE {
                continue;
            }
            let line = line.split('#').next().unwrap_or_default().trim();
            let Some((field, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match field.trim().to_ascii_lowercase().as_str() {
                "sitemap" => sitemaps.extend(base.join(value)),
                "user-agent" => {
                    if !open {
                        groups.push(Group::default());
                        open = true;
                    }
                    if let Some(group) = groups.last_mut() {
                        group.agents.push(value.to_ascii_lowercase());
                    }
                }
                field @ ("allow" | "disallow") => {
                    open = false;
                    if let Some(group) = groups.last_mut()
                        && !value.is_empty()
                    {
                        group.rules.push(Rule {
                            allow: field == "allow",
                            pattern: value.to_owned(),
                        });
                    }
                }
                _ => {}
            }
        }
        let ours = |group: &&Group| group.agents.iter().any(|agent| agent == TOKEN);
        let any = |group: &&Group| group.agents.iter().any(|agent| agent == "*");
        let chosen: Vec<&Group> = if groups.iter().any(|group| ours(&group)) {
            groups.iter().filter(ours).collect()
        } else {
            groups.iter().filter(any).collect()
        };
        Self {
            sitemaps,
            rules: chosen
                .into_iter()
                .flat_map(|group| group.rules.iter().cloned())
                .collect(),
        }
    }

    /// Whether the rules let rBrowser load `url`.
    #[must_use]
    pub fn allows(&self, url: &Url) -> bool {
        let path = url.path_and_query();
        self.rules
            .iter()
            .filter(|rule| matches(&rule.pattern, &path))
            .max_by_key(|rule| (rule.pattern.len(), rule.allow))
            .is_none_or(|rule| rule.allow)
    }
}

#[cfg(test)]
#[path = "robots_tests.rs"]
mod tests;
