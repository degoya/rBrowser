use std::collections::BTreeSet;

use rb_plugin_api::Severity;
use rb_plugin_api::check::{MAX_RULES, valid_rule_id};
use serde_json::Value;

use super::*;

fn locale() -> Value {
    let text = include_str!("../locales/en.json");
    serde_json::from_str(text).expect("the locale reads")
}

#[test]
fn the_table_reads_and_fits_the_contract() {
    let rules = describe();
    assert!(rules.len() > 100, "{} rules", rules.len());
    assert!(rules.len() <= MAX_RULES);
    assert!(rules.iter().all(|rule| valid_rule_id(&rule.id)));
    let ids: BTreeSet<&str> = rules.iter().map(|rule| rule.id.as_str()).collect();
    assert_eq!(ids.len(), rules.len(), "every audit once");
    assert!(lighthouse_version().starts_with("13."));
}

#[test]
fn every_rule_has_a_title_in_the_locale() {
    let locale = locale();
    for rule in describe() {
        let id = rule.id.as_str();
        let title = &locale["plugin"]["lighthouse"]["rules"][id];
        assert!(title.is_string(), "no title for {id}");
        assert_eq!(rule.title, format!("plugin.lighthouse.rules.{id}"));
    }
}

#[test]
fn every_audit_belongs_to_a_known_category() {
    assert!(all().iter().all(
        |audit| !audit.categories.is_empty() && audit.categories.iter().all(|c| is_category(c))
    ));
    let lcp = find("largest-contentful-paint").expect("LCP is an audit");
    assert_eq!(lcp.categories, ["performance"]);
    assert!(lcp.weight > 0.0);
}

#[test]
fn weighted_audits_are_errors_the_others_warnings() {
    assert_eq!(usual_severity(10.0), Severity::Error);
    assert_eq!(usual_severity(0.0), Severity::Warning);
    let image_alt = describe()
        .into_iter()
        .find(|rule| rule.id == "image-alt")
        .expect("image-alt is a rule");
    assert_eq!(image_alt.severity, Severity::Error);
    assert_eq!(image_alt.shared_id, None);
}
