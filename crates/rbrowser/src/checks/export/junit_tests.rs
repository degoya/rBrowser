use rb_plugin_types::{CheckFailure, CheckSkip, SKIP_ENGINE};

use super::super::testing::{MARKUP, fixture, with_context};
use super::*;

/// Whether `xml` is well-formed as far as this export writes XML: one declaration, every element
/// closed in order, attributes quoted, no raw `<` or bare `&` in text or values, no character
/// XML 1.0 forbids.
fn well_formed(xml: &str) -> Result<(), String> {
    if xml
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
    {
        return Err("a control character".into());
    }
    let body = xml
        .strip_prefix("<?xml version=\"1.0\" encoding=\"UTF-8\"?>")
        .ok_or("no declaration")?;
    let mut open: Vec<String> = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find('<') {
        check_text(&rest[..start])?;
        let end = rest[start..].find('>').ok_or("an unclosed tag")? + start;
        let tag = &rest[start + 1..end];
        if let Some(name) = tag.strip_prefix('/') {
            if open.pop().as_deref() != Some(name) {
                return Err(format!("</{name}> closes nothing"));
            }
        } else {
            let empty = tag.ends_with('/');
            let tag = tag.trim_end_matches('/');
            let name = tag.split_whitespace().next().ok_or("an empty tag")?;
            check_attributes(&tag[name.len()..])?;
            if !empty {
                open.push(name.to_owned());
            }
        }
        rest = &rest[end + 1..];
    }
    check_text(rest)?;
    if open.is_empty() {
        Ok(())
    } else {
        Err(format!("unclosed {open:?}"))
    }
}

fn check_text(text: &str) -> Result<(), String> {
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        let entity = &rest[at..];
        let ok = ["&amp;", "&lt;", "&gt;", "&quot;", "&#39;"]
            .iter()
            .any(|known| entity.starts_with(known));
        if !ok {
            return Err(format!("a bare & in {text:?}"));
        }
        rest = &rest[at + 1..];
    }
    if text.contains('<') || text.contains('>') {
        return Err(format!("markup in {text:?}"));
    }
    Ok(())
}

fn check_attributes(text: &str) -> Result<(), String> {
    let mut rest = text.trim();
    while !rest.is_empty() {
        let eq = rest
            .find("=\"")
            .ok_or_else(|| format!("an unquoted attribute in {text:?}"))?;
        let value_end = rest[eq + 2..].find('"').ok_or("an unclosed value")? + eq + 2;
        check_text(&rest[eq + 2..value_end])?;
        rest = rest[value_end + 1..].trim_start();
    }
    Ok(())
}

fn xml_of(file: &rb_plugin_types::FindingsFile) -> String {
    with_context(|context| render(file, context))
}

#[test]
fn the_fixture_is_well_formed_junit_with_five_failures() {
    let xml = xml_of(&fixture());
    well_formed(&xml).expect("well-formed");
    assert_eq!(xml.matches("<failure ").count(), 5);
    assert_eq!(xml.matches("<testsuite ").count(), 2);
    assert!(xml.contains("<testsuites name=\"title run-1700000000000\" tests=\"5\" failures=\"5\" errors=\"0\" skipped=\"0\">"));
}

#[test]
fn there_is_one_suite_per_breakpoint_and_one_case_per_rule() {
    let xml = xml_of(&fixture());
    let mobile = xml
        .find("<testsuite name=\"mobile\" tests=\"3\" failures=\"3\" errors=\"0\" skipped=\"0\">");
    let desktop = xml
        .find("<testsuite name=\"desktop\" tests=\"2\" failures=\"2\" errors=\"0\" skipped=\"0\">");
    assert!(mobile.is_some() && desktop.is_some(), "{xml}");
    assert!(mobile < desktop, "the order of the checked views");
    assert!(xml.contains("<testcase classname=\"seo\" name=\"seo.title\">"));
    assert!(xml.contains("<property name=\"engines\" value=\"chromium\"/>"));
}

#[test]
fn two_findings_of_one_rule_in_a_breakpoint_are_one_case() {
    let mut file = fixture();
    let mut second = file.findings[0].clone();
    second.id = "id-second".to_owned();
    file.findings.push(second);
    let xml = xml_of(&file);
    well_formed(&xml).expect("well-formed");
    assert_eq!(xml.matches("name=\"seo.title\"").count(), 1);
    assert!(xml.contains("message=\"count(count=2)\""), "{xml}");
}

#[test]
fn a_finding_in_two_breakpoints_fails_in_both_suites() {
    let mut file = fixture();
    file.findings[0].views = vec![1, 2];
    file.findings[0].breakpoints = vec!["mobile".to_owned(), "desktop".to_owned()];
    let xml = xml_of(&file);
    assert_eq!(xml.matches("name=\"seo.title\"").count(), 2);
}

#[test]
fn markup_and_control_characters_never_reach_the_xml() {
    let mut file = fixture();
    file.findings[0].rule_id = "seo.\u{1}title\"&".to_owned();
    let xml = xml_of(&file);
    well_formed(&xml).expect("well-formed");
    assert!(!xml.contains(MARKUP));
    assert!(xml.contains("&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;"));
    assert!(xml.contains("name=\"seo.title&quot;&amp;\""));
}

#[test]
fn a_failed_plugin_is_an_error_and_a_skipped_one_is_skipped() {
    let mut file = fixture();
    file.failures.push(CheckFailure {
        plugin: "a11y".to_owned(),
        view: Some(2),
        code: Some("plugins.error.timed_out".to_owned()),
        params: Default::default(),
    });
    file.skipped.push(CheckSkip {
        plugin: "seo".to_owned(),
        view: None,
        reason: SKIP_ENGINE.to_owned(),
        engine: Some("webkit".to_owned()),
    });
    let xml = xml_of(&file);
    well_formed(&xml).expect("well-formed");
    assert!(xml.contains("<error message=\"plugin_failed(plugin=a11y,reason=plugins.error.timed_out)\" type=\"plugin\"/>"), "{xml}");
    assert!(
        xml.contains(
            "<testsuite name=\"all_views\" tests=\"1\" failures=\"0\" errors=\"0\" skipped=\"1\">"
        ),
        "{xml}"
    );
    assert!(xml.contains("errors=\"1\" skipped=\"1\">"));
}

#[test]
fn a_plugin_without_findings_passes_in_every_suite() {
    let mut file = fixture();
    file.plugins.push("fonts".to_owned());
    let xml = xml_of(&file);
    assert_eq!(
        xml.matches("<testcase classname=\"fonts\" name=\"fonts\"/>")
            .count(),
        2
    );
    assert!(xml.contains("tests=\"7\" failures=\"5\""));
}
