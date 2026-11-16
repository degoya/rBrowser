use crate::ProjectConfig;

use super::*;

#[test]
fn the_section_reads_its_keys_and_defaults_the_rest() {
    let config: ProjectConfig = serde_json::from_str(
        r#"{ "version": 1, "sitemap": { "parallel": 2, "fullPage": false,
             "include": ["/blog/**"], "hostIntervalMs": 1000 } }"#,
    )
    .expect("parses");
    assert_eq!(config.sitemap.parallel, 2);
    assert!(!config.sitemap.full_page);
    assert_eq!(config.sitemap.include, ["/blog/**"]);
    assert_eq!(config.sitemap.host_interval_ms, 1000);
    assert!(config.sitemap.respect_robots, "defaults to true");
    assert!(
        config.sitemap.hide_selectors.is_empty(),
        "the built-in list of capture.useBuiltinList holds the old defaults (RB-080-12)"
    );
}

#[test]
fn a_default_section_is_not_written() {
    let config: ProjectConfig = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(config.sitemap, SitemapConfig::default());
    let written = serde_json::to_value(&config).expect("serializes");
    assert!(written.get("sitemap").is_none(), "{written}");
}

#[test]
fn an_unknown_type_is_refused() {
    let bad = r#"{ "version": 1, "sitemap": { "parallel": "four" } }"#;
    assert!(serde_json::from_str::<ProjectConfig>(bad).is_err());
}
