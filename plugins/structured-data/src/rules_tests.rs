use rb_plugin_api::check::valid_rule_id;

use super::*;

fn issue(rule: &'static str) -> Issue {
    Issue {
        rule,
        node: Some("n2".to_owned()),
        property: Some("urll".to_owned()),
        place: "jsonld/0#/urll".to_owned(),
        element: "html > head > script".to_owned(),
        params: vec![("property", "x".repeat(300))],
    }
}

#[test]
fn every_rule_is_described_once_with_a_valid_id_and_help() {
    let described = describe();
    assert_eq!(described.len(), RULES.len());
    for rule in &described {
        assert!(valid_rule_id(&rule.id), "{}", rule.id);
        assert_eq!(
            described.iter().filter(|other| other.id == rule.id).count(),
            1
        );
        assert_eq!(
            rule.title,
            format!("plugin.structured-data.rules.{}", rule.id)
        );
        assert!(
            rule.help_url
                .as_deref()
                .is_some_and(|url| url.starts_with("https://"))
        );
    }
}

#[test]
fn a_finding_names_its_element_place_node_and_cut_params() {
    let scope = Scope {
        enabled: &[],
        min: Severity::Info,
        view: Some(4),
        url: "https://example.test/",
    };
    let found = findings(&[issue(PROPERTY_UNKNOWN)], &scope);
    assert_eq!(found.len(), 1);
    let finding = &found[0];
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(
        finding.message,
        "plugin.structured-data.findings.property.unknown"
    );
    assert_eq!(finding.selector.as_deref(), Some("html > head > script"));
    assert_eq!(
        finding.path.as_deref(),
        Some("structured-data:jsonld/0#/urll")
    );
    assert_eq!(finding.view, Some(4));
    assert_eq!(finding.params[0].1.chars().count(), MAX_PARAM_CHARS);
    let data: serde_json::Value =
        serde_json::from_str(finding.data.as_deref().unwrap_or("null")).expect("data");
    assert_eq!(data["node"], "n2");
    assert_eq!(data["panel"], "structured-data");
}

#[test]
fn the_rules_switched_off_and_the_milder_severities_are_left_out() {
    let enabled = vec![PROPERTY_UNKNOWN.to_owned(), PROPERTY_SUPERSEDED.to_owned()];
    let scope = Scope {
        enabled: &enabled,
        min: Severity::Info,
        view: None,
        url: "u",
    };
    let issues = [
        issue(PROPERTY_UNKNOWN),
        issue(PROPERTY_SUPERSEDED),
        issue(PROPERTY_RANGE),
    ];
    assert_eq!(findings(&issues, &scope).len(), 2);
    let stricter = Scope {
        min: Severity::Warning,
        ..scope
    };
    let kept: Vec<String> = findings(&issues, &stricter)
        .into_iter()
        .map(|finding| finding.rule)
        .collect();
    assert_eq!(kept, [PROPERTY_UNKNOWN]);
}
