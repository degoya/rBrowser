use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, Failure, ViewInfo};
use serde_json::{Value, json};

use crate::Overlays;
use crate::state::{STATE_FILE, STATE_PATH};

fn view(id: u32, engine: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("view {id}"),
        breakpoint: format!("bp{id}"),
        engine: engine.to_owned(),
        width: 375,
        height: 800,
        dpr: 1.0,
        url: "http://localhost/overlays.html".to_owned(),
        title: "Overlays".to_owned(),
    }
}

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::Cdp)
        .grant(Capability::Findings)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite);
    host.cdp_domains = ["Accessibility", "DOM", "CSS", "Emulation"]
        .map(str::to_owned)
        .to_vec();
    host.views = vec![view(1, "chromium"), view(2, "chromium")];
    host
}

fn sync(host: &mut MemoryHost) {
    if let Some(bytes) = host.out.get(STATE_FILE).cloned() {
        host.project.insert(STATE_PATH.to_owned(), bytes);
    }
}

fn tool(host: &mut MemoryHost, tool: &str, args: Value) -> Result<Value, Failure> {
    let answer = Overlays.call_tool(host, tool, &args.to_string());
    sync(host);
    answer.map(|text| serde_json::from_str(&text).expect("JSON"))
}

fn command(host: &mut MemoryHost, command: &str, args: Value) -> Result<Value, Failure> {
    let answer = Overlays.run_command(host, command, &args.to_string());
    sync(host);
    answer.map(|text| serde_json::from_str(&text).expect("JSON"))
}

fn sent(host: &MemoryHost, method: &str) -> Vec<(u32, Value)> {
    host.cdp_sent
        .iter()
        .filter(|(_, sent, _)| sent == method)
        .map(|(view, _, params)| (*view, serde_json::from_str(params).expect("params")))
        .collect()
}

/// What the CDP calls of force-state answer when someone requested the document already: the
/// accessibility root (backend node 3), pushed as node 1, the element found as node 7.
const PUSHED: &str = r#"{"nodes":[{"backendDOMNodeId":3}],"nodeIds":[1],"nodeId":7}"#;

const HITS: &str = r#"{"count":1,"items":[{"selector":"button.tiny","path":"html > body > button","label":"20x20","rect":{"x":0,"y":0,"width":20,"height":20}}]}"#;

#[test]
fn switching_target_size_on_answers_the_hits_per_view_and_reports_findings() {
    let mut host = host();
    host.evaluate_answer = HITS.to_owned();
    let answer = tool(
        &mut host,
        "set",
        json!({ "overlay": "target-size", "target": "all" }),
    )
    .expect("on");
    assert_eq!(answer["overlays"]["target-size"]["enabled"], json!(true));
    let hits = answer["hits"].as_array().expect("hits");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0]["view"], json!(1));
    assert_eq!(hits[0]["count"], json!(1));
    assert_eq!(hits[0]["items"][0]["selector"], json!("button.tiny"));
    let (_, script) = host.evaluated.last().expect("evaluated");
    assert!(
        script.contains(r#"scan("target-size", {"sizePx":24})"#),
        "{script}"
    );
    assert_eq!(host.findings.len(), 2);
    assert_eq!(host.findings[0].rule, "target-size");
    assert_eq!(host.findings[1].view, Some(2));

    let evaluated = host.evaluated.len();
    let off = tool(
        &mut host,
        "set",
        json!({ "overlay": "target-size", "enabled": false }),
    )
    .expect("off");
    assert!(off.get("hits").is_none());
    assert_eq!(host.evaluated.len(), evaluated);
}

#[test]
fn findings_stay_out_with_report_findings_off_and_the_option_wins_over_the_setting() {
    let mut host = host();
    host.evaluate_answer = HITS.to_owned();
    host.settings
        .insert("reportFindings".to_owned(), "false".to_owned());
    host.settings
        .insert("targetSizePx".to_owned(), "44".to_owned());
    tool(&mut host, "set", json!({ "overlay": "target-size" })).expect("on");
    assert!(host.findings.is_empty());
    let (_, script) = host.evaluated.last().expect("evaluated");
    assert!(script.contains(r#"{"sizePx":44}"#), "{script}");
    tool(
        &mut host,
        "set",
        json!({ "overlay": "small-text", "options": { "px": 10 } }),
    )
    .expect("on");
    let (_, script) = host.evaluated.last().expect("evaluated");
    assert!(
        script.contains(r#"scan("small-text", {"px":10})"#),
        "{script}"
    );
}

#[test]
fn force_state_forces_the_pseudo_classes_in_every_chromium_view_and_releases_them() {
    let mut host = host();
    host.views.push(view(3, "firefox"));
    host.cdp_answer = PUSHED.to_owned();
    let answer = tool(
        &mut host,
        "set",
        json!({ "overlay": "force-state", "options": { "selector": "nav a", "states": ["hover", "focus"] } }),
    )
    .expect("on");
    assert_eq!(
        answer["forced"],
        json!([
            { "view": 1, "ok": true },
            { "view": 2, "ok": true },
            { "view": 3, "error": "unsupported" },
        ])
    );
    let forced = sent(&host, "CSS.forcePseudoState");
    assert_eq!(forced.len(), 2);
    assert_eq!(
        forced[0].1,
        json!({ "nodeId": 7, "forcedPseudoClasses": ["hover", "focus"] })
    );
    assert_eq!(
        sent(&host, "DOM.querySelector")[0].1,
        json!({ "nodeId": 1, "selector": "nav a" })
    );
    assert!(host.cdp_sent.iter().all(|(view, _, _)| *view != 3));

    tool(
        &mut host,
        "set",
        json!({ "overlay": "force-state", "enabled": false }),
    )
    .expect("off");
    let forced = sent(&host, "CSS.forcePseudoState");
    assert_eq!(forced.len(), 4);
    assert_eq!(
        forced[3].1,
        json!({ "nodeId": 7, "forcedPseudoClasses": [] })
    );
}

#[test]
fn force_state_keeps_to_the_document_someone_requested_and_switches_no_domain_on() {
    let mut host = host();
    host.cdp_answer = PUSHED.to_owned();
    tool(
        &mut host,
        "set",
        json!({ "overlay": "force-state", "options": { "selector": "nav a" } }),
    )
    .expect("on");
    assert_eq!(
        sent(&host, "DOM.pushNodesByBackendIdsToFrontend")[0].1,
        json!({ "backendNodeIds": [3] })
    );
    for method in ["DOM.getDocument", "DOM.enable", "CSS.enable"] {
        assert!(sent(&host, method).is_empty(), "{method} sent");
    }
    assert_eq!(sent(&host, "CSS.forcePseudoState").len(), 2);
}

#[test]
fn force_state_requests_the_document_only_when_nobody_has() {
    let mut host = host();
    // The push names no node: nobody requested the document yet.
    host.cdp_answer =
        r#"{"nodes":[{"backendDOMNodeId":3}],"root":{"nodeId":1},"nodeId":7}"#.to_owned();
    tool(
        &mut host,
        "set",
        json!({ "overlay": "force-state", "target": 2, "options": { "selector": "nav a" } }),
    )
    .expect("on");
    assert_eq!(sent(&host, "DOM.getDocument").len(), 1);
    assert_eq!(
        sent(&host, "DOM.querySelector")[0].1,
        json!({ "nodeId": 1, "selector": "nav a" })
    );
}

#[test]
fn force_state_switches_the_domains_on_only_when_the_force_is_refused() {
    let mut host = host();
    host.cdp_answer = PUSHED.to_owned();
    // Without the CSS domain every CSS command is refused, as a CSS domain that is off refuses it.
    host.cdp_domains = ["Accessibility", "DOM"].map(str::to_owned).to_vec();
    let answer = tool(
        &mut host,
        "set",
        json!({ "overlay": "force-state", "target": 1, "options": { "selector": "nav a" } }),
    )
    .expect("on");
    assert!(answer["forced"][0]["error"].is_string(), "{answer}");
    assert_eq!(sent(&host, "DOM.enable").len(), 1);
}

#[test]
fn force_state_reports_an_element_it_cannot_find() {
    let mut host = host();
    host.cdp_answer = r#"{"nodes":[{"backendDOMNodeId":3}],"nodeIds":[1],"nodeId":0}"#.to_owned();
    let answer = tool(
        &mut host,
        "set",
        json!({ "overlay": "force-state", "target": 2, "options": { "selector": ".gone" } }),
    )
    .expect("on");
    assert_eq!(
        answer["forced"],
        json!([{ "view": 2, "error": "not_found" }])
    );
    assert!(sent(&host, "CSS.forcePseudoState").is_empty());
}

#[test]
fn the_force_state_command_takes_the_highlight_selector_and_hover_by_default() {
    let mut host = host();
    let refused = command(&mut host, "force_state", json!({})).expect_err("no selector");
    assert_eq!(refused.code(), Some("plugin.overlays.errors.no_selector"));
    host.evaluate_answer = "1".to_owned();
    command(&mut host, "highlight", json!({ "selector": "nav a" })).expect("highlight");
    host.cdp_answer = PUSHED.to_owned();
    let answer = command(&mut host, "force_state", json!({})).expect("on");
    assert_eq!(
        answer["overlays"]["force-state"]["options"],
        json!({ "selector": "nav a", "states": ["hover"] })
    );
}

#[test]
fn js_off_disables_script_execution_and_reloads_and_all_off_restores_it() {
    let mut host = host();
    let answer = command(&mut host, "js_off", json!({})).expect("on");
    assert_eq!(
        answer["scripts"],
        json!([{ "view": 1, "ok": true }, { "view": 2, "ok": true }])
    );
    let disabled = sent(&host, "Emulation.setScriptExecutionDisabled");
    assert_eq!(
        disabled,
        vec![(1, json!({ "value": true })), (2, json!({ "value": true }))]
    );
    assert!(
        host.evaluated
            .iter()
            .all(|(_, script)| script.contains("location.reload()"))
    );
    assert_eq!(host.evaluated.len(), 2);

    command(&mut host, "off", json!({})).expect("all off");
    let disabled = sent(&host, "Emulation.setScriptExecutionDisabled");
    assert_eq!(disabled.len(), 4);
    assert_eq!(disabled[3], (2, json!({ "value": false })));
    // Off again changes nothing in the views.
    command(&mut host, "off", json!({})).expect("all off again");
    assert_eq!(sent(&host, "Emulation.setScriptExecutionDisabled").len(), 4);
}

#[test]
fn a_new_document_gets_its_forced_state_and_disabled_scripts_again() {
    let mut host = host();
    host.cdp_answer = PUSHED.to_owned();
    tool(
        &mut host,
        "set",
        json!({ "overlay": "force-state", "options": { "selector": "a" } }),
    )
    .expect("force");
    command(&mut host, "js_off", json!({})).expect("js off");
    host.cdp_sent.clear();
    host.evaluated.clear();
    tool(
        &mut host,
        "@page",
        json!({ "view": 2, "message": { "type": "hello" } }),
    )
    .expect("hello");
    assert_eq!(sent(&host, "CSS.forcePseudoState").len(), 1);
    assert_eq!(
        sent(&host, "Emulation.setScriptExecutionDisabled"),
        vec![(2, json!({ "value": true }))]
    );
    assert!(host.evaluated.is_empty(), "no reload from a hello");
    assert!(host.cdp_sent.iter().all(|(view, _, _)| *view == 2));
}

#[test]
fn a_start_never_begins_with_scripts_disabled() {
    let mut host = host();
    command(&mut host, "js_off", json!({})).expect("js off");
    Overlays.activate(&mut host).expect("activate");
    sync(&mut host);
    let state = tool(&mut host, "state", json!({})).expect("state");
    assert_eq!(state["overlays"]["js-off"]["enabled"], json!(false));
}

#[test]
fn the_active_view_as_target_sets_that_view_only() {
    let mut host = host();
    host.active = Some(2);
    let answer = tool(
        &mut host,
        "set",
        json!({ "overlay": "focus-tracking", "target": "active" }),
    )
    .expect("on");
    assert_eq!(answer["overlays"]["focus-tracking"]["enabled"], json!(true));
    let state = tool(&mut host, "state", json!({})).expect("state");
    assert_eq!(state["overlays"]["focus-tracking"]["enabled"], json!(false));
    let state = tool(&mut host, "state", json!({ "target": 2 })).expect("state");
    assert_eq!(state["overlays"]["focus-tracking"]["enabled"], json!(true));
}

#[test]
fn every_new_command_toggles_its_overlay() {
    for (command_id, overlay) in crate::overlay::COMMANDS {
        if matches!(*overlay, "highlight" | "force-state") {
            continue;
        }
        let mut host = host();
        let on = command(&mut host, command_id, json!({})).expect("on");
        assert_eq!(
            on["overlays"][overlay]["enabled"],
            json!(true),
            "{command_id}"
        );
        let off = command(&mut host, command_id, json!({})).expect("off");
        assert_eq!(
            off["overlays"][overlay]["enabled"],
            json!(false),
            "{command_id}"
        );
    }
}
