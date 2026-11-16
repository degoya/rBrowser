//! The answer to the host's `@rules` request: the mocks in effect (under workspace trust, while
//! mocks are on, each rule unless a person switched it off) and the person's block list (always),
//! with the session's cache switch and "clear site data" counter; and the debugger's parts
//! (RB-080-37): the breakpoints and their switch, the person's settings for the host and the
//! asks the panel queued. An ask the host answered is dropped here (its error kept for the
//! panel). The host validates and applies it; a request never reaches this component.

use rb_plugin_api::PluginHost;
use serde_json::{Value, json};

use crate::context::{Context, Settings};
use crate::mocks;
use crate::store;

/// The id prefix of the block list's rules.
pub const BLOCK_PREFIX: &str = "settings:block-";

/// A breakpoint of the state in the document's form.
fn breakpoint_rule(breakpoint: &Value) -> Value {
    let mut matcher = json!({ "urlPattern": breakpoint["urlPattern"], "regex": breakpoint["regex"] == Value::Bool(true) });
    for key in ["method", "resourceType"] {
        if breakpoint[key].is_string() {
            matcher[key] = breakpoint[key].clone();
        }
    }
    let mut rule = json!({
        "id": breakpoint["id"],
        "match": matcher,
        "stage": breakpoint["stage"].as_str().unwrap_or("request"),
        "enabled": breakpoint["enabled"] != Value::Bool(false),
    });
    if breakpoint["views"].is_array() {
        rule["views"] = breakpoint["views"].clone();
    }
    rule
}

/// The rules document for the host.
#[must_use]
pub fn document(host: &mut dyn PluginHost, context: &Context) -> Value {
    let settings = Settings::read(host);
    let mut state = store::read(host, &context.session);
    let mut rules = Vec::new();
    if context.trusted && state.mocks_enabled(&settings) {
        for file in mocks::load(host, &settings.mocks_dir) {
            for rule in &file.rules {
                if !rule.enabled || state.disabled.contains(&file.qualified(rule)) {
                    continue;
                }
                match mocks::host_rule(host, &file, rule, &settings.mocks_dir) {
                    Ok(rule) => rules.push(rule),
                    Err(problem) => host.log("warn", &format!("{}: {problem}", file.path)),
                }
            }
        }
    }
    for (index, pattern) in settings.block.iter().enumerate() {
        rules.push(json!({
            "id": format!("{BLOCK_PREFIX}{index}"),
            "origin": "settings",
            "match": { "urlPattern": pattern },
            "action": { "type": "block" },
        }));
    }
    // An answered ask leaves the document; a refused one leaves its reason for the panel.
    let before = state.asks.len();
    let mut error = None;
    state.asks.retain(|ask| {
        let Some(answer) = ask["id"].as_str().and_then(|id| context.answers.get(id)) else {
            return true;
        };
        if let Some(reason) = answer["error"].as_str() {
            error = Some(reason.to_owned());
        }
        answer["done"] != Value::Bool(true) && answer["error"].is_null()
    });
    if state.asks.len() != before {
        if error.is_some() {
            state.last_error = error;
        }
        if let Err(failure) = store::write(host, &state) {
            host.log(
                "warn",
                &format!(
                    "the asks were not written: {}",
                    failure.code.unwrap_or_default()
                ),
            );
        }
    }
    json!({
        "rules": rules,
        "cacheDisabled": state.cache_disabled(&settings),
        "clearSiteData": state.clear_site_data,
        "breakpoints": state.breakpoints.iter().map(breakpoint_rule).collect::<Vec<_>>(),
        "breakpointsEnabled": state.breakpoints_enabled(),
        "settings": {
            "breakpointTimeoutMs": settings.breakpoint_timeout_ms,
            "maxBodyBytes": settings.rewrite_max_body_bytes,
            "mapLocalRoots": settings.map_local_roots,
            "allowComposeFromMcp": settings.allow_compose_from_mcp,
        },
        "asks": state.asks,
    })
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
