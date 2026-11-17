use std::sync::Mutex;

use async_trait::async_trait;
use rb_engine::{EngineError, ViewId};
use serde_json::{Value, json};

use super::*;

/// A browser end per view: the document is node 1, the selector finds `found` (0: nothing), and
/// `broken` answers every call with an error.
struct FakeCdp {
    found: u64,
    broken: Option<ViewId>,
    calls: Mutex<Vec<(ViewId, String, Value)>>,
}

impl FakeCdp {
    fn new(found: u64) -> Self {
        Self {
            found,
            broken: None,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn methods(&self, view: ViewId) -> Vec<String> {
        self.calls
            .lock()
            .expect("the call list")
            .iter()
            .filter(|(of, _, _)| *of == view)
            .map(|(_, method, _)| method.clone())
            .collect()
    }

    fn params(&self, method: &str) -> Value {
        self.calls
            .lock()
            .expect("the call list")
            .iter()
            .find(|(_, called, _)| called == method)
            .map(|(_, _, params)| params.clone())
            .unwrap_or_else(|| panic!("{method} was not sent"))
    }
}

#[async_trait]
impl ViewCdp for FakeCdp {
    async fn send(&self, view: ViewId, method: &str, params: Value) -> Result<Value, EngineError> {
        self.calls
            .lock()
            .expect("the call list")
            .push((view, method.to_owned(), params));
        if self.broken == Some(view) {
            return Err(EngineError::Closed);
        }
        Ok(match method {
            "DOM.getDocument" => json!({ "root": { "nodeId": 1 } }),
            "DOM.querySelector" => json!({ "nodeId": self.found }),
            _ => json!({}),
        })
    }
}

/// A selector as a hostile plugin might write it: it must reach `DOM.querySelector` unchanged
/// and nowhere else.
const HOSTILE: &str = r#"img[alt="x"]'); alert(1); //"#;

#[tokio::test]
async fn the_element_is_marked_with_dom_and_overlay_calls_only() {
    let cdp = FakeCdp::new(7);
    let found = highlight_view(&cdp, ViewId(1), HOSTILE)
        .await
        .expect("marked");
    assert!(found);
    assert_eq!(
        cdp.methods(ViewId(1)),
        vec![
            "DOM.enable",
            "Overlay.enable",
            "DOM.getDocument",
            "DOM.querySelector",
            "DOM.scrollIntoViewIfNeeded",
            "Overlay.highlightNode",
        ]
    );
    assert_eq!(
        cdp.params("DOM.querySelector"),
        json!({ "nodeId": 1, "selector": HOSTILE })
    );
    assert_eq!(
        cdp.params("DOM.scrollIntoViewIfNeeded"),
        json!({ "nodeId": 7 })
    );
    let highlight = cdp.params("Overlay.highlightNode");
    assert_eq!(highlight["nodeId"], 7);
    assert!(highlight["highlightConfig"].is_object());
    let all = cdp.calls.lock().expect("the call list").clone();
    for (_, method, params) in all {
        assert!(!method.starts_with("Runtime."), "{method}");
        if method != "DOM.querySelector" {
            assert!(!params.to_string().contains("alert"), "{method}: {params}");
        }
    }
}

#[tokio::test]
async fn an_element_the_page_does_not_have_clears_the_old_mark() {
    let cdp = FakeCdp::new(0);
    let found = highlight_view(&cdp, ViewId(2), "img.gone")
        .await
        .expect("answered");
    assert!(!found);
    let methods = cdp.methods(ViewId(2));
    assert_eq!(
        methods.last().map(String::as_str),
        Some("Overlay.hideHighlight")
    );
    assert!(
        !methods
            .iter()
            .any(|method| method == "Overlay.highlightNode")
    );
}

#[tokio::test]
async fn every_view_is_marked_and_one_failing_stops_none_other() {
    let cdp = FakeCdp {
        broken: Some(ViewId(2)),
        ..FakeCdp::new(5)
    };
    let outcomes = highlight_all(&cdp, &[ViewId(1), ViewId(2), ViewId(3)], "img.hero").await;
    assert_eq!(outcomes.len(), 3);
    assert_eq!(outcomes[0].0, ViewId(1));
    assert!(matches!(outcomes[0].1, Ok(true)));
    assert!(outcomes[1].1.is_err());
    assert!(matches!(outcomes[2].1, Ok(true)));
}

#[test]
fn the_request_is_checked_before_anything_is_sent() {
    let known = [ViewId(1), ViewId(2)];
    let ok = |selector: &str| validate("9f2c4e1a", selector, &[ViewId(1)], &known);
    assert_eq!(ok("img.hero"), Ok(()));
    assert_eq!(ok(&"a".repeat(MAX_SELECTOR_CHARS)), Ok(()));
    assert_eq!(
        ok(&"\u{4e2d}".repeat(MAX_SELECTOR_CHARS)),
        Ok(()),
        "characters are counted, not bytes"
    );
    for selector in [
        String::new(),
        "   ".to_owned(),
        "a".repeat(MAX_SELECTOR_CHARS + 1),
        "img\n.hero".to_owned(),
        "img\u{0}".to_owned(),
    ] {
        assert_eq!(ok(&selector), Err(HighlightError::Selector), "{selector:?}");
    }
    assert_eq!(
        validate("9f2c4e1a", "img", &[ViewId(1), ViewId(9)], &known),
        Err(HighlightError::UnknownView(ViewId(9)))
    );
    assert_eq!(
        validate("9f2c4e1a", "img", &[], &known),
        Err(HighlightError::Views)
    );
    let many: Vec<ViewId> = (0..=u32::try_from(MAX_VIEWS).expect("small"))
        .map(ViewId)
        .collect();
    assert_eq!(
        validate("9f2c4e1a", "img", &many, &many),
        Err(HighlightError::Views)
    );
    for id in [
        String::new(),
        "x".repeat(MAX_ID_CHARS + 1),
        "a b".to_owned(),
    ] {
        assert_eq!(
            validate(&id, "img", &[ViewId(1)], &known),
            Err(HighlightError::FindingId),
            "{id:?}"
        );
    }
}
