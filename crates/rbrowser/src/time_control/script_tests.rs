use serde_json::json;

use super::*;
use crate::time_control::model::{Clock, ClockStart};

#[test]
fn the_clock_script_names_its_key_and_the_reset_reaches_it() {
    assert!(CLOCK_SOURCE.contains("Symbol.for('rbrowser.clock')"));
    assert!(RESET_EXPRESSION.contains(&format!("Symbol.for(\"{CLOCK_KEY}\")")));
    assert!(RESET_EXPRESSION.contains(".reset()"));
}

#[test]
fn a_view_gets_the_script_then_its_clock_as_json() {
    let clock = Clock::starting(ClockStart::Instant(5_000), 0.0, 1_000);
    let source = install_source(&clock);
    assert!(source.starts_with(CLOCK_SOURCE));
    let call = source.strip_prefix(CLOCK_SOURCE).expect("the script first");
    let config = call
        .strip_prefix("\n;globalThis[Symbol.for(\"rbrowser.clock\")].set(")
        .and_then(|rest| rest.strip_suffix(");"))
        .expect("then set(config)");
    let config: serde_json::Value = serde_json::from_str(config).expect("JSON");
    assert_eq!(
        config,
        json!({ "origin": 5_000, "offset": 0.0, "anchor": 1_000, "speed": 0.0 })
    );
}

#[test]
fn the_step_is_the_function_called_with_its_ms() {
    let expression = step_expression(250);
    assert!(expression.starts_with('('));
    assert!(expression.contains(STEP_SOURCE.trim_end()));
    assert!(expression.ends_with(")(250)"));
}

#[test]
fn the_rate_goes_to_the_document_timeline() {
    assert_eq!(RATE_METHOD, "Animation.setPlaybackRate");
    assert_eq!(rate_params(0.25), json!({ "playbackRate": 0.25 }));
}
