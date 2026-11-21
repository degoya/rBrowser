use rb_plugin_types::CheckFailure;
use serde_json::{Value, json};

use super::super::testing::{MARKUP, PAGE, fixture, with_context};
use super::*;

/// What the SARIF 2.1.0 schema requires of the objects this export writes, and the values it
/// allows where the export sets them (OASIS SARIF 2.1.0, `sarif-schema-2.1.0.json`).
fn problems(log: &Value) -> Vec<String> {
    let mut found = Vec::new();
    let mut need = |ok: bool, what: String| {
        if !ok {
            found.push(what);
        }
    };
    need(log["version"] == "2.1.0", "sarifLog.version".into());
    need(log["$schema"].is_string(), "sarifLog.$schema".into());
    let runs = log["runs"].as_array().cloned().unwrap_or_default();
    need(log["runs"].is_array(), "sarifLog.runs".into());
    for (r, run) in runs.iter().enumerate() {
        let driver = &run["tool"]["driver"];
        need(
            driver["name"].as_str().is_some_and(|name| !name.is_empty()),
            format!("runs[{r}].tool.driver.name"),
        );
        let rules = driver["rules"].as_array().cloned().unwrap_or_default();
        for (i, rule) in rules.iter().enumerate() {
            need(rule["id"].is_string(), format!("runs[{r}].rules[{i}].id"));
        }
        for (i, invocation) in run["invocations"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            need(
                invocation["executionSuccessful"].is_boolean(),
                format!("runs[{r}].invocations[{i}].executionSuccessful"),
            );
            for notification in invocation["toolExecutionNotifications"]
                .as_array()
                .cloned()
                .unwrap_or_default()
            {
                need(
                    notification["message"]["text"].is_string(),
                    format!("runs[{r}].invocations[{i}] notification message"),
                );
            }
        }
        for (i, result) in run["results"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let at = format!("runs[{r}].results[{i}]");
            need(
                result["message"]["text"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty()),
                format!("{at}.message.text"),
            );
            need(
                ["none", "note", "warning", "error"]
                    .contains(&result["level"].as_str().unwrap_or("")),
                format!("{at}.level"),
            );
            let index = result["ruleIndex"]
                .as_u64()
                .and_then(|i| usize::try_from(i).ok());
            need(
                index.and_then(|i| rules.get(i)).map(|rule| &rule["id"]) == Some(&result["ruleId"]),
                format!("{at}.ruleIndex"),
            );
            for location in result["locations"].as_array().cloned().unwrap_or_default() {
                let physical = &location["physicalLocation"];
                need(
                    physical.is_null() || physical["artifactLocation"]["uri"].is_string(),
                    format!("{at}.locations.physicalLocation"),
                );
            }
        }
    }
    found
}

fn log() -> Value {
    with_context(|context| render(&fixture(), context))
}

fn results(log: &Value) -> Vec<Value> {
    log["runs"]
        .as_array()
        .expect("runs")
        .iter()
        .flat_map(|run| run["results"].as_array().cloned().unwrap_or_default())
        .collect()
}

#[test]
fn the_fixture_is_valid_sarif_with_exactly_five_results_each_with_its_breakpoint() {
    let log = log();
    assert_eq!(problems(&log), Vec::<String>::new());
    let results = results(&log);
    assert_eq!(results.len(), 5);
    for result in &results {
        let breakpoints = result["properties"]["breakpoints"]
            .as_array()
            .expect("breakpoints");
        assert_eq!(breakpoints.len(), 1, "{result}");
        assert!(result["properties"]["views"].is_array());
        assert_eq!(result["properties"]["url"], PAGE);
    }
}

#[test]
fn there_is_one_run_per_plugin_in_the_order_they_ran() {
    let log = log();
    let names: Vec<&str> = log["runs"]
        .as_array()
        .expect("runs")
        .iter()
        .map(|run| run["tool"]["driver"]["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(names, ["seo", "a11y"]);
    assert_eq!(log["runs"][0]["results"].as_array().map(Vec::len), Some(3));
    assert_eq!(log["runs"][1]["results"].as_array().map(Vec::len), Some(2));
}

#[test]
fn a_result_carries_level_text_location_engine_and_fingerprint() {
    let log = log();
    let title = &log["runs"][0]["results"][0];
    assert_eq!(title["ruleId"], "seo.title");
    assert_eq!(title["level"], "error");
    assert_eq!(title["message"]["text"], "The page has no title");
    assert_eq!(
        title["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        PAGE
    );
    assert_eq!(
        title["locations"][0]["logicalLocations"][0]["fullyQualifiedName"],
        "head>title"
    );
    assert_eq!(
        title["partialFingerprints"]["rbrowserFindingId/v1"],
        "id-seo.title"
    );
    assert_eq!(title["properties"]["engines"], json!(["chromium"]));
    assert_eq!(title["properties"]["views"], json!([1]));
    let alt = &log["runs"][1]["results"][1];
    assert_eq!(alt["level"], "note");
    assert_eq!(alt["properties"]["engines"], json!(["firefox"]));
}

#[test]
fn a_rule_names_its_help_page_only_when_it_is_a_web_page() {
    let log = log();
    let seo_rules = &log["runs"][0]["tool"]["driver"]["rules"];
    let images = seo_rules
        .as_array()
        .expect("rules")
        .iter()
        .find(|rule| rule["id"] == "seo.images")
        .expect("the rule");
    assert_eq!(images["helpUri"], "https://help.test/alt?a=1&b=2");
    let a11y_rules = &log["runs"][1]["tool"]["driver"]["rules"];
    let alt = a11y_rules
        .as_array()
        .expect("rules")
        .iter()
        .find(|rule| rule["id"] == "a11y.alt")
        .expect("the rule");
    assert!(alt.get("helpUri").is_none());
}

#[test]
fn markup_in_a_parameter_stays_text() {
    let log = log();
    let meta = &log["runs"][0]["results"][2];
    assert_eq!(
        meta["message"]["text"],
        format!("The meta tag {MARKUP} is missing")
    );
}

#[test]
fn a_failed_plugin_is_an_unsuccessful_invocation_with_its_reason() {
    let mut file = fixture();
    file.failures.push(CheckFailure {
        plugin: "a11y".to_owned(),
        view: Some(2),
        code: Some("plugins.error.timed_out".to_owned()),
        params: Default::default(),
    });
    let log = with_context(|context| render(&file, context));
    assert_eq!(problems(&log), Vec::<String>::new());
    assert_eq!(
        log["runs"][0]["invocations"][0]["executionSuccessful"],
        true
    );
    let a11y = &log["runs"][1]["invocations"][0];
    assert_eq!(a11y["executionSuccessful"], false);
    assert_eq!(
        a11y["toolExecutionNotifications"][0]["message"]["text"],
        "plugin_failed(plugin=a11y,reason=plugins.error.timed_out)"
    );
    assert_eq!(a11y["toolExecutionNotifications"][0]["level"], "error");
}

#[test]
fn a_plugin_without_findings_still_has_its_run() {
    let mut file = fixture();
    file.plugins.push("fonts".to_owned());
    let log = with_context(|context| render(&file, context));
    let fonts = &log["runs"][2];
    assert_eq!(fonts["tool"]["driver"]["name"], "fonts");
    assert_eq!(fonts["results"], json!([]));
}

#[test]
fn the_run_is_named_for_automation() {
    let log = log();
    assert_eq!(
        log["runs"][0]["automationDetails"]["id"],
        "rbrowser/checks/run-1700000000000"
    );
}
