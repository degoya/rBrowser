use rb_engine::Emulation;
use serde_json::json;

use super::*;
use crate::emulation::cdp::emulation_calls;

const CHROME: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.6778.86 Safari/537.36";
const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

fn engine() -> EngineUserAgent {
    engine_user_agent(&json!({
        "protocolVersion": "1.3",
        "product": "Chrome/131.0.6778.86",
        "userAgent": CHROME,
    }))
    .expect("Browser.getVersion's answer")
}

fn user_agent_call(emulation: &Emulation) -> serde_json::Value {
    emulation_calls(None, emulation, true)
        .into_iter()
        .find(|call| call.method == "Emulation.setUserAgentOverride")
        .map(|call| call.params)
        .expect("a user agent call")
}

#[test]
fn the_product_is_rbrowser_and_the_version() {
    assert_eq!(
        UA_PRODUCT,
        format!("rBrowser/{}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_suffix_goes_at_the_end_once() {
    let suffixed = with_suffix(CHROME);
    assert!(suffixed.ends_with(&format!(" {UA_PRODUCT}")), "{suffixed}");
    assert_eq!(with_suffix(&suffixed), suffixed);
}

#[test]
fn the_engines_user_agent_gets_the_suffix_and_chromium_brands() {
    let engine = engine();
    let emulation = suffixed(&Emulation::default(), Some(&engine));
    let params = with_brands(user_agent_call(&emulation), &Emulation::default(), &engine);
    let user_agent = params["userAgent"].as_str().expect("a user agent");
    assert!(user_agent.starts_with(CHROME));
    assert!(user_agent.ends_with(UA_PRODUCT));
    let metadata = &params["userAgentMetadata"];
    assert_eq!(
        metadata["brands"][0],
        json!({ "brand": "Chromium", "version": "131" })
    );
    assert_eq!(
        metadata["fullVersionList"][0],
        json!({ "brand": "Chromium", "version": "131.0.6778.86" })
    );
    assert_eq!(metadata["platform"], "Windows");
}

#[test]
fn an_emulated_user_agent_gets_the_suffix_and_no_chromium_brands() {
    let engine = engine();
    let emulation = Emulation {
        user_agent: Some(IPHONE.to_owned()),
        mobile: true,
        ..Emulation::default()
    };
    let next = suffixed(&emulation, Some(&engine));
    assert_eq!(
        next.user_agent.as_deref(),
        Some(format!("{IPHONE} {UA_PRODUCT}").as_str())
    );
    let params = with_brands(user_agent_call(&next), &emulation, &engine);
    assert!(params["userAgentMetadata"].get("brands").is_none());
    assert_eq!(params["userAgentMetadata"]["platform"], "iOS");
}

#[test]
fn without_the_engines_user_agent_only_an_emulated_one_gets_the_suffix() {
    assert_eq!(suffixed(&Emulation::default(), None), Emulation::default());
    let emulation = Emulation {
        user_agent: Some(IPHONE.to_owned()),
        ..Emulation::default()
    };
    assert!(
        suffixed(&emulation, None)
            .user_agent
            .is_some_and(|agent| agent.ends_with(UA_PRODUCT))
    );
}

#[test]
fn switching_the_suffix_changes_the_user_agent_call() {
    let engine = engine();
    let plain = Emulation::default();
    let with = suffixed(&plain, Some(&engine));
    let on = emulation_calls(Some(&plain), &with, false);
    assert!(
        on.iter()
            .any(|call| call.method == "Emulation.setUserAgentOverride")
    );
    let off = emulation_calls(Some(&with), &plain, false);
    let call = off
        .iter()
        .find(|call| call.method == "Emulation.setUserAgentOverride")
        .expect("the suffix is taken off again");
    assert_eq!(call.params["userAgent"], "");
}

#[test]
fn a_malformed_version_answer_gives_no_engine_user_agent() {
    assert!(engine_user_agent(&json!({ "product": "Chrome/131" })).is_none());
    assert!(engine_user_agent(&json!({ "userAgent": "" })).is_none());
    let bare = engine_user_agent(&json!({ "userAgent": CHROME })).expect("product optional");
    let emulation = suffixed(&Emulation::default(), Some(&bare));
    let params = with_brands(user_agent_call(&emulation), &Emulation::default(), &bare);
    assert!(params["userAgentMetadata"].get("brands").is_none());
}
