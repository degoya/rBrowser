use serde_json::json;

use super::capped;

#[test]
fn a_small_value_passes_and_a_large_one_is_cut() {
    assert_eq!(capped(json!({ "a": 1 }), 100), json!({ "a": 1 }));
    let cut = capped(json!("\u{2603}".repeat(100)), 11);
    assert_eq!(cut["truncated"], true);
    assert_eq!(cut["bytes"], 302);
    assert!(cut["preview"].as_str().is_some_and(|text| text.len() <= 11));
}

#[test]
fn a_failure_of_a_prompt_or_resource_is_a_json_rpc_error_with_its_code() {
    use rmcp::model::ErrorCode;

    use super::rpc_error;
    use crate::backend::ToolFailure;

    let failure = |code: &str| ToolFailure::new(code, "why");
    for (code, expected) in [
        ("resources.not_found", ErrorCode::RESOURCE_NOT_FOUND),
        ("findings.none", ErrorCode::RESOURCE_NOT_FOUND),
        ("screenshots.none", ErrorCode::RESOURCE_NOT_FOUND),
        ("prompts.not_found", ErrorCode::INVALID_PARAMS),
        ("args.invalid", ErrorCode::INVALID_PARAMS),
        ("mcp.internal", ErrorCode::INTERNAL_ERROR),
        ("resources.disabled", ErrorCode::INVALID_REQUEST),
        ("answer.too_large", ErrorCode::INVALID_REQUEST),
    ] {
        assert_eq!(rpc_error(&failure(code)).code, expected, "{code}");
    }
    let error = rpc_error(&failure("prompts.disabled").with_setting("mcp.prompts"));
    assert_eq!(error.message, "why");
    assert_eq!(
        error.data,
        Some(
            json!({ "code": "prompts.disabled", "settingsKey": "mcp.prompts",
                    "legacyCode": "prompts_disabled" })
        )
    );
    assert_eq!(
        rpc_error(&failure("mcp.internal")).data,
        Some(json!({ "code": "mcp.internal", "legacyCode": "internal" }))
    );
}

#[test]
fn a_tool_failure_with_a_renamed_code_carries_the_flat_one_until_1_0() {
    use super::error_result;
    use crate::backend::ToolFailure;

    let body = |failure: &ToolFailure| -> serde_json::Value {
        let result = error_result(failure);
        let wire = serde_json::to_value(&result).expect("a result serialises");
        let text = wire["content"][0]["text"].as_str().expect("a text block");
        serde_json::from_str(text).expect("JSON text")
    };
    let renamed = ToolFailure::new("evaluate.disabled", "off").with_setting("mcp.allowEvaluate");
    assert_eq!(
        body(&renamed),
        json!({ "error": "off", "code": "evaluate.disabled",
                "settingsKey": "mcp.allowEvaluate", "legacyCode": "evaluate_disabled" })
    );
    let kept = ToolFailure::new("cdp.disabled", "off");
    assert_eq!(
        body(&kept),
        json!({ "error": "off", "code": "cdp.disabled" })
    );
}
