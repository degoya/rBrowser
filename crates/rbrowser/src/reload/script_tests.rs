use serde_json::json;

use super::*;

#[test]
fn the_swap_is_a_call_with_json_arguments() {
    let expression = swap_expression(&["app.css".into()], &["logo.png".into()], false);
    assert!(expression.starts_with("(// Swaps changed stylesheets"));
    assert!(
        expression.ends_with(r#")({"allCss":false,"css":["app.css"],"images":["logo.png"]})"#)
            || expression
                .ends_with(r#")({"css":["app.css"],"images":["logo.png"],"allCss":false})"#),
        "{expression}"
    );
}

#[test]
fn a_hostile_file_name_stays_a_string() {
    let expression = swap_expression(&[r#"a"]);alert(1);//.css"#.into()], &[], false);
    assert!(
        expression.contains(r#"\"]);alert(1);//.css"#),
        "{expression}"
    );
}

#[test]
fn the_probes_are_expressions() {
    for probe in [HMR_PROBE, TYPING_PROBE] {
        assert!(probe.starts_with("(function () {"));
        assert!(probe.ends_with("})()"));
    }
    assert!(HMR_PROBE.contains("/@vite/client"));
    assert!(TYPING_PROBE.contains("document.activeElement"));
}

#[test]
fn evaluate_asks_for_the_value_and_reads_it_back() {
    let params = evaluate_params("1 + 1");
    assert_eq!(params["expression"], "1 + 1");
    assert_eq!(params["returnByValue"], true);
    let ok = json!({ "result": { "type": "number", "value": 2 } });
    assert_eq!(evaluate_value(&ok), Some(&json!(2)));
    let thrown = json!({ "result": { "type": "object" }, "exceptionDetails": { "text": "x" } });
    assert_eq!(evaluate_value(&thrown), None);
}
