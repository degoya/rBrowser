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
        ("resource_not_found", ErrorCode::RESOURCE_NOT_FOUND),
        ("findings_none", ErrorCode::RESOURCE_NOT_FOUND),
        ("screenshots_none", ErrorCode::RESOURCE_NOT_FOUND),
        ("prompt_not_found", ErrorCode::INVALID_PARAMS),
        ("invalid_arguments", ErrorCode::INVALID_PARAMS),
        ("internal", ErrorCode::INTERNAL_ERROR),
        ("resources_disabled", ErrorCode::INVALID_REQUEST),
        ("answer_too_large", ErrorCode::INVALID_REQUEST),
    ] {
        assert_eq!(rpc_error(&failure(code)).code, expected, "{code}");
    }
    let error = rpc_error(&failure("prompts_disabled").with_setting("mcp.prompts"));
    assert_eq!(error.message, "why");
    assert_eq!(
        error.data,
        Some(json!({ "code": "prompts_disabled", "settingsKey": "mcp.prompts" }))
    );
    assert_eq!(
        rpc_error(&failure("internal")).data,
        Some(json!({ "code": "internal" }))
    );
}
