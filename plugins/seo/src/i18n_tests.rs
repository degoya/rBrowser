use super::*;
use crate::rules::{RULES, describe};

fn catalogue() -> Value {
    serde_json::from_str(EN).expect("the catalogue is JSON")
}

#[test]
fn texts_fill_their_placeholders() {
    let params = vec![("status".to_owned(), "404".to_owned())];
    assert_eq!(
        text("plugin.seo.findings.index.status", &params),
        "The page answers 404, not 200."
    );
    assert_eq!(text("plugin.seo.nope", &[]), "plugin.seo.nope");
}

/// Every message key a rule emits (`src/rules/**`), below `plugin.seo.findings.`.
const MESSAGES: &[&str] = &[
    "index.status",
    "index.fetch_failed",
    "index.redirect",
    "index.redirect_loop",
    "index.redirect_location",
    "index.noindex_meta",
    "index.noindex_header",
    "index.robots_blocked",
    "canonical.missing",
    "canonical.multiple",
    "canonical.relative",
    "canonical.not_self",
    "canonical.target_invalid",
    "canonical.target_failed",
    "canonical.target_redirect",
    "canonical.target_status",
    "canonical.target_noindex",
    "meta.title_missing",
    "meta.title_empty",
    "meta.title_short",
    "meta.title_long",
    "meta.title_multiple",
    "meta.title_duplicate",
    "meta.description_missing",
    "meta.description_empty",
    "meta.description_short",
    "meta.description_long",
    "meta.description_multiple",
    "meta.description_duplicate",
    "meta.lang_missing",
    "meta.viewport_missing",
    "meta.charset_missing",
    "headings.h1_missing",
    "headings.h1_multiple",
    "headings.skipped",
    "social.og_missing",
    "structured.jsonld_invalid",
    "site.sitemap_missing",
    "site.robots_missing",
    "site.sitemap_unreachable",
    "render.js_only",
    "render.removed",
    "render.changed",
    "images.alt_missing",
    "images.alt_empty",
    "images.linked_no_name",
    "images.alt_filename",
    "images.alt_placeholder",
    "images.alt_prefix",
    "images.alt_too_long",
    "images.alt_duplicate",
    "images.alt_redundant",
    "images.alt_language",
    "images.input_alt_missing",
    "images.area_alt_missing",
    "images.svg_name_missing",
    "images.social_alt_missing",
    "images.background",
    "images.alt_untranslated",
    "hreflang.none",
    "hreflang.source_conflict",
    "hreflang.source_missing",
    "hreflang.code_invalid",
    "hreflang.code_suggestion",
    "hreflang.target_relative",
    "hreflang.duplicate_code",
    "hreflang.duplicate_url",
    "hreflang.self_missing",
    "hreflang.x_default_missing",
    "hreflang.locale_missing",
    "hreflang.target_status",
    "hreflang.target_redirect",
    "hreflang.target_failed",
    "hreflang.target_noindex",
    "hreflang.target_robots",
    "hreflang.target_canonical",
    "hreflang.target_canonical_language",
    "hreflang.unchecked_robots",
    "hreflang.unchecked_local",
    "hreflang.unchecked_limit",
    "hreflang.unchecked_set",
    "hreflang.lang_mismatch",
    "hreflang.content_language_mismatch",
    "hreflang.content_detected",
    "hreflang.return_missing",
    "hreflang.set_incomplete",
    "parity.differs",
    "parity.text",
    "social.image_relative",
    "social.image_unreachable",
    "social.image_type",
    "social.image_svg",
    "social.image_small",
    "social.image_ratio",
    "social.twitter_card_missing",
    "social.twitter_card_invalid",
    "social.url_canonical",
    "wellknown.llms_missing",
    "wellknown.llms_no_h1",
    "wellknown.llms_h1_not_first",
    "wellknown.llms_h1_multiple",
    "wellknown.llms_no_summary",
    "wellknown.llms_bad_item",
    "wellknown.llms_empty_section",
    "wellknown.llms_link",
    "wellknown.llms_link_invalid",
    "wellknown.security_missing",
    "wellknown.security_location",
    "wellknown.security_http",
    "wellknown.security_type",
    "wellknown.security_contact",
    "wellknown.security_expires_missing",
    "wellknown.security_expires_multiple",
    "wellknown.security_expires_invalid",
    "wellknown.security_expired",
    "wellknown.security_expires_far",
    "microdata.type_missing",
    "microdata.type_unknown",
    "microdata.required_missing",
    "microdata.required_any",
    "microdata.jsonld_conflict",
    "head.charset_late",
    "head.charset_not_first",
    "head.viewport_late",
    "head.title_late",
    "head.preconnect_late",
    "head.script_before_css",
    "head.in_body",
    "head.in_body_after",
    "head.rendered_differs",
];

#[test]
fn every_rule_has_a_title_and_every_message_a_text() {
    let catalogue = catalogue();
    for rule in describe() {
        assert!(lookup(&catalogue, &rule.title).is_some(), "{}", rule.title);
    }
    for message in MESSAGES {
        let key = format!("plugin.seo.findings.{message}");
        assert!(lookup(&catalogue, &key).is_some(), "{key}");
    }
    assert!(RULES.len() <= MESSAGES.len());
}

#[test]
fn every_text_is_below_the_plugins_prefix() {
    let catalogue = catalogue();
    let keys: Vec<&String> = catalogue.as_object().expect("object").keys().collect();
    assert_eq!(keys, ["plugin"]);
    let plugins: Vec<&String> = catalogue["plugin"]
        .as_object()
        .expect("object")
        .keys()
        .collect();
    assert_eq!(plugins, ["seo"]);
}
