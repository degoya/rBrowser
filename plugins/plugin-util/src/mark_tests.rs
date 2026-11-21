use std::collections::BTreeMap;

use rb_plugin_api::{HttpRequest, HttpResponse, ViewInfo};
use serde_json::json;

use super::*;

/// A host that answers CDP by method and records every call; the rest is not needed here.
#[derive(Default)]
struct FakeHost {
    /// CDP answers by method; an unknown method answers `{}`.
    cdp: BTreeMap<String, Result<Value, Failure>>,
    cdp_sent: Vec<(ViewId, String, Value)>,
    evaluated: Vec<(ViewId, String)>,
}

impl FakeHost {
    fn methods(&self) -> Vec<&str> {
        self.cdp_sent.iter().map(|(_, m, _)| m.as_str()).collect()
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, _level: &str, _message: &str) {}

    fn now_unix_seconds(&mut self) -> u64 {
        0
    }

    fn setting(&mut self, _key: &str) -> Option<String> {
        None
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Ok(Vec::new())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(None)
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.evaluated.push((view, script.to_owned()));
        Ok("null".to_owned())
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        self.cdp_sent.push((view, method.to_owned(), params));
        match self.cdp.get(method) {
            Some(Ok(value)) => Ok(value.to_string()),
            Some(Err(failure)) => Err(failure.clone()),
            None => Ok("{}".to_owned()),
        }
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Failure::new(FailureKind::NotSupported))
    }

    fn project_read(&mut self, _path: &str) -> Result<Vec<u8>, Failure> {
        Err(Failure::new(FailureKind::NotSupported))
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Err(Failure::new(FailureKind::NotSupported))
    }

    fn out_write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), Failure> {
        Err(Failure::new(FailureKind::NotSupported))
    }

    fn report_findings(&mut self, _items: &[rb_plugin_api::Finding]) -> Result<(), Failure> {
        Err(Failure::new(FailureKind::NotSupported))
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Failure::new(FailureKind::NotSupported))
    }
}

#[test]
fn targets_are_checked() {
    let ok = targets(&json!([{ "view": 1, "selector": "#a" }, { "view": 2, "backendNodeId": 7 }]))
        .expect("targets");
    assert_eq!(ok[0], (1, Locator::Selector("#a".to_owned())));
    assert_eq!(ok[1], (2, Locator::Backend(7)));
    for bad in [
        json!([]),
        json!("x"),
        json!([{ "view": -1, "selector": "a" }]),
        json!([{ "view": 1, "selector": "  " }]),
        json!([{ "view": 1, "selector": "a\nb" }]),
        json!([{ "view": 1, "selector": "a".repeat(MAX_SELECTOR_CHARS + 1) }]),
        json!([{ "view": 1 }]),
    ] {
        let failure = targets(&bad).expect_err("refused");
        assert_eq!(failure.code(), Some(codes::INVALID), "{bad}");
    }
}

#[test]
fn a_selector_is_a_cdp_parameter_and_the_element_is_scrolled_and_highlighted() {
    let mut host = FakeHost::default();
    // Someone requested the document already: it is found without requesting it again, which
    // would drop every node id the host and the other plugins hold (review L2 of Charge 7).
    host.cdp.insert(
        "Accessibility.getFullAXTree".to_owned(),
        Ok(json!({ "nodes": [{ "backendDOMNodeId": 5 }] })),
    );
    host.cdp.insert(
        "DOM.pushNodesByBackendIdsToFrontend".to_owned(),
        Ok(json!({ "nodeIds": [1] })),
    );
    host.cdp
        .insert("DOM.querySelector".to_owned(), Ok(json!({ "nodeId": 9 })));
    let selector = "a[href=\"');alert(1)//\"]";
    assert!(mark(&mut host, 3, &Locator::Selector(selector.to_owned())).expect("marked"));
    assert_eq!(
        host.methods(),
        [
            "DOM.enable",
            "Overlay.enable",
            "Accessibility.getFullAXTree",
            "DOM.pushNodesByBackendIdsToFrontend",
            "DOM.querySelector",
            "DOM.scrollIntoViewIfNeeded",
            "Overlay.highlightNode"
        ]
    );
    assert_eq!(host.cdp_sent[3].2["backendNodeIds"], json!([5]));
    assert_eq!(host.cdp_sent[4].2["nodeId"], 1);
    assert_eq!(host.cdp_sent[4].2["selector"], selector);
    assert_eq!(host.cdp_sent[6].2["nodeId"], 9);
    assert!(host.evaluated.is_empty(), "no script");
}

#[test]
fn the_document_is_requested_only_when_nobody_has() {
    let mut host = FakeHost::default();
    host.cdp.insert(
        "Accessibility.getFullAXTree".to_owned(),
        Ok(json!({ "nodes": [{ "backendDOMNodeId": 5 }] })),
    );
    host.cdp.insert(
        "DOM.pushNodesByBackendIdsToFrontend".to_owned(),
        Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED)),
    );
    host.cdp.insert(
        "DOM.getDocument".to_owned(),
        Ok(json!({ "root": { "nodeId": 1 } })),
    );
    host.cdp
        .insert("DOM.querySelector".to_owned(), Ok(json!({ "nodeId": 9 })));
    assert!(mark(&mut host, 3, &Locator::Selector("#a".to_owned())).expect("marked"));
    let methods = host.methods();
    assert_eq!(
        methods.iter().filter(|m| **m == "DOM.getDocument").count(),
        1
    );
    assert_eq!(methods[5], "DOM.querySelector");
    assert_eq!(host.cdp_sent[5].2["nodeId"], 1);
}

#[test]
fn a_missing_element_clears_the_old_mark() {
    let mut host = FakeHost::default();
    host.cdp.insert(
        "DOM.getDocument".to_owned(),
        Ok(json!({ "root": { "nodeId": 1 } })),
    );
    host.cdp
        .insert("DOM.querySelector".to_owned(), Ok(json!({ "nodeId": 0 })));
    assert!(!mark(&mut host, 1, &Locator::Selector("#gone".to_owned())).expect("answered"));
    assert_eq!(host.methods().last(), Some(&"Overlay.hideHighlight"));
}

#[test]
fn an_accessibility_node_is_marked_by_its_backend_id() {
    let mut host = FakeHost::default();
    assert!(mark(&mut host, 1, &Locator::Backend(42)).expect("marked"));
    assert_eq!(host.cdp_sent[3].2["backendNodeId"], 42);
    assert!(!host.methods().contains(&"DOM.querySelector"));
}
