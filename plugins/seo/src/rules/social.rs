//! `social.og_missing`: the Open Graph basics (`og:title`, `og:description`, `og:image`,
//! `og:url`); `structured.jsonld_invalid`: every JSON-LD block is JSON; and, of the alt text
//! group, `images.social_alt_missing`: a share image has its alt text (`og:image:alt`,
//! `twitter:image:alt`). RB-080-11: `social.image_relative` (`og:image` written relative),
//! `social.twitter_card` (missing or no card type X knows) and `social.url_canonical` (`og:url`
//! names another page than the canonical); the share image itself is `rules::social_image`.

use crate::audit::Page;
use crate::page::Facts;
use crate::rules::{Element, Sink, short};
use crate::url::Url;

/// The card types X knows.
pub const TWITTER_CARDS: [&str; 4] = ["summary", "summary_large_image", "app", "player"];

fn value<'a>(facts: &'a Facts, key: &str) -> Option<&'a str> {
    facts
        .social
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

/// `og:image` as an absolute `http(s)` URL; the platforms resolve no relative one.
fn image_relative(facts: &Facts, sink: &mut Sink<'_>) {
    if let Some(image) = value(facts, "og:image")
        && Url::parse(image).is_none()
    {
        sink.emit(
            "social.image_relative",
            "social.image_relative",
            None,
            &[("href", short(image, 200))],
        );
    }
}

fn twitter_card(facts: &Facts, sink: &mut Sink<'_>) {
    match value(facts, "twitter:card") {
        None => sink.emit(
            "social.twitter_card",
            "social.twitter_card_missing",
            None,
            &[],
        ),
        Some(card) if !TWITTER_CARDS.contains(&card.to_ascii_lowercase().as_str()) => sink.emit(
            "social.twitter_card",
            "social.twitter_card_invalid",
            None,
            &[("value", short(card, 100))],
        ),
        Some(_) => {}
    }
}

/// `og:url` and the first canonical, both resolved against the page, name the same URL.
fn url_canonical(page: &Page<'_>, facts: &Facts, sink: &mut Sink<'_>) {
    let (Some(og_url), Some(canonical)) = (value(facts, "og:url"), facts.canonicals.first()) else {
        return;
    };
    let (Some(og), Some(target)) = (page.url.join(og_url), page.url.join(canonical)) else {
        return;
    };
    if og != target {
        sink.emit(
            "social.url_canonical",
            "social.url_canonical",
            None,
            &[
                ("og_url", short(&og.to_string(), 200)),
                ("canonical", short(&target.to_string(), 200)),
            ],
        );
    }
}

/// The Open Graph properties every page should carry.
pub const OG_BASICS: [&str; 4] = ["og:title", "og:description", "og:image", "og:url"];

pub fn check(page: &Page<'_>, sink: &mut Sink<'_>) {
    let Some((_, facts)) = page.primary() else {
        return;
    };
    for property in OG_BASICS {
        if facts
            .social
            .get(property)
            .is_none_or(|value| value.is_empty())
        {
            sink.emit(
                "social.og_missing",
                "social.og_missing",
                None,
                &[("property", property.to_owned())],
            );
        }
    }
    image_relative(facts, sink);
    twitter_card(facts, sink);
    url_canonical(page, facts, sink);
    for (image, alt) in [
        ("og:image", "og:image:alt"),
        ("twitter:image", "twitter:image:alt"),
    ] {
        let has_image = facts
            .social
            .get(image)
            .is_some_and(|value| !value.is_empty());
        let has_alt = facts.social.get(alt).is_some_and(|value| !value.is_empty());
        if has_image && !has_alt {
            sink.emit(
                "images.social_alt_missing",
                "images.social_alt_missing",
                None,
                &[("property", alt.to_owned())],
            );
        }
    }
    for block in &facts.json_ld {
        if let Err(error) = serde_json::from_str::<serde_json::Value>(&block.text) {
            sink.emit(
                "structured.jsonld_invalid",
                "structured.jsonld_invalid",
                Some(Element::at(&block.path)),
                &[
                    ("line", error.line().to_string()),
                    ("column", error.column().to_string()),
                    ("error", short(&error.to_string(), 200)),
                ],
            );
        }
    }
}

#[cfg(test)]
#[path = "social_tests.rs"]
mod tests;
