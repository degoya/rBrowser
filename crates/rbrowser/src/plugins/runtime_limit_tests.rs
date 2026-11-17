//! RB-090-44: a panel's call is held to `plugins.panelMessageMaxBytes` (4 MiB by default), a
//! page script's request to the page bridge's 64 KiB; over its limit a panel's answer is refused
//! with the code, the limit and the setting's name.

use super::*;
use crate::plugins::panel_limit::{REASON, SETTING};

const MIB: usize = 1024 * 1024;

fn one_mib() -> String {
    json!({ "text": "x".repeat(MIB) }).to_string()
}

#[tokio::test]
async fn a_panel_takes_an_answer_of_one_mib_a_page_script_does_not() {
    let (_fake, _events, runtime) = setup(vec![plugin("hello", PluginSource::Bundled, &one_mib())]);
    let limit = rb_config::PluginSettings::default().panel_message_limit();
    let answer = runtime
        .panel_call_tool("hello", "greet", &json!({}), limit)
        .await
        .expect("1 MiB fits the default limit");
    assert_eq!(answer["text"].as_str().map(str::len), Some(MIB));
    let answer = runtime
        .panel_run_command("hello", "greet", &json!({}), limit)
        .await
        .expect("a command's answer too");
    assert_eq!(answer["text"].as_str().map(str::len), Some(MIB));
    assert_eq!(
        runtime.request("hello", ViewId(1), json!(1)).await,
        Err(codes::TOO_LARGE.to_owned()),
        "the page bridge keeps its 64 KiB"
    );
}

#[tokio::test]
async fn over_the_setting_a_panel_s_answer_is_refused_naming_it() {
    let (_fake, _events, runtime) = setup(vec![plugin("hello", PluginSource::Bundled, &one_mib())]);
    let refused = runtime
        .panel_call_tool("hello", "greet", &json!({}), MIB)
        .await
        .expect_err("over a 1 MiB limit");
    assert_eq!(refused.code(), Some(codes::TOO_LARGE));
    assert_eq!(refused.param("limit"), Some("1048576"));
    assert_eq!(refused.param("reason"), Some(REASON));
    assert_eq!(refused.param("setting"), Some(SETTING));
}
