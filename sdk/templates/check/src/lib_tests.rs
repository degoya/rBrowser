use super::{LANG_MISSING, RULES, TITLE_MISSING, page_findings};

fn all_rules() -> Vec<String> {
    RULES.iter().map(|rule| rule.id.to_owned()).collect()
}

#[test]
fn a_page_without_title_and_language_has_two_findings() {
    let findings = page_findings(&all_rules(), "\"\"", "\"\"");
    let rules: Vec<&str> = findings.iter().map(|finding| finding.rule).collect();
    assert_eq!(rules, vec![TITLE_MISSING, LANG_MISSING]);
    assert!(page_findings(&all_rules(), "\"  \"", "\"de\"").len() == 1);
}

#[test]
fn a_complete_page_has_none() {
    assert!(page_findings(&all_rules(), "\"Home\"", "\"en\"").is_empty());
}

#[test]
fn only_the_rules_switched_on_run() {
    let findings = page_findings(&[LANG_MISSING.to_owned()], "\"\"", "\"\"");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, LANG_MISSING);
}

#[test]
fn every_rule_and_message_speaks_in_the_plugins_keys() {
    for rule in RULES {
        assert!(rule.title.starts_with("plugin.example-check."));
    }
    for finding in page_findings(&all_rules(), "\"\"", "\"\"") {
        assert!(
            finding
                .message
                .starts_with("plugin.example-check.findings.")
        );
    }
}
