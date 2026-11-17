use rb_sync::CaptureSwitches;
use serde_json::json;

use super::*;
use crate::client::DEFAULT_TIMEOUT;
use crate::testing::FakeFirefox;

fn config() -> AgentConfig {
    AgentConfig {
        active: true,
        capture: CaptureSwitches {
            click: true,
            hover: false,
            scroll: true,
            keyboard: true,
            input: true,
            forms: true,
            focus: false,
        },
        hover_throttle_ms: 100,
        scroll_debounce_ms: rb_sync::message::DEFAULT_SCROLL_DEBOUNCE_MS,
        scroll_mode: rb_sync::AgentScrollMode::Anchor,
    }
}

#[test]
fn every_view_has_its_own_channel() {
    assert_eq!(channel(ViewId(7)), "rb-agent-7");
    assert_eq!(view_of("rb-agent-7"), Some(ViewId(7)));
    assert_eq!(view_of("rb-agent-x"), None);
    assert_eq!(view_of("other-7"), None);
}

#[test]
fn the_preload_makes_the_channel_the_binding_then_boots() {
    let source = preload_source("BUNDLE();", &config()).expect("a bundle");
    assert!(source.starts_with("(emit) => {"), "{source}");
    let binding = source.find("globalThis.__rbEmit = (payload) => { emit(payload); };");
    let boot = source.find("globalThis.__rbAgentBoot = {\"active\":true,");
    let bundle = source.find("BUNDLE();");
    assert!(
        binding.is_some() && boot.is_some() && bundle.is_some(),
        "{source}"
    );
    assert!(binding < boot && boot < bundle, "{source}");
    assert!(source.trim_end().ends_with('}'));
    assert_eq!(
        preload_source("  \n", &config()),
        None,
        "no bundle, no agent"
    );
}

#[tokio::test]
async fn a_command_runs_in_the_sandbox_and_its_answer_is_typed() {
    let fake = FakeFirefox::start().await;
    let client = BidiClient::connect(&fake.endpoint(), DEFAULT_TIMEOUT)
        .await
        .expect("connect");
    fake.answer_agent(json!({ "status": "not_visible" }));
    let answer = command(&client, "ctx-1", &AgentCommand::Ping)
        .await
        .expect("answer");
    assert_eq!(answer, CommandResult::NotVisible);
    let call = fake.calls("script.evaluate").pop().expect("evaluated");
    assert_eq!(
        call["target"],
        json!({ "context": "ctx-1", "sandbox": "rbrowser" })
    );
    assert_eq!(
        call["expression"],
        "JSON.stringify(globalThis.__rbAgent.command({\"cmd\":\"ping\"}))"
    );
}

#[tokio::test]
async fn a_page_operation_gets_its_data_as_a_json_literal() {
    let fake = FakeFirefox::start().await;
    let client = BidiClient::connect(&fake.endpoint(), DEFAULT_TIMEOUT)
        .await
        .expect("connect");
    let answer = page(
        &client,
        "ctx-1",
        BLOCK_SUBMIT,
        &json!({ "target": "x');alert(1);('" }),
    )
    .await
    .expect("answer");
    assert_eq!(answer, PageAnswer::Ok);
    let call = fake.calls("script.evaluate").pop().expect("evaluated");
    let expression = call["expression"].as_str().expect("text");
    assert!(expression.starts_with("((d)=>{const stop="), "{expression}");
    assert!(
        expression.ends_with("})({\"target\":\"x');alert(1);('\"})"),
        "{expression}"
    );
}
