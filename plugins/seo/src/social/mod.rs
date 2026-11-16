//! Social previews (RB-080-11): what Facebook, X, LinkedIn, Slack, WhatsApp, Discord and the
//! other platforms of `data/social.json` (versioned) show for a shared link, built from the
//! page's `og:*` and `twitter:*` tags with each platform's fallback chain and cut at its lengths;
//! which card each shows follows its rule (`variant`, RB-080-99). The previews come from
//! the raw HTML: the platforms' scrapers run no JavaScript. The share image is fetched through
//! the host (`share`), never by the panel, whose frame has no network.

use serde::{Deserialize, Serialize};

use crate::page::Facts;
use crate::url::Url;

pub mod base64;
pub mod image;
pub mod share;
pub mod tool;
pub mod variant;

pub use variant::{Layout, Reason, Rule, Shape};

/// The platform table, compiled in.
const DATA: &str = include_str!("../../data/social.json");
/// The mark of a cut text.
const ELLIPSIS: char = '\u{2026}';

/// Which tags a platform reads first.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Prefer {
    Og,
    Twitter,
}

/// How sure a platform's rule is.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// The platform's documentation.
    Official,
    /// The platform's own client.
    Code,
    /// No published rule.
    Unverified,
}

/// One platform of `data/social.json`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Platform {
    pub id: String,
    pub name: String,
    pub prefer: Prefer,
    /// In the default selection of `seo.social.platforms`.
    pub default: bool,
    /// The most characters of the title the card shows; `None`: no published cut.
    pub title: Option<usize>,
    /// The most characters of the description; `None`: no published cut.
    pub description: Option<usize>,
    /// The card shows no description.
    #[serde(default)]
    pub no_description: bool,
    /// How the platform picks its card.
    pub rule: Rule,
    pub confidence: Confidence,
    /// Where the rule is written (documentation or the client's code).
    pub source: Option<String>,
}

/// `data/social.json`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub version: u32,
    pub platforms: Vec<Platform>,
}

/// The platform table.
#[must_use]
pub fn data() -> Data {
    serde_json::from_str(DATA).expect("data/social.json is the plugin's own and parses")
}

/// The ids of every platform, in the table's order.
#[must_use]
pub fn platform_ids() -> Vec<String> {
    data().platforms.into_iter().map(|p| p.id).collect()
}

/// The ids of the platforms the preview shows by default.
#[must_use]
pub fn default_ids() -> Vec<String> {
    data()
        .platforms
        .into_iter()
        .filter(|p| p.default)
        .map(|p| p.id)
        .collect()
}

/// `text` cut to `max` characters (`None`: never): the first `max - 1`, trailing white space
/// dropped, and `...` as one character; whether it was cut.
#[must_use]
pub fn cut(text: &str, max: Option<usize>) -> (String, bool) {
    let Some(max) = max else {
        return (text.to_owned(), false);
    };
    if text.chars().count() <= max {
        return (text.to_owned(), false);
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.truncate(out.trim_end().len());
    out.push(ELLIPSIS);
    (out, true)
}

/// A text of a card: what the card shows, the whole text, and the tag it came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardText {
    pub text: String,
    pub full: String,
    pub truncated: bool,
    pub source: String,
}

/// One platform's preview.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    pub name: String,
    /// The card the platform shows (`large`, `small`, `text`).
    pub layout: Layout,
    /// The other card to look at; `None` when the platform has no other.
    pub alternate: Option<Layout>,
    /// No sure rule (none published, or the image's size unknown): both are shown.
    pub unsure: bool,
    /// Why the card has its layout.
    pub reason: Reason,
    pub confidence: Confidence,
    pub source: Option<String>,
    pub title: Option<CardText>,
    pub description: Option<CardText>,
    /// The share image's absolute URL; `None` without one.
    pub image: Option<String>,
    pub image_source: Option<String>,
    /// The platform's rule, to settle the layout once the image's size is known.
    #[serde(skip)]
    pub rule: Rule,
    /// The page's `twitter:card`.
    #[serde(skip)]
    pub twitter_card: Option<String>,
}

impl Card {
    /// The layout for a share image of `shape` (`None`: its size is not known).
    pub fn settle(&mut self, shape: Option<Shape>) {
        let chosen = variant::choose(
            &self.rule,
            self.twitter_card.as_deref(),
            self.image.is_some(),
            shape,
        );
        self.layout = chosen.layout;
        self.alternate = chosen.alternate;
        self.unsure = chosen.unsure;
        self.reason = chosen.reason;
    }
}

/// The first non-empty value of `keys` in the page's social tags, then `fallback` as `name`.
fn pick(
    facts: &Facts,
    keys: &[&str],
    fallback: Option<(&str, &String)>,
) -> Option<(String, String)> {
    keys.iter()
        .find_map(|key| {
            facts
                .social
                .get(*key)
                .filter(|value| !value.trim().is_empty())
                .map(|value| ((*key).to_owned(), value.clone()))
        })
        .or_else(|| {
            fallback
                .filter(|(_, value)| !value.trim().is_empty())
                .map(|(name, value)| (name.to_owned(), value.clone()))
        })
}

fn card_text(found: Option<(String, String)>, max: Option<usize>) -> Option<CardText> {
    let (source, full) = found?;
    let (text, truncated) = cut(&full, max);
    Some(CardText {
        text,
        full,
        truncated,
        source,
    })
}

/// The domain a card names: `og:url`'s host, else the page's.
#[must_use]
pub fn domain(facts: &Facts, page: &Url) -> String {
    facts
        .social
        .get("og:url")
        .and_then(|value| page.join(value))
        .map_or_else(|| page.host().to_owned(), |url| url.host().to_owned())
}

/// The cards of the `enabled` platforms (every one when empty) for a page with `facts` at
/// `page`; a size rule stays unsure until [`Card::settle`] gets the image's size.
#[must_use]
pub fn cards(facts: &Facts, page: &Url, enabled: &[String]) -> Vec<Card> {
    data()
        .platforms
        .into_iter()
        .filter(|platform| enabled.is_empty() || enabled.contains(&platform.id))
        .map(|platform| card(facts, page, &platform))
        .collect()
}

fn card(facts: &Facts, page: &Url, platform: &Platform) -> Card {
    let (title_keys, description_keys, image_keys): (&[&str], &[&str], &[&str]) =
        match platform.prefer {
            Prefer::Og => (
                &["og:title", "twitter:title"],
                &["og:description", "twitter:description"],
                &["og:image", "twitter:image"],
            ),
            Prefer::Twitter => (
                &["twitter:title", "og:title"],
                &["twitter:description", "og:description"],
                &["twitter:image", "og:image"],
            ),
        };
    let title = pick(
        facts,
        title_keys,
        facts.titles.first().map(|t| ("title", t)),
    );
    let description = if platform.no_description {
        None
    } else {
        let fallback = facts.descriptions.first().map(|d| ("description", d));
        card_text(
            pick(facts, description_keys, fallback),
            platform.description,
        )
    };
    let image = pick(facts, image_keys, None)
        .and_then(|(source, value)| page.join(value.trim()).map(|url| (source, url.to_string())));
    let twitter_card = facts.social.get("twitter:card").cloned();
    let chosen = variant::choose(
        &platform.rule,
        twitter_card.as_deref(),
        image.is_some(),
        None,
    );
    Card {
        id: platform.id.clone(),
        name: platform.name.clone(),
        layout: chosen.layout,
        alternate: chosen.alternate,
        unsure: chosen.unsure,
        reason: chosen.reason,
        confidence: platform.confidence,
        source: platform.source.clone(),
        title: card_text(title, platform.title),
        description,
        image_source: image.as_ref().map(|(source, _)| source.clone()),
        image: image.map(|(_, url)| url),
        rule: platform.rule.clone(),
        twitter_card,
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
