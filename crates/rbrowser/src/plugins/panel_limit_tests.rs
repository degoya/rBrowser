use serde_json::json;

use super::*;

#[test]
fn an_answer_within_the_limit_passes_one_over_it_names_the_setting() {
    let answer = json!({ "text": "x".repeat(100) });
    let size = answer.to_string().len();
    assert_eq!(bounded(answer.clone(), size), Ok(answer.clone()));
    let refused = bounded(answer, size - 1).expect_err("one byte over");
    assert_eq!(refused.code(), Some(codes::TOO_LARGE));
    assert_eq!(
        refused.param("limit"),
        Some((size - 1).to_string().as_str())
    );
    assert_eq!(refused.param("reason"), Some("panel_message_limit"));
    assert_eq!(
        refused.param("setting"),
        Some("plugins.panelMessageMaxBytes")
    );
}

#[test]
fn the_reason_has_a_text_that_names_the_setting_and_panels_get_it() {
    let key = format!("plugins.reason.{REASON}");
    let text = rb_i18n::t(&key);
    assert_ne!(text, key, "the catalogue has the reason's text");
    assert!(text.contains("Panel message limit"), "{text}");
    assert!(super::super::panel_errors::PANEL_KEYS.contains(&key.as_str()));
}
