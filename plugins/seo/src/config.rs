//! The settings section `seo` (`settings.schema.json`, Settings > Checks > SEO) as the check
//! reads it: every key through `settings.get` with its property path (`title.maxLength`), the
//! schema's default when the host answers none or a value out of range; the arguments of a run
//! (`maxPages`) win over the settings.

use std::collections::BTreeMap;

use rb_plugin_api::{PluginHost, Severity};
use serde::Deserialize;
use serde_json::Value;

/// Pages a crawl loads by default.
pub const DEFAULT_MAX_PAGES: usize = 10;
/// The most pages a crawl loads, whatever the settings or arguments say.
pub const MAX_PAGES: usize = 100;
/// The most ignore patterns and rule overrides read.
const MAX_ENTRIES: usize = 256;
/// The most placeholder words read.
const MAX_PLACEHOLDERS: usize = 128;
/// The most expected locales read.
const MAX_LOCALES: usize = 64;
/// The longest expected locale read.
const MAX_LOCALE_CHARS: usize = 35;
/// `seo.parity.reference`'s default: the narrowest view.
pub const SMALLEST: &str = "smallest";
/// The rule groups of RB-080-11 a setting `seo.<group>.enabled` switches off.
pub const SWITCHED_GROUPS: [&str; 4] = ["social", "wellknown", "microdata", "head"];
/// The most links of `llms.txt` checked, whatever the settings say.
pub const MAX_LLMS_LINKS: usize = 50;

/// What `seo.rules.<rule>` sets: the rule off, or the severity of its findings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    Off,
    Severity(Severity),
}

/// One entry of `seo.ignore`: every field it names has to match (`*` any text, `?` one
/// character).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct IgnorePattern {
    pub rule: Option<String>,
    pub url: Option<String>,
    pub selector: Option<String>,
}

/// The effective settings of one run.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub max_pages: usize,
    pub title_min: usize,
    pub title_max: usize,
    pub description_min: usize,
    pub description_max: usize,
    pub max_alt_length: usize,
    /// Lower-case words that alone are no alt text (`image`, `logo`).
    pub placeholders: Vec<String>,
    /// An image at most this many CSS pixels wide and high may be decorative with `alt=""`.
    pub decorative_max_size: u32,
    /// By rule id (`images.alt_missing`) or group (`images`).
    pub rules: BTreeMap<String, Level>,
    pub ignore: Vec<IgnorePattern>,
    /// `seo.locales`: the codes the language set has to have (`de`, `en-GB`).
    pub locales: Vec<String>,
    /// `seo.requireXDefault`: a set without `x-default` is a finding.
    pub require_x_default: bool,
    /// `seo.hreflang.checkContentLanguage`: the text of each page held to its code's language.
    pub check_content_language: bool,
    /// `seo.parity.reference`: [`SMALLEST`], or a view's breakpoint or label.
    pub parity_reference: String,
    /// `seo.parity.textSimilarity`: the least similarity (0 to 1) of a view's main text to the
    /// reference view's.
    pub text_similarity: f64,
    /// The groups of [`SWITCHED_GROUPS`] whose `seo.<group>.enabled` is `false`.
    pub groups_off: Vec<String>,
    /// `seo.social.platforms`: the platforms the preview shows (every one when empty).
    pub social_platforms: Vec<String>,
    /// `seo.social.imageMinWidth` and `imageMinHeight`: a smaller share image is a finding.
    pub image_min_width: u32,
    pub image_min_height: u32,
    /// `seo.wellknown.maxLinks`: the links of `llms.txt` fetched.
    pub llms_max_links: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_pages: DEFAULT_MAX_PAGES,
            title_min: 30,
            title_max: 60,
            description_min: 70,
            description_max: 160,
            max_alt_length: 125,
            placeholders: [
                "image",
                "img",
                "picture",
                "pic",
                "photo",
                "foto",
                "bild",
                "graphic",
                "grafik",
                "logo",
                "icon",
                "banner",
                "placeholder",
                "untitled",
            ]
            .map(str::to_owned)
            .to_vec(),
            decorative_max_size: 32,
            rules: BTreeMap::new(),
            ignore: Vec::new(),
            locales: Vec::new(),
            require_x_default: true,
            check_content_language: false,
            parity_reference: SMALLEST.to_owned(),
            text_similarity: 0.9,
            groups_off: Vec::new(),
            social_platforms: crate::social::default_ids(),
            image_min_width: 1200,
            image_min_height: 630,
            llms_max_links: 20,
        }
    }
}

fn setting(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn number(host: &mut dyn PluginHost, key: &str, min: u64, max: u64, default: usize) -> usize {
    setting(host, key)
        .and_then(|value| value.as_u64())
        .filter(|value| (min..=max).contains(value))
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(default)
}

fn boolean(host: &mut dyn PluginHost, key: &str, default: bool) -> bool {
    setting(host, key)
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

fn dimension(host: &mut dyn PluginHost, key: &str, default: u32) -> u32 {
    u32::try_from(number(host, key, 1, 10_000, default as usize)).unwrap_or(default)
}

fn level(value: &Value) -> Option<Level> {
    match value.as_str()? {
        "off" => Some(Level::Off),
        name => Severity::parse(name).map(Level::Severity),
    }
}

impl Config {
    /// The settings as the host answers them; `args` (`maxPages`) win.
    #[must_use]
    pub fn read(host: &mut dyn PluginHost, args: &Value) -> Self {
        let default = Self::default();
        let mut config = Self {
            max_pages: number(host, "maxPages", 1, MAX_PAGES as u64, default.max_pages),
            title_min: number(host, "title.minLength", 0, 1000, default.title_min),
            title_max: number(host, "title.maxLength", 1, 1000, default.title_max),
            description_min: number(
                host,
                "description.minLength",
                0,
                1000,
                default.description_min,
            ),
            description_max: number(
                host,
                "description.maxLength",
                1,
                1000,
                default.description_max,
            ),
            max_alt_length: number(host, "images.maxAltLength", 1, 1000, default.max_alt_length),
            decorative_max_size: u32::try_from(number(
                host,
                "images.decorativeMaxSize",
                0,
                1000,
                default.decorative_max_size as usize,
            ))
            .unwrap_or(default.decorative_max_size),
            placeholders: setting(host, "images.placeholders")
                .and_then(|value| {
                    value.as_array().map(|words| {
                        words
                            .iter()
                            .filter_map(Value::as_str)
                            .map(|word| word.trim().to_lowercase())
                            .filter(|word| !word.is_empty())
                            .take(MAX_PLACEHOLDERS)
                            .collect()
                    })
                })
                .unwrap_or(default.placeholders),
            rules: setting(host, "rules")
                .and_then(|value| {
                    value.as_object().map(|map| {
                        map.iter()
                            .take(MAX_ENTRIES)
                            .filter_map(|(rule, value)| Some((rule.clone(), level(value)?)))
                            .collect()
                    })
                })
                .unwrap_or_default(),
            ignore: setting(host, "ignore")
                .and_then(|value| serde_json::from_value::<Vec<IgnorePattern>>(value).ok())
                .map(|mut patterns| {
                    patterns.truncate(MAX_ENTRIES);
                    patterns
                })
                .unwrap_or_default(),
            locales: setting(host, "locales")
                .and_then(|value| {
                    value.as_array().map(|codes| {
                        codes
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::trim)
                            .filter(|code| !code.is_empty() && code.len() <= MAX_LOCALE_CHARS)
                            .map(str::to_owned)
                            .take(MAX_LOCALES)
                            .collect()
                    })
                })
                .unwrap_or_default(),
            require_x_default: boolean(host, "requireXDefault", default.require_x_default),
            check_content_language: boolean(
                host,
                "hreflang.checkContentLanguage",
                default.check_content_language,
            ),
            parity_reference: setting(host, "parity.reference")
                .and_then(|value| value.as_str().map(|text| text.trim().to_owned()))
                .filter(|text| !text.is_empty() && text.len() <= 64)
                .unwrap_or(default.parity_reference),
            text_similarity: setting(host, "parity.textSimilarity")
                .and_then(|value| value.as_f64())
                .filter(|value| (0.0..=1.0).contains(value))
                .unwrap_or(default.text_similarity),
            groups_off: SWITCHED_GROUPS
                .into_iter()
                .filter(|group| !boolean(host, &format!("{group}.enabled"), true))
                .map(str::to_owned)
                .collect(),
            social_platforms: setting(host, "social.platforms")
                .and_then(|value| {
                    let known = crate::social::platform_ids();
                    value.as_array().map(|ids| {
                        ids.iter()
                            .filter_map(Value::as_str)
                            .filter(|id| known.iter().any(|known| known == id))
                            .map(str::to_owned)
                            .collect()
                    })
                })
                .unwrap_or(default.social_platforms),
            image_min_width: dimension(host, "social.imageMinWidth", default.image_min_width),
            image_min_height: dimension(host, "social.imageMinHeight", default.image_min_height),
            llms_max_links: number(
                host,
                "wellknown.maxLinks",
                0,
                MAX_LLMS_LINKS as u64,
                default.llms_max_links,
            ),
        };
        if let Some(pages) = args
            .get("maxPages")
            .and_then(Value::as_u64)
            .and_then(|pages| usize::try_from(pages).ok())
        {
            config.max_pages = pages.clamp(1, MAX_PAGES);
        }
        config
    }

    /// The level of `rule`: off when its group is switched off (`seo.social.enabled`), else its
    /// own entry, else its group's (`images`), else none.
    #[must_use]
    pub fn level(&self, rule: &str) -> Option<Level> {
        if rule
            .split_once('.')
            .is_some_and(|(group, _)| self.groups_off.iter().any(|off| off == group))
        {
            return Some(Level::Off);
        }
        self.rules.get(rule).copied().or_else(|| {
            rule.split_once('.')
                .and_then(|(group, _)| self.rules.get(group).copied())
        })
    }

    /// Whether a pattern of `seo.ignore` drops the finding.
    #[must_use]
    pub fn ignores(&self, rule: &str, url: Option<&str>, selector: Option<&str>) -> bool {
        let field = |pattern: &Option<String>, value: Option<&str>| match pattern {
            None => true,
            Some(pattern) => value.is_some_and(|value| glob(pattern, value)),
        };
        self.ignore.iter().any(|pattern| {
            (pattern.rule.is_some() || pattern.url.is_some() || pattern.selector.is_some())
                && field(&pattern.rule, Some(rule))
                && field(&pattern.url, url)
                && field(&pattern.selector, selector)
        })
    }
}

/// Whether `text` matches `pattern` as a whole: `*` any text, `?` one character.
#[must_use]
pub fn glob(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((at, from)) => {
                    p = at + 1;
                    t = from + 1;
                    star = Some((at, from + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
